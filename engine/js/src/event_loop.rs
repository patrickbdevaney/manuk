//! D3 **events tranche — the event loop** (first piece: the loop core).
//!
//! The HTML event loop is *run a task, then drain the microtask queue*, repeated.
//! This module provides that substrate over SpiderMonkey, host-side:
//!
//! - **Macrotasks** (`setTimeout` callbacks, and later DOM event dispatch): a FIFO of
//!   callback functions in a rooted global array `__tasks` (rooted for free because
//!   the global is). `setTimeout(fn)` pushes; [`run`] pops one per turn.
//! - **Microtasks** (`queueMicrotask` callbacks): a second FIFO `__micro`, drained
//!   **fully** at each checkpoint (a microtask may queue more microtasks), before the
//!   next macrotask and once at the end — the spec's microtask-before-next-task rule.
//!
//! Driving both queues through `evaluate_script` (rather than low-level `Call`) keeps
//! this piece small and rooting-safe: callbacks live in the rooted global arrays, not
//! in Rust.
//!
//! **Scope / honest limitation:** this is the host loop core (`setTimeout` +
//! `queueMicrotask` with correct ordering). It does **not** yet route native
//! `Promise.prototype.then` reactions through the loop: that needs SpiderMonkey's job
//! queue (`JS::UseInternalJobQueues` + `RunJobs`), whose mozjs-0.18 wrapper *segfaults
//! on call in this build* — a real environment blocker, tracked in CLAUDE.md § D3.
//! `queueMicrotask` (a first-class web API) gives the same ordering guarantees for
//! host-scheduled microtasks in the meantime. `addEventListener`/`dispatchEvent`
//! (DOM events routed through this loop) are the next piece.
//!
//! Modification boundary: sanctioned embedding APIs only (script evaluation); no
//! JIT/GC/sandbox internals.

use mozjs::rust::Runtime;

/// The host prelude: the macrotask + microtask FIFOs and their schedulers, plus the
/// `fetch`/`XMLHttpRequest` APIs whose I/O the Rust [`run_with_fetcher`] loop performs.
/// Evaluated once per global.
const PRELUDE: &str = r#"
    globalThis.__tasks = [];
    globalThis.__micro = [];
    globalThis.setTimeout = function(cb) {
        if (typeof cb === 'function') { __tasks.push(cb); }
        return __tasks.length;   // a fake, monotonic timer id
    };
    globalThis.queueMicrotask = function(cb) {
        if (typeof cb === 'function') { __micro.push(cb); }
    };

    // --- fetch / XMLHttpRequest -------------------------------------------
    // Requests are enqueued here (as "id\x01kind\x01method\x01url\x01body" strings). The Rust
    // loop (run_with_fetcher) or the host (drain_pending + deliver) performs the network I/O,
    // then calls __deliver(id, status, body) — kind-agnostic; it routes to the right settler.
    // fetch() returns a REAL Promise (native promise jobs are routed into this loop via
    // job_queue), so `.then(...).then(...)` chains and `await` work.
    globalThis.__fetchId = 0;
    globalThis.__fetchCb = {};   // id -> {resolve, reject}  (fetch)
    globalThis.__xhrObj = {};    // id -> XMLHttpRequest     (xhr)
    globalThis.__pendingFetches = [];

    globalThis.__makeResponse = function(status, text) {
        return {
            ok: (status >= 200 && status < 300), status: status, statusText: "",
            url: "", redirected: false, type: "basic", bodyUsed: false, body: null,
            headers: { get: function(){ return null; }, has: function(){ return false; },
                       forEach: function(){} },
            text: function(){ return Promise.resolve(text); },
            json: function(){ try { return Promise.resolve(JSON.parse(text)); }
                              catch (e) { return Promise.reject(e); } },
            clone: function(){ return globalThis.__makeResponse(status, text); }
        };
    };

    // fetch(url[, {method, body}]) -> Promise<Response-like>.
    globalThis.fetch = function(url, opts) {
        var id = ++__fetchId;
        var method = (opts && opts.method) || "GET";
        var body = (opts && opts.body != null) ? String(opts.body) : "";
        __pendingFetches.push(id + "\x01f\x01" + method + "\x01" + url + "\x01" + body);
        return new Promise(function(resolve, reject){ __fetchCb[id] = { resolve: resolve, reject: reject }; });
    };
    globalThis.__deliverFetch = function(id, status, text) {
        var cb = __fetchCb[id]; if (!cb) return; delete __fetchCb[id];
        if (status === 0) { cb.reject(new TypeError("Failed to fetch")); return; }
        cb.resolve(globalThis.__makeResponse(status, text));
    };

    globalThis.XMLHttpRequest = function() {
        this.readyState = 0; this.status = 0; this.statusText = "";
        this.responseText = ""; this.response = ""; this.responseType = "";
        this.onload = null; this.onerror = null; this.onreadystatechange = null;
        this._m = "GET"; this._u = ""; this._id = null;
    };
    XMLHttpRequest.prototype.open = function(m, u) { this._m = m || "GET"; this._u = u || ""; this.readyState = 1; };
    XMLHttpRequest.prototype.setRequestHeader = function() {};
    XMLHttpRequest.prototype.getAllResponseHeaders = function() { return ""; };
    XMLHttpRequest.prototype.getResponseHeader = function() { return null; };
    XMLHttpRequest.prototype.abort = function() {};
    XMLHttpRequest.prototype.send = function(body) {
        var id = ++__fetchId; __xhrObj[id] = this; this._id = id;
        __pendingFetches.push(id + "\x01x\x01" + this._m + "\x01" + this._u + "\x01" + (body != null ? String(body) : ""));
    };
    globalThis.__deliverXhr = function(id, status, text) {
        var x = __xhrObj[id]; if (!x) return; delete __xhrObj[id];
        x.status = status; x.statusText = ""; x.responseText = text;
        x.response = (x.responseType === "json")
            ? (function(){ try { return JSON.parse(text); } catch (e) { return null; } })()
            : text;
        x.readyState = 4;
        if (typeof x.onreadystatechange === 'function') { try { x.onreadystatechange(); } catch (e) {} }
        if (status === 0) { if (typeof x.onerror === 'function') { try { x.onerror(new Error("network")); } catch (e) {} } }
        else if (typeof x.onload === 'function') { try { x.onload(); } catch (e) {} }
    };

    // Kind-agnostic delivery: the host settles a request by id without tracking whether it was
    // a fetch or an XHR.
    globalThis.__deliver = function(id, status, text) {
        if (__fetchCb[id]) { globalThis.__deliverFetch(id, status, text); return; }
        if (__xhrObj[id]) { globalThis.__deliverXhr(id, status, text); return; }
    };

    // ---------------------------------------------------------------------------------------------
    // **DOM interface constructors — because frameworks `instanceof` constantly.**
    //
    // React's scheduled work throws `invalid 'instanceof' operand` on `node instanceof
    // HTMLIFrameElement` — not because the node is wrong, but because the CONSTRUCTOR is `undefined`,
    // and `x instanceof undefined` is a TypeError. That single missing global stops React's render
    // dead, after `nodeType` and `ownerDocument` have already let it get that far.
    //
    // Our reflectors are plain objects, so there is no real prototype chain to hang these off. But
    // `instanceof` does not require one: `Symbol.hasInstance` lets a constructor answer the question
    // directly, which is exactly the question the frameworks are asking — "is this an iframe?", "is
    // this an input?" — and answering it correctly matters far more than the prototype chain they are
    // using to ask it.
    //
    // (Also `MessageChannel` and `performance`, both of which the schedulers feature-detect. React
    // falls back gracefully without them; plenty of libraries do not.)
    (function(){
      // **Never clobber a constructor that already exists — it is load-bearing.**
      //
      // `HTMLElement` is not an inert marker here: the custom-elements shim defines it, and its
      // constructor RETURNS the element under upgrade so that `class X extends HTMLElement` gets the
      // real element as its `this`. Replacing it with a throwing "Illegal constructor" broke every
      // custom element and every `attachShadow` in the JS conformance suite — the gate caught it
      // immediately, which is the entire reason the gate exists.
      //
      // So: attach `Symbol.hasInstance` to whatever is already there, and only *define* the ones that
      // do not exist. The frameworks get their `instanceof` either way, and nothing that already works
      // stops working.
      function iface(name, test) {
        var C = globalThis[name];
        if (typeof C !== 'function') {
          // Constructible and inert — NOT throwing. A base class that throws on `super()` is
          // indistinguishable, from the author's side, from a browser that does not support classes.
          C = function(){ return this; };
          Object.defineProperty(C, 'name', { value: name });
          C.prototype = {};
          globalThis[name] = C;
        }
        try { Object.defineProperty(C, Symbol.hasInstance, { value: test, configurable: true }); }
        catch (e) { /* a frozen builtin: its own instanceof is already right */ }
        return C;
      }
      var isEl   = function(o){ return !!o && o.nodeType === 1; };
      var isNode = function(o){ return !!o && typeof o.nodeType === 'number'; };
      var tagIs  = function(t){ return function(o){ return isEl(o) && o.tagName === t; }; };

      iface('Node', isNode);
      iface('Element', isEl);
      iface('HTMLElement', isEl);
      iface('SVGElement', function(o){ return isEl(o) && o.tagName === 'SVG'; });
      iface('Text', function(o){ return !!o && o.nodeType === 3; });
      iface('Comment', function(o){ return !!o && o.nodeType === 8; });
      iface('DocumentFragment', function(o){ return !!o && o.nodeType === 11; });
      iface('Document', function(o){ return o === document; });
      iface('Window', function(o){ return o === globalThis; });

      iface('HTMLIFrameElement',   tagIs('IFRAME'));
      iface('HTMLInputElement',    tagIs('INPUT'));
      iface('HTMLTextAreaElement', tagIs('TEXTAREA'));
      iface('HTMLSelectElement',   tagIs('SELECT'));
      iface('HTMLOptionElement',   tagIs('OPTION'));
      iface('HTMLButtonElement',   tagIs('BUTTON'));
      iface('HTMLAnchorElement',   tagIs('A'));
      iface('HTMLImageElement',    tagIs('IMG'));
      iface('HTMLFormElement',     tagIs('FORM'));
      iface('HTMLCanvasElement',   tagIs('CANVAS'));
      iface('HTMLScriptElement',   tagIs('SCRIPT'));
      iface('HTMLStyleElement',    tagIs('STYLE'));
      iface('HTMLLinkElement',     tagIs('LINK'));
      iface('HTMLTemplateElement', tagIs('TEMPLATE'));
      iface('HTMLDivElement',      tagIs('DIV'));
      iface('HTMLSpanElement',     tagIs('SPAN'));

      // `performance.now()` — schedulers, profilers and animation libraries all feature-detect it and
      // most fall back to `Date.now()`. The ones that don't simply break.
      if (typeof globalThis.performance === 'undefined') {
        var t0 = Date.now();
        globalThis.performance = {
          now: function(){ return Date.now() - t0; },
          mark: function(){}, measure: function(){},
          getEntriesByName: function(){ return []; }, getEntriesByType: function(){ return []; },
          timeOrigin: t0
        };
      }

      // `MessageChannel` — React's scheduler prefers it over setTimeout for yielding. Implemented on
      // the microtask queue, which is the closest thing we have to "after the current task, before
      // paint" and is what the schedulers actually want it for.
      if (typeof globalThis.MessageChannel === 'undefined') {
        globalThis.MessageChannel = function() {
          var p1 = { onmessage: null }, p2 = {};
          p1.postMessage = function(d){ queueMicrotask(function(){ if (p2.onmessage) p2.onmessage({ data: d }); }); };
          p2.postMessage = function(d){ queueMicrotask(function(){ if (p1.onmessage) p1.onmessage({ data: d }); }); };
          p1.close = function(){}; p2.close = function(){};
          p1.start = function(){}; p2.start = function(){};
          this.port1 = p1; this.port2 = p2;
        };
      }
      // **`Error.captureStackTrace` — V8-only, and a meaningful number of popular libraries
      // feature-detect it and depend on it for custom error classes** (METHODOLOGY Part 30.3). Real,
      // recurring "works in Chrome, throws in Firefox" bugs exist against exactly this API family
      // across widely-used libraries — which is why it is now a TC39 proposal, and why implementing it
      // is adopting something headed for the spec rather than chasing a V8 quirk.
      //
      // A shim in the embedding layer, not a SpiderMonkey patch. Consistent with "never patch
      // Stylo/SpiderMonkey", which is a settled decision and not up for relitigation.
      if (typeof Error.captureStackTrace !== 'function') {
        Error.captureStackTrace = function(target, ctor) {
          // SpiderMonkey already puts a `stack` on every Error; the V8 contract is only that `target`
          // ends up WITH one. Handing back the real stack is strictly better than the empty string
          // most shims settle for.
          try {
            var e = new Error();
            Object.defineProperty(target, 'stack', {
              value: (e.stack || ''), writable: true, configurable: true
            });
          } catch (_) { try { target.stack = ''; } catch (__) {} }
        };
      }
      if (typeof Error.stackTraceLimit !== 'number') { Error.stackTraceLimit = 10; }

      if (typeof globalThis.requestIdleCallback === 'undefined') {
        globalThis.requestIdleCallback = function(cb){
          return setTimeout(function(){ cb({ didTimeout: false, timeRemaining: function(){ return 5; } }); }, 0);
        };
        globalThis.cancelIdleCallback = function(){};
      }
    })();
"#;

/// Run the next macrotask if any; report whether one ran.
const NEXT_TASK: &str =
    "(function(){ if (__tasks.length === 0) return false; var t = __tasks.shift(); t(); return true; })()";

/// Drain the microtask queue completely (microtasks may enqueue more microtasks).
const DRAIN_MICRO: &str =
    "(function(){ while (__micro.length) { var m = __micro.shift(); m(); } })()";

/// Shift the next pending network request as `"id\x01kind\x01method\x01url"`, or null.
const NEXT_PENDING: &str =
    "(function(){ return __pendingFetches.length ? __pendingFetches.shift() : null; })()";

/// Install the event-loop host surface onto `global` (the queues + schedulers). Call
/// once, inside the global's realm.
pub fn install(rt: &mut Runtime, global: mozjs::rust::HandleObject) -> Result<(), String> {
    // N9: route SpiderMonkey's native promise jobs into this loop. Installed once per
    // process; `SetJobQueue` has no ordering constraint (unlike `UseInternalJobQueues`,
    // which must precede `InitSelfHostedCode` and therefore cannot be used with mozjs's
    // `Runtime::new` — see the N7 research note).
    let raw_cx = unsafe { rt.cx().raw_cx() };
    unsafe { crate::job_queue::install_once(raw_cx) }?;
    eval(rt, global, PRELUDE, "event_loop_prelude.js").map(|_| ())
}

/// A **microtask checkpoint**: drain the host `queueMicrotask` queue *and* SpiderMonkey's
/// native promise-job queue. Both must run, and both must run to quiescence — a job may
/// enqueue another job.
fn microtask_checkpoint(rt: &mut Runtime, global: mozjs::rust::HandleObject) -> Result<(), String> {
    eval(rt, global, DRAIN_MICRO, "event_loop_micro.js")?;
    let raw_cx = unsafe { rt.cx().raw_cx() };
    unsafe { crate::job_queue::drain(raw_cx) };
    // A native reaction may have called `queueMicrotask`; drain again so the checkpoint is
    // a fixed point rather than one pass.
    eval(rt, global, DRAIN_MICRO, "event_loop_micro.js")?;
    Ok(())
}

/// Run the event loop to quiescence with **no network** (pending `fetch`/XHR requests,
/// if any, resolve with status 0). See [`run_with_fetcher`] for the I/O-enabled loop.
pub fn run(rt: &mut Runtime, global: mozjs::rust::HandleObject) -> Result<u32, String> {
    run_with_fetcher(rt, global, |_method, _url| (0, String::new()))
}

/// Run the event loop to quiescence **without touching the network**: microtasks + timers run,
/// but pending `fetch`/XHR requests are left queued in `__pendingFetches` for the *host* to
/// perform (via [`drain_pending`] + [`deliver`]). This is the interactive path — a page's
/// `fetch` must reach real `manuk-net` I/O on the shell thread, not resolve to status 0 inline.
/// Returns the number of macrotasks run.
pub fn run_deferred(rt: &mut Runtime, global: mozjs::rust::HandleObject) -> Result<u32, String> {
    let mut count = 0u32;
    loop {
        microtask_checkpoint(rt, global)?;
        let ran = eval(rt, global, NEXT_TASK, "event_loop_tick.js")?;
        if ran.is_boolean() && ran.to_boolean() {
            count += 1;
            continue;
        }
        break;
    }
    microtask_checkpoint(rt, global)?;
    Ok(count)
}

/// Drain the page's queued `fetch`/XHR requests, returning `(id, url, method, body)` for each so
/// the host can perform them over the real network. Kind (`fetch` vs XHR) is intentionally
/// dropped — [`deliver`] settles by id regardless.
pub fn drain_pending(
    rt: &mut Runtime,
    global: mozjs::rust::HandleObject,
) -> Result<Vec<(u32, String, String, String)>, String> {
    let mut out = Vec::new();
    while let Some(req) = eval_string(rt, global, NEXT_PENDING, "event_loop_drain.js")? {
        let parts: Vec<&str> = req.splitn(5, '\u{1}').collect();
        if parts.len() < 4 {
            continue;
        }
        let id: u32 = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let method = parts[2].to_string();
        let url = parts[3].to_string();
        let body = parts.get(4).map(|s| s.to_string()).unwrap_or_default();
        out.push((id, url, method, body));
    }
    Ok(out)
}

/// Settle a page request by `id` with an HTTP `status` + response `body` (`status == 0` =
/// network failure). Routes to the fetch Promise or XHR callbacks (kind-agnostic). The caller
/// runs [`run_deferred`] afterward to process the reactions.
pub fn deliver(
    rt: &mut Runtime,
    global: mozjs::rust::HandleObject,
    id: u32,
    status: u16,
    body: &str,
) -> Result<(), String> {
    let script = format!("__deliver({}, {}, {})", id, status, js_string_literal(body));
    eval(rt, global, &script, "event_loop_deliver.js").map(|_| ())
}

/// Run the event loop to quiescence, performing pending `fetch`/XHR I/O via `fetch`
/// (`fn(method, url) -> (status, body)` — production injects a `manuk-net`-backed
/// closure; tests inject a deterministic mock). Each turn: drain microtasks → perform
/// all pending network requests (delivering each result back into JS) → drain
/// microtasks → run one macrotask. Loops while any work remains. Returns the number
/// of macrotasks run.
pub fn run_with_fetcher<F>(
    rt: &mut Runtime,
    global: mozjs::rust::HandleObject,
    fetch: F,
) -> Result<u32, String>
where
    F: Fn(&str, &str) -> (u16, String),
{
    let mut count = 0u32;
    loop {
        microtask_checkpoint(rt, global)?;

        // Perform all currently-pending network requests.
        let mut did_io = false;
        while let Some(req) = eval_string(rt, global, NEXT_PENDING, "event_loop_net.js")? {
            did_io = true;
            // "id\x01kind\x01method\x01url\x01body" (body optional for back-compat).
            let parts: Vec<&str> = req.splitn(5, '\u{1}').collect();
            if parts.len() < 4 {
                continue;
            }
            let (id, kind, method, url) = (parts[0], parts[1], parts[2], parts[3]);
            let (status, body) = fetch(method, url);
            let deliver = if kind == "x" {
                format!(
                    "__deliverXhr({}, {}, {})",
                    id,
                    status,
                    js_string_literal(&body)
                )
            } else {
                format!(
                    "__deliverFetch({}, {}, {})",
                    id,
                    status,
                    js_string_literal(&body)
                )
            };
            eval(rt, global, &deliver, "event_loop_deliver.js")?;
        }

        microtask_checkpoint(rt, global)?; // delivery may queue micro/promise jobs

        let ran = eval(rt, global, NEXT_TASK, "event_loop_tick.js")?;
        let ran = ran.is_boolean() && ran.to_boolean();
        if ran {
            count += 1;
            continue;
        }
        if did_io {
            continue; // a delivered result may have scheduled more work
        }
        break;
    }
    microtask_checkpoint(rt, global)?; // final checkpoint
    Ok(count)
}

/// Escape a Rust string as a JS double-quoted string literal (for embedding a fetched
/// body into a `__deliver*` call).
fn js_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Evaluate `src` and return its (unrooted, immediately-consumed) value. Kept local
/// so callers never touch rooting.
fn eval(
    rt: &mut Runtime,
    global: mozjs::rust::HandleObject,
    src: &str,
    file: &str,
) -> Result<mozjs::jsapi::Value, String> {
    use mozjs::jsval::UndefinedValue;
    use mozjs::rooted;
    use mozjs::rust::{evaluate_script, CompileOptionsWrapper};
    rooted!(&in(rt.cx()) let mut rval = UndefinedValue());
    let fname = std::ffi::CString::new(file).unwrap_or_default();
    let opts = CompileOptionsWrapper::new(rt.cx_no_gc(), fname, 1);
    evaluate_script(rt.cx(), global, src, rval.handle_mut(), opts)
        .map_err(|()| format!("event-loop eval failed: {file}"))?;
    Ok(rval.get())
}

/// Evaluate `src` and read its result as a Rust `String`, or `None` if the result is
/// null/undefined. The value stays rooted across the conversion.
fn eval_string(
    rt: &mut Runtime,
    global: mozjs::rust::HandleObject,
    src: &str,
    file: &str,
) -> Result<Option<String>, String> {
    use mozjs::conversions::{ConversionResult, FromJSValConvertible};
    use mozjs::jsval::UndefinedValue;
    use mozjs::rooted;
    use mozjs::rust::{evaluate_script, CompileOptionsWrapper};
    rooted!(&in(rt.cx()) let mut rval = UndefinedValue());
    let fname = std::ffi::CString::new(file).unwrap_or_default();
    let opts = CompileOptionsWrapper::new(rt.cx_no_gc(), fname, 1);
    evaluate_script(rt.cx(), global, src, rval.handle_mut(), opts)
        .map_err(|()| format!("event-loop eval failed: {file}"))?;
    if rval.get().is_null() || rval.get().is_undefined() {
        return Ok(None);
    }
    match String::safe_from_jsval(rt.cx(), rval.handle(), ()) {
        Ok(ConversionResult::Success(s)) => Ok(Some(s)),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mozjs::jsapi::OnNewGlobalHookOption;
    use mozjs::rooted;
    use mozjs::rust::wrappers2::JS_NewGlobalObject;
    use mozjs::rust::{RealmOptions, SIMPLE_GLOBAL_CLASS};
    use std::ptr;

    // Run isolated (SpiderMonkey multi-Runtime teardown):
    //   cargo test -p manuk-js --features spidermonkey event_loop -- --ignored
    #[test]
    #[ignore = "SpiderMonkey multi-Runtime-per-process teardown; run in isolation"]
    fn microtasks_run_before_macrotasks() {
        let handle = crate::spidermonkey::engine_handle().expect("engine");
        let mut runtime = Runtime::new(handle);
        let options = RealmOptions::default();
        rooted!(&in(runtime.cx()) let global = unsafe {
            JS_NewGlobalObject(runtime.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook, &*options)
        });
        let _ar = mozjs::jsapi::JSAutoRealm::new(unsafe { runtime.cx().raw_cx() }, global.get());

        install(&mut runtime, global.handle()).expect("install event loop");

        // A microtask (p), and a macrotask (T) that itself queues a nested microtask
        // (m). The loop must drain the microtask BEFORE the macrotask, then run the
        // macrotask, then drain its nested microtask → "pTm". A second macrotask (U)
        // confirms FIFO macrotask order → final "pTmU".
        let user = r#"
            globalThis.log = "";
            queueMicrotask(function(){ log += "p"; });
            setTimeout(function(){ log += "T"; queueMicrotask(function(){ log += "m"; }); });
            setTimeout(function(){ log += "U"; });
        "#;
        eval(&mut runtime, global.handle(), user, "user.js").expect("user script");

        let ran = run(&mut runtime, global.handle()).expect("run loop");
        assert_eq!(ran, 2, "two macrotasks ran");

        let ok =
            eval(&mut runtime, global.handle(), "log === 'pTmU'", "check.js").expect("read log");
        assert!(
            ok.is_boolean() && ok.to_boolean(),
            "expected ordering pTmU (microtask before macrotask, FIFO macrotasks)"
        );
    }

    #[test]
    #[ignore = "SpiderMonkey multi-Runtime-per-process teardown; run in isolation"]
    fn fetch_and_xhr_through_the_loop() {
        let handle = crate::spidermonkey::engine_handle().expect("engine");
        let mut runtime = Runtime::new(handle);
        let options = RealmOptions::default();
        rooted!(&in(runtime.cx()) let global = unsafe {
            JS_NewGlobalObject(runtime.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook, &*options)
        });
        let _ar = mozjs::jsapi::JSAutoRealm::new(unsafe { runtime.cx().raw_cx() }, global.get());

        install(&mut runtime, global.handle()).expect("install event loop");

        // A fetch (real Promise) and an XHR (callback), both delivered by the loop's I/O.
        let user = r#"
            globalThis.fr = "";
            globalThis.xr = "";
            fetch("http://host/api").then(function(r){
                return r.text().then(function(t){ fr = r.status + ":" + t + ":" + r.ok; });
            });
            var x = new XMLHttpRequest();
            x.open("POST", "http://host/data");
            x.onload = function(){ xr = x.status + ":" + x.responseText; };
            x.send();
        "#;
        eval(&mut runtime, global.handle(), user, "user.js").expect("user script");

        // Deterministic mock fetcher keyed on the URL/method.
        let fetch = |method: &str, url: &str| -> (u16, String) {
            if url.ends_with("/api") {
                (200, "hello".to_string())
            } else if method == "POST" {
                (201, "world".to_string())
            } else {
                (404, String::new())
            }
        };
        run_with_fetcher(&mut runtime, global.handle(), fetch).expect("run loop");

        let ok = eval(
            &mut runtime,
            global.handle(),
            "fr === '200:hello:true' && xr === '201:world'",
            "check.js",
        )
        .expect("read results");
        assert!(
            ok.is_boolean() && ok.to_boolean(),
            "fetch thenable and XHR onload should receive the mocked responses via the loop"
        );
    }
}
