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
    // Requests are enqueued here (as "id\x01kind\x01method\x01url" strings) and the
    // Rust event loop performs the network I/O, then calls __deliverFetch/__deliverXhr.
    globalThis.__fetchId = 0;
    globalThis.__fetchCb = {};
    globalThis.__xhrObj = {};
    globalThis.__pendingFetches = [];

    // fetch(url[, {method}]) -> a thenable resolving with a Response-like object.
    // (A host thenable, not a native Promise — the native promise-job queue is
    // blocked in this mozjs build; see module docs.)
    globalThis.fetch = function(url, opts) {
        var id = ++__fetchId;
        var method = (opts && opts.method) || "GET";
        __pendingFetches.push(id + "\x01f\x01" + method + "\x01" + url);
        var thenable = { then: function(onF) { __fetchCb[id] = onF; return thenable; } };
        return thenable;
    };
    globalThis.__deliverFetch = function(id, status, text) {
        var cb = __fetchCb[id]; if (!cb) return; delete __fetchCb[id];
        cb({ status: status, ok: (status >= 200 && status < 300),
             body: text, text: function(){ return text; } });
    };

    globalThis.XMLHttpRequest = function() {
        this.readyState = 0; this.status = 0; this.responseText = "";
        this.onload = null; this._m = "GET"; this._u = "";
    };
    XMLHttpRequest.prototype.open = function(m, u) { this._m = m; this._u = u; this.readyState = 1; };
    XMLHttpRequest.prototype.send = function() {
        var id = ++__fetchId; __xhrObj[id] = this; this._id = id;
        __pendingFetches.push(id + "\x01x\x01" + this._m + "\x01" + this._u);
    };
    globalThis.__deliverXhr = function(id, status, text) {
        var x = __xhrObj[id]; if (!x) return; delete __xhrObj[id];
        x.status = status; x.responseText = text; x.readyState = 4;
        if (typeof x.onload === 'function') x.onload();
    };
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
    eval(rt, global, PRELUDE, "event_loop_prelude.js").map(|_| ())
}

/// Run the event loop to quiescence with **no network** (pending `fetch`/XHR requests,
/// if any, resolve with status 0). See [`run_with_fetcher`] for the I/O-enabled loop.
pub fn run(rt: &mut Runtime, global: mozjs::rust::HandleObject) -> Result<u32, String> {
    run_with_fetcher(rt, global, |_method, _url| (0, String::new()))
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
        eval(rt, global, DRAIN_MICRO, "event_loop_micro.js")?; // checkpoint

        // Perform all currently-pending network requests.
        let mut did_io = false;
        while let Some(req) = eval_string(rt, global, NEXT_PENDING, "event_loop_net.js")? {
            did_io = true;
            let parts: Vec<&str> = req.splitn(4, '\u{1}').collect();
            if parts.len() != 4 {
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

        eval(rt, global, DRAIN_MICRO, "event_loop_micro.js")?; // delivery may queue micro

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
    eval(rt, global, DRAIN_MICRO, "event_loop_micro.js")?; // final checkpoint
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

        // A fetch (thenable) and an XHR (callback), both delivered by the loop's I/O.
        let user = r#"
            globalThis.fr = "";
            globalThis.xr = "";
            fetch("http://host/api").then(function(r){ fr = r.status + ":" + r.body + ":" + r.ok; });
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
