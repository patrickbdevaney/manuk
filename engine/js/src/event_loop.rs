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
/// A runaway task chain must not hang the browser. See `run_deferred`.
const MAX_TASKS_PER_DRAIN: u32 = 20_000;

const PRELUDE: &str = r#"
    globalThis.__tasks = [];
    globalThis.__micro = [];
    // **Timers, with cancellation and repetition.** `clearTimeout`, `setInterval` and `clearInterval`
    // did not exist at all — so every carousel, clock, poller, progress bar and "check again in 5s" on
    // the web either threw or silently never ran. A page cannot even STOP a timer it started.
    //
    // Intervals reschedule themselves, which is exactly the shape that turns "drain to quiescence" into
    // "never return" — the drain is capped for that reason (see MAX_TASKS_PER_DRAIN). An interval that
    // outlives the load simply stops when the page does, which is the honest behaviour for a renderer
    // that is not running a live UI thread.
    globalThis.__timerId = 0;
    globalThis.__cancelled = {};

    // ── **THE DELAY IS NOT DECORATION. `setTimeout(cb, ms)` USED TO THROW `ms` AWAY.** ────────────
    //
    // Every timer was a bare FIFO push, so `setTimeout(f, 10000)` ran BEFORE a `setTimeout(g, 0)`
    // queued after it. Insertion order, not time order. That silently mis-orders every debounce,
    // throttle, retry-backoff, staged animation and "show the spinner, then check again in 5s" on the
    // open web — and none of it *errors*, it just happens in the wrong order, which is precisely the
    // kind of bug a box-diff against Chromium cannot see.
    //
    // WPT found it instantly and unmistakably: `testharness.js` arms its own 10-second harness
    // timeout up front, so OUR loop fired the timeout *before the async tests it was guarding*. The
    // whole suite reported TIMEOUT and the score looked like a conformance problem. It was a clock
    // problem.
    //
    // **The clock is VIRTUAL.** Tasks are ordered by `(due, seq)` and time jumps forward to whatever
    // is due next — we never actually sleep. A headless load must not take ten real seconds because
    // the page armed a ten-second timer; it must only run that timer LAST. Ordering is the property
    // that matters; waiting is not.
    globalThis.__now = 0;      // the virtual clock, in ms. Monotonic, never rewinds.
    globalThis.__seq = 0;      // ties at the same due time break FIFO, as the spec requires.

    // **The virtual clock may not run AHEAD OF THE PAGE'S LIFECYCLE.**
    //
    // Collapsing time is what makes a headless load fast — we never sleep, we just run the next-due
    // task. But it has a trap, and WPT walked straight into it: while the page is still loading, the
    // only task left is often a LONG timer, so the clock leaps to it and fires it **before `load`
    // ever happens.**
    //
    // `testharness.js` arms a 10-second harness timeout at setup. Our loop drained everything else,
    // jumped the clock to 10s, fired the timeout, and testharness declared TIMEOUT — *and only then*
    // did we fire `load`, into a page that had already given up. Every single test file reported
    // TIMEOUT and it looked like a conformance catastrophe. It was a **clock that ran ahead of the
    // document**.
    //
    // So during load the budget is 0: only tasks due NOW may run — which is exactly what a real
    // browser does, since real time has barely advanced. `load` opens the budget, and the delayed
    // timers then run in correct order behind it.
    globalThis.__timeBudget = 0;
    globalThis.__enqueue = function(fn, ms) {
        ms = (typeof ms === 'number' && ms > 0) ? ms : 0;
        __tasks.push({ f: fn, w: __now + ms, s: ++__seq });
    };
    globalThis.setTimeout = function(cb, ms) {
        if (typeof cb !== 'function') { return 0; }
        var id = ++__timerId;
        __enqueue(function(){ if (!__cancelled[id]) { cb(); } }, ms);
        return id;
    };
    globalThis.clearTimeout = function(id) { if (id) { __cancelled[id] = true; } };
    globalThis.setInterval = function(cb, ms) {
        if (typeof cb !== 'function') { return 0; }
        var id = ++__timerId;
        var tick = function() {
            if (__cancelled[id]) { return; }
            cb();
            // Reschedule at +ms from NOW, so an interval is a cadence rather than a tight loop.
            if (!__cancelled[id]) { __enqueue(tick, ms); }
        };
        __enqueue(tick, ms);
        return id;
    };
    globalThis.clearInterval = function(id) { if (id) { __cancelled[id] = true; } };
    globalThis.queueMicrotask = function(cb) {
        if (typeof cb === 'function') { __micro.push(cb); }
    };

    // --- fetch / XMLHttpRequest -------------------------------------------
    // Requests are enqueued here (as "id\x01kind\x01method\x01url\x01headers\x01body" strings,
    // headers being "name\x02value\x02…"; body is the greedy tail so it may itself contain \x01). The Rust
    // loop (run_with_fetcher) or the host (drain_pending + deliver) performs the network I/O,
    // then calls __deliver(id, status, body) — kind-agnostic; it routes to the right settler.
    // fetch() returns a REAL Promise (native promise jobs are routed into this loop via
    // job_queue), so `.then(...).then(...)` chains and `await` work.
    globalThis.__fetchId = 0;
    globalThis.__fetchCb = {};   // id -> {resolve, reject}  (fetch)
    globalThis.__xhrObj = {};    // id -> XMLHttpRequest     (xhr)
    globalThis.__pendingFetches = [];

    // A `Headers`-like over the server's actual response headers — `pairs` is an array of
    // `[name, value]`. Names match case-insensitively (HTTP header names are case-insensitive), and
    // `get` comma-joins repeated fields, both per the Fetch standard. Empty/omitted `pairs` yields a
    // Headers whose `get` returns null (the pre-header behaviour), so the mock-fetcher event loop —
    // which delivers with no headers — keeps working unchanged.
    globalThis.__makeHeaders = function(pairs) {
        var norm = [];
        if (pairs) { for (var i = 0; i < pairs.length; i++) {
            if (pairs[i] && pairs[i].length >= 2) norm.push([String(pairs[i][0]), String(pairs[i][1])]);
        } }
        return {
            get: function(name){
                name = String(name).toLowerCase(); var vals = [];
                for (var i = 0; i < norm.length; i++) { if (norm[i][0].toLowerCase() === name) vals.push(norm[i][1]); }
                return vals.length ? vals.join(", ") : null;
            },
            has: function(name){
                name = String(name).toLowerCase();
                for (var i = 0; i < norm.length; i++) { if (norm[i][0].toLowerCase() === name) return true; }
                return false;
            },
            forEach: function(cb, thisArg){
                for (var i = 0; i < norm.length; i++) { cb.call(thisArg, norm[i][1], norm[i][0], this); }
            }
        };
    };

    // ── ReadableStream — a REAL one, because `response.body` is the streaming entry point. ──────
    // The canonical streaming read on the web is:
    //
    //     const reader = (await fetch(url)).body.getReader();
    //     for (;;) { const {done, value} = await reader.read(); if (done) break; ... }
    //
    // Until this existed, `ReadableStream` was one of the `__inertNames` stubs (a named, EMPTY
    // constructor with no `getReader`) and `__makeResponse` hardcoded `body: null` — so that first
    // line threw a TypeError *inside* the response handler and took the whole handler with it. The
    // symptom is not "the answer streams in slowly", it is **the answer never appears**: every AI
    // chat, cloud-console live-log tail and inference token stream ships exactly this loop.
    //
    // Note that `typeof ReadableStream === 'function'` was ALREADY true against the stub — which is
    // why the gate asserts a reader that actually READS, not a name that exists (the `g_globals`
    // lesson; see the `__inertNames` comment). Defining it HERE, ahead of the inert sweep that runs
    // last, is what suppresses the stub — the same mechanism `AbortSignal` uses.
    if (typeof globalThis.ReadableStream === 'undefined') {
        var RS = function ReadableStream(source) {
            this.__q = [];          // chunks enqueued and not yet read
            this.__done = false;    // the source called close()
            this.__err = null;      // the source called error()
            this.__locked = false;  // a reader holds the stream
            this.__waiters = [];    // read() calls parked on an empty queue
            var self = this;
            this.__controller = {
                enqueue: function(c) { self.__push(c); },
                close:   function()  { self.__close(); },
                error:   function(e) { self.__fail(e); },
                get desiredSize() { return self.__done ? null : 1; }
            };
            this.__src = source || null;
            if (source && typeof source.start === 'function') {
                try {
                    var p = source.start(this.__controller);
                    if (p && typeof p.then === 'function') { p.then(null, function(e){ self.__fail(e); }); }
                } catch (e) { this.__fail(e); }
            }
        };
        // Settle one parked read() if we can, else leave it parked.
        RS.prototype.__serve = function() {
            while (this.__waiters.length) {
                if (this.__err !== null)      { this.__waiters.shift().reject(this.__err); continue; }
                if (this.__q.length)          { this.__waiters.shift().resolve({ value: this.__q.shift(), done: false }); continue; }
                if (this.__done)              { this.__waiters.shift().resolve({ value: undefined, done: true }); continue; }
                return;
            }
        };
        RS.prototype.__push  = function(c) { if (!this.__done && this.__err === null) { this.__q.push(c); this.__serve(); } };
        RS.prototype.__close = function()  { this.__done = true; this.__serve(); };
        RS.prototype.__fail  = function(e) { if (this.__err === null) { this.__err = e; this.__serve(); } };
        RS.prototype.getReader = function() {
            if (this.__locked) throw new TypeError("ReadableStream is locked to a reader");
            this.__locked = true;
            return new globalThis.ReadableStreamDefaultReader(this);
        };
        RS.prototype.cancel = function(reason) {
            this.__q = []; this.__done = true; this.__serve();
            if (this.__src && typeof this.__src.cancel === 'function') {
                try { this.__src.cancel(reason); } catch (e) {}
            }
            return Promise.resolve(undefined);
        };
        // `tee()` — an AI SDK routinely forks the token stream (one branch to the UI, one to a log).
        // Pump the source once and mirror every chunk into both branches.
        RS.prototype.tee = function() {
            var reader = this.getReader(), cA = null, cB = null;
            var mk = function(assign) { return new RS({ start: function(c) { assign(c); } }); };
            var a = mk(function(c) { cA = c; }), b = mk(function(c) { cB = c; });
            var pump = function() {
                return reader.read().then(function(step) {
                    if (step.done) { cA.close(); cB.close(); return; }
                    cA.enqueue(step.value); cB.enqueue(step.value);
                    return pump();
                }, function(e) { cA.error(e); cB.error(e); });
            };
            pump();
            return [a, b];
        };
        Object.defineProperty(RS.prototype, 'locked', {
            get: function() { return this.__locked; }, enumerable: true, configurable: true
        });
        // `for await (const chunk of res.body)` — the shorter spelling of the same pump loop.
        if (typeof Symbol !== 'undefined' && Symbol.asyncIterator) {
            RS.prototype[Symbol.asyncIterator] = function() {
                var reader = this.getReader();
                return {
                    next: function() { return reader.read(); },
                    "return": function(v) { reader.releaseLock(); return Promise.resolve({ value: v, done: true }); }
                };
            };
        }
        globalThis.ReadableStream = RS;

        var RSDR = function ReadableStreamDefaultReader(stream) { this.__s = stream; };
        RSDR.prototype.read = function() {
            var s = this.__s;
            if (!s) return Promise.reject(new TypeError("reader has been released"));
            if (s.__ondisturb) { s.__ondisturb(); }   // this is what flips Response.bodyUsed
            if (s.__err !== null)  return Promise.reject(s.__err);
            if (s.__q.length)      return Promise.resolve({ value: s.__q.shift(), done: false });
            if (s.__done)          return Promise.resolve({ value: undefined, done: true });
            return new Promise(function(resolve, reject) { s.__waiters.push({ resolve: resolve, reject: reject }); });
        };
        RSDR.prototype.releaseLock = function() { if (this.__s) { this.__s.__locked = false; this.__s = null; } };
        RSDR.prototype.cancel = function(reason) { return this.__s ? this.__s.cancel(reason) : Promise.resolve(undefined); };
        Object.defineProperty(RSDR.prototype, 'closed', {
            get: function() { return Promise.resolve(undefined); }, enumerable: true, configurable: true
        });
        globalThis.ReadableStreamDefaultReader = RSDR;
    }

    globalThis.__makeResponse = function(status, text, headers) {
        // `bodyUsed` is now HONEST — it flips when the body is actually consumed, by any of the
        // routes that consume it (a reader read, or text/json/arrayBuffer/blob/bytes).
        var used = false;
        var stream = null;
        var res = {
            ok: (status >= 200 && status < 300), status: status, statusText: "",
            url: "", redirected: false, type: "basic",
            headers: globalThis.__makeHeaders(headers),
            text: function(){ used = true; return Promise.resolve(text); },
            json: function(){ used = true;
                              try { return Promise.resolve(JSON.parse(text)); }
                              catch (e) { return Promise.reject(e); } },
            arrayBuffer: function(){ used = true; return Promise.resolve(globalThis.__bodyBytes(text).buffer); },
            bytes: function(){ used = true; return Promise.resolve(globalThis.__bodyBytes(text)); },
            blob: function(){ used = true; return Promise.resolve(new globalThis.Blob([text])); },
            clone: function(){ return globalThis.__makeResponse(status, text, headers); }
        };
        Object.defineProperty(res, 'bodyUsed', {
            get: function(){ return used; }, enumerable: true, configurable: true
        });
        // **Lazy, and one per response.** Constructing the stream eagerly would allocate a byte copy
        // for every response a page ever fetches, including the ones it only calls `.json()` on.
        Object.defineProperty(res, 'body', {
            get: function() {
                if (stream === null) {
                    var bytes = globalThis.__bodyBytes(text);
                    stream = new globalThis.ReadableStream({
                        start: function(c) { if (bytes.length) { c.enqueue(bytes); } c.close(); }
                    });
                    stream.__ondisturb = function() { used = true; };
                }
                return stream;
            },
            enumerable: true, configurable: true
        });
        return res;
    };

    // The body as bytes. `TextEncoder` is defined further down the prelude, which is fine — this only
    // runs when a page calls it, long after the whole prelude has been evaluated.
    globalThis.__bodyBytes = function(text) { return new globalThis.TextEncoder().encode(String(text)); };

    // ── INCREMENTAL delivery — the response arrives in pieces, as it does on the wire. ──────────
    // `__deliverFetch` settles a request with its WHOLE body at once, which is all the buffered host
    // path can do. The streaming path instead settles the promise at the HEADERS (which is when the
    // real `fetch()` resolves) and then feeds the body in as it arrives, so a page's pump loop sees
    // chunk after chunk and re-renders between them. That is the difference between an AI answer
    // that appears in one lump when the server is finished and one that types itself out.
    //
    // Bytes cross from Rust as one JS char per byte (the same convention the binary-upload path
    // uses) — see `js_bytes_literal`.
    globalThis.__streamCtl = {};
    globalThis.__bytesFromLatin1 = function(str) {
        var out = new Uint8Array(str.length);
        for (var i = 0; i < str.length; i++) { out[i] = str.charCodeAt(i) & 0xff; }
        return out;
    };

    globalThis.__makeStreamingResponse = function(status, headers) {
        var ctl = null;
        var stream = new globalThis.ReadableStream({ start: function(c) { ctl = c; } });
        // A buffered mirror, so `text()`/`json()` still work on a streamed response — but it is
        // DROPPED the moment the page takes a reader. Keeping both would mean an SSE stream that
        // never ends grows a copy of every token forever; a page that streams does not also buffer.
        var mirror = [];
        var ended = false, endWaiters = [], used = false;
        stream.__ondisturb = function() { used = true; mirror = null; };

        var whenEnded = function() {
            if (ended) return Promise.resolve();
            return new Promise(function(r) { endWaiters.push(r); });
        };
        var buffered = function() {
            return whenEnded().then(function() {
                if (mirror === null) throw new TypeError("Body has already been consumed.");
                var total = 0, i;
                for (i = 0; i < mirror.length; i++) { total += mirror[i].length; }
                var out = new Uint8Array(total), off = 0;
                for (i = 0; i < mirror.length; i++) { out.set(mirror[i], off); off += mirror[i].length; }
                return out;
            });
        };

        var res = {
            ok: (status >= 200 && status < 300), status: status, statusText: "",
            url: "", redirected: false, type: "basic",
            headers: globalThis.__makeHeaders(headers),
            text: function(){ used = true; return buffered().then(function(b){ return new globalThis.TextDecoder().decode(b); }); },
            json: function(){ return res.text().then(function(t){ return JSON.parse(t); }); },
            arrayBuffer: function(){ used = true; return buffered().then(function(b){ return b.buffer; }); },
            bytes: function(){ used = true; return buffered(); },
            blob: function(){ return res.text().then(function(t){ return new globalThis.Blob([t]); }); },
            // A streaming body cannot be cloned without teeing it; `body.tee()` is the honest way.
            clone: function(){ throw new TypeError("cannot clone a Response whose body is still streaming"); }
        };
        Object.defineProperty(res, 'bodyUsed', { get: function(){ return used; }, enumerable: true, configurable: true });
        Object.defineProperty(res, 'body',     { get: function(){ return stream; }, enumerable: true, configurable: true });

        return {
            res: res,
            push: function(bytes) { if (mirror !== null) { mirror.push(bytes); } ctl.enqueue(bytes); },
            end:  function() { ended = true; ctl.close(); var w = endWaiters; endWaiters = []; w.forEach(function(f){ f(); }); }
        };
    };

    // Headers are in: resolve the promise NOW, with a body that is still arriving.
    globalThis.__deliverHead = function(id, status, headers) {
        var cb = globalThis.__fetchCb[id];
        if (cb) {
            delete globalThis.__fetchCb[id];
            if (status === 0) { cb.reject(new TypeError("Failed to fetch")); return; }
            var s = globalThis.__makeStreamingResponse(status, headers);
            globalThis.__streamCtl[id] = s;
            cb.resolve(s.res);
            return;
        }
        // **XHR takes the same streaming path (tick 206).** `readyState 3` (LOADING) with a
        // growing `responseText` is how an XHR reports progress, and it is what every
        // upload/download progress bar and pre-fetch-era streaming client reads. Delivering the
        // whole body at once means `readyState` goes 1 → 4 and the page's progress handler never
        // runs — the download appears to take zero time and then be finished.
        var x = globalThis.__xhrObj[id];
        if (!x) return;                        // aborted, or already settled
        x._respHeaders = headers || [];
        x.status = status; x.statusText = "";
        x.responseText = ""; x.response = "";
        x._dec = new globalThis.TextDecoder();
        x.readyState = 2;                      // HEADERS_RECEIVED
        if (typeof x.onreadystatechange === 'function') { try { x.onreadystatechange(); } catch (e) {} }
    };
    globalThis.__deliverChunk = function(id, str) {
        var s = globalThis.__streamCtl[id];
        if (s) { s.push(globalThis.__bytesFromLatin1(str)); return; }
        var x = globalThis.__xhrObj[id];
        if (!x || x.readyState < 2) return;
        // `{stream:true}` — a chunk boundary can split a multi-byte character.
        x.responseText += x._dec.decode(globalThis.__bytesFromLatin1(str), { stream: true });
        if (x.responseType !== "json") { x.response = x.responseText; }
        x.readyState = 3;                      // LOADING — more is coming
        if (typeof x.onreadystatechange === 'function') { try { x.onreadystatechange(); } catch (e) {} }
        if (typeof x.onprogress === 'function') {
            try { x.onprogress({ type: 'progress', target: x, loaded: x.responseText.length, lengthComputable: false }); }
            catch (e) {}
        }
    };
    globalThis.__deliverEnd = function(id) {
        var s = globalThis.__streamCtl[id];
        if (s) { delete globalThis.__streamCtl[id]; s.end(); return; }
        var x = globalThis.__xhrObj[id];
        if (!x) return;
        delete globalThis.__xhrObj[id];
        if (x.responseType === "json") {
            try { x.response = JSON.parse(x.responseText); } catch (e) { x.response = null; }
        }
        x.readyState = 4;                      // DONE
        if (typeof x.onreadystatechange === 'function') { try { x.onreadystatechange(); } catch (e) {} }
        if (x.status === 0) {
            if (typeof x.onerror === 'function') { try { x.onerror(new Error("network")); } catch (e) {} }
        } else if (typeof x.onload === 'function') { try { x.onload(); } catch (e) {} }
        if (typeof x.onloadend === 'function') {
            try { x.onloadend({ type: 'loadend', target: x }); } catch (e) {}
        }
    };

    // Flatten a request's headers to "name\x02value\x02name\x02value" for the host to replay onto
    // the wire. `Authorization`, a non-JSON `Content-Type`, `Accept`, `X-*` — every one of these was
    // silently dropped before, so an authenticated `fetch`/XHR reached the server as an anonymous one
    // and came back 401. Accepts the three shapes a page passes: a plain object, an array of
    // `[name, value]` pairs, and a `forEach(value, name)` Headers-like.
    globalThis.__encHeaders = function(h){
        if (!h) return "";
        var out = [];
        if (Array.isArray(h)) {
            for (var i = 0; i < h.length; i++) { var p = h[i]; if (p && p.length >= 2) { out.push(String(p[0])); out.push(String(p[1])); } }
        } else if (typeof h.forEach === 'function') {
            h.forEach(function(v, k){ out.push(String(k)); out.push(String(v)); });
        } else {
            for (var k in h) { if (Object.prototype.hasOwnProperty.call(h, k)) { out.push(String(k)); out.push(String(h[k])); } }
        }
        return out.join("\x02");
    };
    // A unique multipart boundary. Not security-sensitive (it only needs to not occur in the body),
    // so `Math.random` is fine; two draws make an accidental collision astronomically unlikely.
    globalThis.__multipartBoundary = function(){
        return "----manukFormBoundary" + Math.random().toString(36).slice(2) + Math.random().toString(36).slice(2);
    };
    // Return the `\x02`-encoded header list with `Content-Type` forced to `ctype` — any existing
    // Content-Type (case-insensitive) is dropped first, because for a multipart body only the browser
    // knows the boundary, so a page-set Content-Type must not survive.
    globalThis.__withContentType = function(hdrs, ctype){
        var parts = hdrs ? hdrs.split("\x02") : [];
        var out = [];
        for (var i = 0; i + 1 < parts.length; i += 2) {
            if (parts[i].toLowerCase() !== "content-type") { out.push(parts[i], parts[i + 1]); }
        }
        out.push("Content-Type", ctype);
        return out.join("\x02");
    };
    // fetch(url[, {method, headers, body, signal}]) -> Promise<Response-like>.
    // **`signal` is honoured** — every modern `fetch` passes an `AbortController.signal`, and React's
    // `useEffect` cleanup (and StrictMode's double-mount) *depends* on the abort actually cancelling
    // the request. An already-aborted signal rejects synchronously without ever queuing the request; an
    // abort that fires while the request is in flight rejects the Promise with the signal's reason (a
    // DOMException named 'AbortError') and drops the pending callback so a late host delivery is a
    // no-op (it cannot resolve an already-rejected fetch).
    globalThis.fetch = function(url, opts) {
        var signal = opts && opts.signal;
        if (signal && signal.aborted) {
            return Promise.reject(signal.reason !== undefined ? signal.reason
                : new DOMException('signal is aborted without reason', 'AbortError'));
        }
        var id = ++__fetchId;
        var method = (opts && opts.method) || "GET";
        var hdrs = (opts && opts.headers) ? __encHeaders(opts.headers) : "";
        var body;
        if (opts && opts.body && opts.body.__isFormData) {
            // A FormData body is multipart/form-data — the browser generates the boundary (the page
            // cannot know it), so any page-set Content-Type is REPLACED with the multipart one.
            var boundary = __multipartBoundary();
            body = opts.body.__multipart(boundary);
            hdrs = __withContentType(hdrs, "multipart/form-data; boundary=" + boundary);
        } else {
            body = (opts && opts.body != null) ? String(opts.body) : "";
        }
        __pendingFetches.push(id + "\x01f\x01" + method + "\x01" + url + "\x01" + hdrs + "\x01" + body);
        return new Promise(function(resolve, reject){
            __fetchCb[id] = { resolve: resolve, reject: reject };
            if (signal) {
                signal.addEventListener('abort', function(){
                    if (!__fetchCb[id]) { return; }   // already settled — do not double-reject
                    delete __fetchCb[id];             // a later __deliverFetch(id, …) now finds no callback
                    reject(signal.reason !== undefined ? signal.reason
                        : new DOMException('signal is aborted without reason', 'AbortError'));
                });
            }
        });
    };
    globalThis.__deliverFetch = function(id, status, text, headers) {
        var cb = __fetchCb[id]; if (!cb) return; delete __fetchCb[id];
        if (status === 0) { cb.reject(new TypeError("Failed to fetch")); return; }
        cb.resolve(globalThis.__makeResponse(status, text, headers));
    };

    globalThis.XMLHttpRequest = function() {
        this.readyState = 0; this.status = 0; this.statusText = "";
        this.responseText = ""; this.response = ""; this.responseType = "";
        this.onload = null; this.onerror = null; this.onreadystatechange = null;
        this.onabort = null; this.onloadend = null;
        this._m = "GET"; this._u = ""; this._id = null; this._h = []; this._respHeaders = [];
    };
    XMLHttpRequest.prototype.open = function(m, u) { this._m = m || "GET"; this._u = u || ""; this._h = []; this.readyState = 1; };
    XMLHttpRequest.prototype.setRequestHeader = function(k, v) { if (k != null) this._h.push([k, v == null ? "" : v]); };
    XMLHttpRequest.prototype.getAllResponseHeaders = function() {
        var h = this._respHeaders; if (!h || !h.length) return "";
        // Spec: one `name: value` field per line, CRLF-terminated, header names lower-cased.
        var out = "";
        for (var i = 0; i < h.length; i++) { out += String(h[i][0]).toLowerCase() + ": " + String(h[i][1]) + "\r\n"; }
        return out;
    };
    XMLHttpRequest.prototype.getResponseHeader = function(name) {
        var h = this._respHeaders; if (!h) return null;
        name = String(name).toLowerCase(); var vals = [];
        for (var i = 0; i < h.length; i++) { if (String(h[i][0]).toLowerCase() === name) vals.push(h[i][1]); }
        return vals.length ? vals.join(", ") : null;
    };
    // Cancel an in-flight request. This was a **no-op**, so an aborted XHR still fired `onload` with
    // the full response when the host delivered it — a search-as-you-type box that fires a request per
    // keystroke and `abort()`s the stale one would apply the OLD response over the new (the classic
    // race), and any request library's cancel path did nothing. Now abort drops the pending callback
    // (a late `__deliverXhr` for this id finds no object and is a no-op — the response cannot resolve a
    // cancelled request) and fires `readystatechange` → `abort` → `loadend`, per the XHR standard.
    XMLHttpRequest.prototype.abort = function() {
        if (this._id != null) { delete __xhrObj[this._id]; }
        this.status = 0; this.statusText = ""; this.responseText = ""; this.response = "";
        this._respHeaders = [];
        this.readyState = 4; // DONE while the events fire...
        if (typeof this.onreadystatechange === 'function') { try { this.onreadystatechange(); } catch (e) {} }
        var ev = { type: 'abort', target: this };
        if (typeof this.onabort === 'function') { try { this.onabort(ev); } catch (e) {} }
        if (typeof this.onloadend === 'function') { try { this.onloadend({ type: 'loadend', target: this }); } catch (e) {} }
        this.readyState = 0; // ...then back to UNSENT (XHR standard's abort() steps).
    };
    XMLHttpRequest.prototype.send = function(body) {
        var id = ++__fetchId; __xhrObj[id] = this; this._id = id;
        var hdrs = __encHeaders(this._h);
        var payload;
        if (body && body.__isFormData) {
            // Same as fetch: a FormData body is multipart/form-data with a browser-generated boundary,
            // so a File part is actually uploaded instead of stringified to "[object File]".
            var boundary = __multipartBoundary();
            payload = body.__multipart(boundary);
            hdrs = __withContentType(hdrs, "multipart/form-data; boundary=" + boundary);
        } else {
            payload = (body != null ? String(body) : "");
        }
        __pendingFetches.push(id + "\x01x\x01" + this._m + "\x01" + this._u + "\x01" + hdrs + "\x01" + payload);
    };
    globalThis.__deliverXhr = function(id, status, text, headers) {
        var x = __xhrObj[id]; if (!x) return; delete __xhrObj[id];
        x._respHeaders = headers || [];
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
    globalThis.__deliver = function(id, status, text, headers) {
        if (__fetchCb[id]) { globalThis.__deliverFetch(id, status, text, headers); return; }
        if (__xhrObj[id]) { globalThis.__deliverXhr(id, status, text, headers); return; }
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
      // The REAL prototypes, built in Rust (see dom_bindings::dom_protos). The chain is
      //   instance -> HTMLElement.prototype -> Element.prototype -> Node.prototype -> EventTarget.prototype
      // and every DOM method is an own-property of Node.prototype — not of the element, as it used to be.
      var REAL = {
        EventTarget:      globalThis.__protoEventTarget,
        Node:             globalThis.__protoNode,
        Element:          globalThis.__protoElement,
        HTMLElement:      globalThis.__protoHTMLElement,
        Document:         globalThis.__protoDocument,
        Text:             globalThis.__protoNode,
        Comment:          globalThis.__protoNode,
        DocumentFragment: globalThis.__protoNode,
      };

      function iface(name, test) {
        var C = globalThis[name];
        if (typeof C !== 'function') {
          // Constructible and inert — NOT throwing. A base class that throws on `super()` is
          // indistinguishable, from the author's side, from a browser that does not support classes.
          C = function(){ return this; };
          Object.defineProperty(C, 'name', { value: name });
          // Point the constructor at the REAL prototype where one exists, instead of a fresh `{}`.
          // A fake `{}` is why `Element.prototype.setAttribute` was `undefined` for sixty ticks, and why
          // patching a DOM prototype silently did nothing — the fake was not in any instance's chain.
          C.prototype = REAL[name] || {};
          globalThis[name] = C;
        }
        try { Object.defineProperty(C, Symbol.hasInstance, { value: test, configurable: true }); }
        catch (e) { /* a frozen builtin: its own instanceof is already right */ }
        return C;
      }
      var isEl   = function(o){ return !!o && o.nodeType === 1; };
      var isNode = function(o){ return !!o && typeof o.nodeType === 'number'; };
      var tagIs  = function(t){ return function(o){ return isEl(o) && o.tagName === t; }; };

      // ── The prototype accessor bridge — what Svelte 5 needs, and what nothing else asks for.
      //
      // Our reflectors carry their DOM members as OWN properties: `define_members` installs them on
      // each object, and there is no shared prototype chain. That is fine for every framework that
      // simply *uses* the DOM — `node.firstChild` finds the own accessor and works.
      //
      // Svelte 5 does not use the DOM that way. For speed it reaches past the instance and lifts the
      // raw accessor functions straight off the interface prototypes, once, at startup:
      //
      //     first_child_getter = get_descriptor(Node.prototype, 'firstChild').get;
      //     next_sibling_getter = get_descriptor(Node.prototype, 'nextSibling').get;
      //
      // and then calls them as `first_child_getter.call(node)` on every node it walks. With an empty
      // `Node.prototype`, `get_descriptor(...)` is `undefined` and `.get` throws:
      //
      //     TypeError: can't access property "get", a(...) is undefined
      //
      // — thrown inside an async mount, so it surfaced only as an unhandled promise rejection, and
      // `#app` stayed empty with nothing to point at.
      //
      // So: put real accessor descriptors on the prototypes. Each one looks up the OWN descriptor of
      // whatever `this` it was called with and delegates. It reads the own descriptor rather than the
      // property, which is what keeps this from recursing infinitely when `this` has no own accessor.
      // The prototypes are not in our reflectors' chain, so these exist purely to be *lifted* — which
      // is precisely the use Svelte makes of them.
      function bridge(proto, names) {
        names.forEach(function(name) {
          if (Object.getOwnPropertyDescriptor(proto, name)) return;
          Object.defineProperty(proto, name, {
            configurable: true,
            get: function() {
              var d = Object.getOwnPropertyDescriptor(this, name);
              return d && d.get ? d.get.call(this) : (d ? d.value : undefined);
            },
            set: function(v) {
              var d = Object.getOwnPropertyDescriptor(this, name);
              if (d && d.set) { d.set.call(this, v); }
              else { Object.defineProperty(this, name, { value: v, writable: true, configurable: true }); }
            }
          });
        });
      }
      var NODE_ACCESSORS = ['firstChild','lastChild','nextSibling','previousSibling','parentNode',
                            'parentElement','childNodes','textContent','nodeValue','nodeType',
                            'nodeName','ownerDocument','isConnected'];
      var EL_ACCESSORS   = ['className','id','innerHTML','outerHTML','innerText','children',
                            'firstElementChild','lastElementChild','nextElementSibling',
                            'previousElementSibling','tagName','attributes'];
      var CD_ACCESSORS   = ['data','nodeValue','textContent'];

      iface('EventTarget', function(o){ return isNode(o) || o === globalThis; });
      iface('Node', isNode);
      iface('Element', isEl);
      iface('HTMLElement', isEl);
      iface('SVGElement', function(o){ return isEl(o) && o.tagName === 'SVG'; });
      iface('Text', function(o){ return !!o && o.nodeType === 3; });

      iface('Comment', function(o){ return !!o && o.nodeType === 8; });
      // CharacterData is the abstract base of Text (3), Comment (8), ProcessingInstruction (7) and
      // CDATASection (4) — `dom/nodes/Document-create{TextNode,Comment}` assert `c instanceof
      // CharacterData` FIRST, so its absence (a ReferenceError) aborted every one of those subtests.
      iface('CharacterData', function(o){ return !!o && (o.nodeType === 3 || o.nodeType === 8 || o.nodeType === 7 || o.nodeType === 4); });
      iface('DocumentFragment', function(o){ return !!o && o.nodeType === 11; });

      // ── Text, Comment and DocumentFragment are CONSTRUCTABLE (`new Text('x')`, `new Comment('x')`,
      //    `new DocumentFragment()`), not merely `instanceof` targets. `iface()` above gives every
      //    interface an *inert* constructor that returns an empty `{}` — right for the un-constructable
      //    ones (Element, Node) but wrong for these three, where the spec mints a real detached node
      //    owned by the current document. `new Text('x')` was returning `{data: undefined, nodeType:
      //    undefined}`, so every test (and every library) that builds a node with `new Text()` got a
      //    dead object. Delegating to the existing `document.create*` factories (evaluated at call time,
      //    when `document` is fully wired) makes them real; `hasInstance` from `iface` still answers
      //    `instanceof` via the nodeType predicate, so the flat-prototype node still tests true.
      (function () {
        var mkCtor = function (name, build, test) {
          var C = function (arg) { return build(arg); };
          try { Object.defineProperty(C, 'name', { value: name }); } catch (e) {}
          C.prototype = (typeof REAL !== 'undefined' && REAL[name]) ||
                        (globalThis[name] && globalThis[name].prototype) || {};
          try { Object.defineProperty(C, Symbol.hasInstance, { value: test, configurable: true }); }
          catch (e) {}
          globalThis[name] = C;
        };
        mkCtor('Text',
          function (d) { return globalThis.document.createTextNode(d === undefined ? '' : String(d)); },
          function (o) { return !!o && o.nodeType === 3; });
        mkCtor('Comment',
          function (d) { return globalThis.document.createComment(d === undefined ? '' : String(d)); },
          function (o) { return !!o && o.nodeType === 8; });
        mkCtor('DocumentFragment',
          function () { return globalThis.document.createDocumentFragment(); },
          function (o) { return !!o && o.nodeType === 11; });
      })();

      // ── The Node interface CONSTANTS + compareDocumentPosition. Ordinary code writes
      //    `n.nodeType === Node.ELEMENT_NODE` and libraries order the DOM with
      //    `a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING`. Absent, the first is
      //    `=== undefined` (silently false) and the second throws.
      (function(){
        var NC = {
          ELEMENT_NODE:1, ATTRIBUTE_NODE:2, TEXT_NODE:3, CDATA_SECTION_NODE:4,
          ENTITY_REFERENCE_NODE:5, ENTITY_NODE:6, PROCESSING_INSTRUCTION_NODE:7, COMMENT_NODE:8,
          DOCUMENT_NODE:9, DOCUMENT_TYPE_NODE:10, DOCUMENT_FRAGMENT_NODE:11, NOTATION_NODE:12,
          DOCUMENT_POSITION_DISCONNECTED:1, DOCUMENT_POSITION_PRECEDING:2, DOCUMENT_POSITION_FOLLOWING:4,
          DOCUMENT_POSITION_CONTAINS:8, DOCUMENT_POSITION_CONTAINED_BY:16,
          DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC:32
        };
        var N = globalThis.Node;
        for (var k in NC) {
          try { Object.defineProperty(N, k, { value: NC[k], enumerable: true }); } catch (e) {}
          try { Object.defineProperty(N.prototype, k, { value: NC[k], enumerable: true }); } catch (e) {}
        }
        if (typeof N.prototype.compareDocumentPosition !== 'function') {
          N.prototype.compareDocumentPosition = function(other){
            if (other === this) return 0;
            var chain = function(n){ var c=[]; while(n){ c.push(n); n=n.parentNode; } return c; };
            var ca = chain(this), cb = chain(other);
            if (ca[ca.length-1] !== cb[cb.length-1]) {           // different roots → disconnected
              return NC.DOCUMENT_POSITION_DISCONNECTED | NC.DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC |
                     NC.DOCUMENT_POSITION_PRECEDING;
            }
            if (cb.indexOf(this) >= 0)                            // this is an ancestor of other
              return NC.DOCUMENT_POSITION_CONTAINED_BY | NC.DOCUMENT_POSITION_FOLLOWING;
            if (ca.indexOf(other) >= 0)                          // other is an ancestor of this
              return NC.DOCUMENT_POSITION_CONTAINS | NC.DOCUMENT_POSITION_PRECEDING;
            ca.reverse(); cb.reverse();                          // now [root ... node]
            var i=0; while(i<ca.length && i<cb.length && ca[i]===cb[i]) i++;
            var parent = ca[i-1], childA = ca[i], childB = cb[i];
            var kids = parent.childNodes;
            for (var k=0;k<kids.length;k++){
              if (kids[k]===childA) return NC.DOCUMENT_POSITION_FOLLOWING;  // this earlier → other follows
              if (kids[k]===childB) return NC.DOCUMENT_POSITION_PRECEDING;  // other earlier
            }
            return NC.DOCUMENT_POSITION_DISCONNECTED;
          };
        }
      })();

      // ── HTMLMediaElement — **an honest NO, not a TypeError.**
      //
      // We cannot decode video or audio. That is a real limit and it is not going away this tick. What
      // is NOT acceptable is the shape the limit currently takes: `video.play` is `undefined`, so a
      // site that calls it throws and takes the whole page down, and a site that *politely feature-
      // detects* with `if (v.canPlayType('video/mp4'))` reads `undefined` and cannot even be told no.
      //
      // Graceful degradation is not doing nothing. It is **answering the question honestly**, and the
      // spec already has the exact vocabulary for a browser that cannot play a thing:
      //
      //   canPlayType(t)  →  ''   the empty string IS the spec's "no"
      //   play()          →  a REJECTED promise (NotSupportedError). Every player library on the web
      //                      already handles this path, because autoplay policies make rejection
      //                      routine in real browsers — so this is the best-tested failure mode there is.
      //   error           →  MediaError { code: 4 }  (MEDIA_ERR_SRC_NOT_SUPPORTED)
      //   readyState      →  0 (HAVE_NOTHING)   networkState → 3 (NETWORK_NO_SOURCE)
      //
      // A site told *that* hides its player and shows its fallback. A site told `undefined` throws.
      // The poster still renders (see `fetch_images_owned`), so what the user sees is the frame the
      // author chose — which is exactly what a real browser shows before you press play.
      // ── **`<canvas>.getContext()` — it THREW, and a throw takes the page with it.**
      //
      // Only ~3% of the corpus uses `<canvas>`, which is why this is not a rasterizer yet. But
      // `getContext` was **undefined**, so `ctx.fillRect(...)` on the next line was a `TypeError`, and a
      // charting library that initialises at boot took the whole bundle down with it. **3% of sites
      // using a feature is 3% of sites BROKEN when that feature throws** — the usage number and the
      // damage number are not the same number.
      //
      // So: a real 2D context object, with the real surface, whose drawing operations are no-ops. A
      // blank chart on a working page. That is the same trade as `<video>` (poster + an honest "cannot
      // play") and it is the right one: legible beats absent, and absent beats an exception.
      //
      // `getContext('webgl')` returns **null**, which is the spec's way of saying "this browser cannot
      // give you that context" — every library on the web already branches on it, because that is what
      // a machine without a GPU returns.
      //
      // Rasterizing this for real is genuinely within reach — `tiny-skia` already backs our painter and
      // has paths, fills and strokes — and it is on the list. It is not on TODAY's list, because
      // `localStorage` is 27% and forms are 50%.
      globalThis.__manukCanvas = function(el) {
        if (!el || el.__canvasReady) { return el; }
        Object.defineProperty(el, '__canvasReady', { value: true, enumerable: false });

        // ── `canvas.width` / `canvas.height` — IDL attributes that REFLECT the content attributes,
        // defaulting to 300x150.
        //
        // They were simply absent, so `el.width` read `undefined` and the backing store fell back to
        // 300x150 for every canvas on the web. The drawing was then correct *inside a 300x150 surface*,
        // which the painter dutifully scaled down into the element's real box — so a chart drawn at its
        // true size came out as a smudge in the corner, and everything else was transparent. **The
        // pixels were right and the surface was wrong**, which is a far more confusing bug than a blank
        // canvas, because `getImageData` agrees with you the whole way.
        //
        // Assigning either one also RESIZES and CLEARS the surface — that is the spec, and it is the
        // idiomatic way to erase a canvas, so a chart library that re-renders on resize depends on it.
        ['width', 'height'].forEach(function (dim) {
          var dflt = dim === 'width' ? 300 : 150;
          Object.defineProperty(el, dim, {
            configurable: true,
            get: function () {
              var v = parseInt(el.getAttribute(dim), 10);
              return (isNaN(v) || v < 0) ? dflt : v;
            },
            set: function (v) {
              v = parseInt(v, 10);
              el.setAttribute(dim, String((isNaN(v) || v < 0) ? dflt : v));
              el.__cvInit(el.width, el.height);
            },
          });
        });

        // ── COLOUR. Everything the spec calls a "CSS color" that a chart library actually emits.
        function color(v) {
          if (v && v.__grad) { return v.__stops.length ? v.__stops[v.__stops.length-1] : [0,0,0,1]; }
          var s = String(v == null ? '#000' : v).trim().toLowerCase();
          var NAMED = { black:[0,0,0], white:[255,255,255], red:[255,0,0], green:[0,128,0],
                        blue:[0,0,255], gray:[128,128,128], grey:[128,128,128], silver:[192,192,192],
                        yellow:[255,255,0], orange:[255,165,0], purple:[128,0,128], transparent:[0,0,0,0] };
          if (NAMED[s]) { var n = NAMED[s]; return [n[0], n[1], n[2], n.length > 3 ? n[3] : 1]; }
          var m;
          if ((m = /^#([0-9a-f]{3})$/.exec(s))) {
            return [parseInt(m[1][0]+m[1][0],16), parseInt(m[1][1]+m[1][1],16), parseInt(m[1][2]+m[1][2],16), 1];
          }
          if ((m = /^#([0-9a-f]{6})$/.exec(s))) {
            return [parseInt(s.slice(1,3),16), parseInt(s.slice(3,5),16), parseInt(s.slice(5,7),16), 1];
          }
          if ((m = /^#([0-9a-f]{8})$/.exec(s))) {
            return [parseInt(s.slice(1,3),16), parseInt(s.slice(3,5),16), parseInt(s.slice(5,7),16),
                    parseInt(s.slice(7,9),16)/255];
          }
          if ((m = /^rgba?\(([^)]+)\)$/.exec(s))) {
            var p = m[1].split(/[,\s\/]+/).filter(function(x){ return x.length; });
            var c = function(i){ var t = p[i] || '0';
              return t.indexOf('%') >= 0 ? Math.round(parseFloat(t) * 2.55) : parseInt(t, 10) || 0; };
            return [c(0), c(1), c(2), p.length > 3 ? parseFloat(p[3]) : 1];
          }
          return [0, 0, 0, 1];   // unparseable: black, exactly as a browser does
        }

        // ── TRANSFORM. A 2x3 matrix stack, multiplied in JS. Rust receives it already resolved.
        function mul(a, b) {
          return [a[0]*b[0] + a[2]*b[1],       a[1]*b[0] + a[3]*b[1],
                  a[0]*b[2] + a[2]*b[3],       a[1]*b[2] + a[3]*b[3],
                  a[0]*b[4] + a[2]*b[5] + a[4], a[1]*b[4] + a[3]*b[5] + a[5]];
        }

        el.getContext = function(kind) {
          kind = String(kind || '2d').toLowerCase();
          if (kind !== '2d') {
            // The spec's "cannot": every library already branches on a null WebGL context, because that
            // is what a machine without a GPU returns. We have no GPU tier here yet.
            return null;
          }
          if (el.__ctx) { return el.__ctx; }
          el.__cvInit(el.width || 300, el.height || 150);

          var ctx = {
            canvas: el,
            fillStyle: '#000', strokeStyle: '#000', lineWidth: 1, globalAlpha: 1,
            lineCap: 'butt', lineJoin: 'miter', miterLimit: 10, lineDashOffset: 0,
            font: '10px sans-serif', textAlign: 'start', textBaseline: 'alphabetic', direction: 'inherit',
            globalCompositeOperation: 'source-over', imageSmoothingEnabled: true,
            shadowBlur: 0, shadowColor: 'rgba(0,0,0,0)', shadowOffsetX: 0, shadowOffsetY: 0, filter: 'none',
          };
          var M = [1, 0, 0, 1, 0, 0];      // current transform
          var STACK = [];                  // save()/restore()
          var P = [];                      // the current path, as the flat command stream Rust reads

          function rgba(style) {
            var c = color(style);
            return [c[0], c[1], c[2], (c.length > 3 ? c[3] : 1) * (ctx.globalAlpha == null ? 1 : ctx.globalAlpha)];
          }

          // ── State
          ctx.save = function(){ STACK.push({ m: M.slice(), fs: ctx.fillStyle, ss: ctx.strokeStyle,
                                              lw: ctx.lineWidth, ga: ctx.globalAlpha }); };
          ctx.restore = function(){ var s = STACK.pop(); if (!s) return;
                                    M = s.m; ctx.fillStyle = s.fs; ctx.strokeStyle = s.ss;
                                    ctx.lineWidth = s.lw; ctx.globalAlpha = s.ga; };
          ctx.translate = function(x, y){ M = mul(M, [1, 0, 0, 1, +x || 0, +y || 0]); };
          ctx.scale     = function(x, y){ M = mul(M, [+x || 0, 0, 0, +y || 0, 0, 0]); };
          ctx.rotate    = function(a){ var c = Math.cos(+a || 0), s = Math.sin(+a || 0);
                                       M = mul(M, [c, s, -s, c, 0, 0]); };
          ctx.transform = function(a,b,c,d,e,f){ M = mul(M, [+a||0, +b||0, +c||0, +d||0, +e||0, +f||0]); };
          ctx.setTransform = function(a,b,c,d,e,f){
            M = (a && typeof a === 'object')
              ? [a.a||1, a.b||0, a.c||0, a.d||1, a.e||0, a.f||0]
              : [+a||0, +b||0, +c||0, +d||0, +e||0, +f||0];
          };
          ctx.resetTransform = function(){ M = [1, 0, 0, 1, 0, 0]; };
          ctx.getTransform = function(){ return { a:M[0], b:M[1], c:M[2], d:M[3], e:M[4], f:M[5] }; };

          // ── Rects
          ctx.fillRect = function(x, y, w, h){
            var c = rgba(ctx.fillStyle);
            el.__cvRect(+x||0, +y||0, +w||0, +h||0, c[0], c[1], c[2], c[3], 0, M);
          };
          ctx.strokeRect = function(x, y, w, h){
            var c = rgba(ctx.strokeStyle);
            el.__cvRect(+x||0, +y||0, +w||0, +h||0, c[0], c[1], c[2], c[3],
                        Math.max(+ctx.lineWidth || 1, 0.01), M);
          };
          ctx.clearRect = function(x, y, w, h){ el.__cvClear(+x||0, +y||0, +w||0, +h||0, M); };

          // ── Paths. Accumulated as [op, args...] and rasterized in ONE native call, because a chart
          // with 10,000 points must not pay 10,000 FFI crossings.
          ctx.beginPath = function(){ P = []; };
          ctx.closePath = function(){ P.push(4); };
          ctx.moveTo = function(x, y){ P.push(0, +x||0, +y||0); };
          ctx.lineTo = function(x, y){ P.push(1, +x||0, +y||0); };
          ctx.quadraticCurveTo = function(cx, cy, x, y){ P.push(2, +cx||0, +cy||0, +x||0, +y||0); };
          ctx.bezierCurveTo = function(a,b,c,d,e,f){ P.push(3, +a||0,+b||0,+c||0,+d||0,+e||0,+f||0); };
          ctx.rect = function(x, y, w, h){ P.push(5, +x||0, +y||0, +w||0, +h||0); };
          ctx.roundRect = function(x, y, w, h){ P.push(5, +x||0, +y||0, +w||0, +h||0); };
          // `arc` is flattened to cubics here rather than added as a Rust op: it keeps the command
          // stream to six primitives, and the error at this segment count is well under a pixel.
          ctx.arc = function(cx, cy, r, a0, a1, ccw){
            cx = +cx||0; cy = +cy||0; r = +r||0; a0 = +a0||0; a1 = +a1||0;
            var span = a1 - a0;
            if (ccw) { if (span > 0) span -= 2*Math.PI; } else { if (span < 0) span += 2*Math.PI; }
            var n = Math.max(2, Math.ceil(Math.abs(span) / (Math.PI/8)));
            for (var i = 0; i <= n; i++) {
              var t = a0 + span * (i/n), x = cx + r*Math.cos(t), y = cy + r*Math.sin(t);
              P.push(i === 0 ? 0 : 1, x, y);
            }
          };
          ctx.arcTo = function(x1, y1, x2, y2){ ctx.lineTo(x1, y1); ctx.lineTo(x2, y2); };
          ctx.ellipse = function(cx, cy, rx, ry, rot, a0, a1, ccw){ ctx.arc(cx, cy, Math.max(+rx||0, +ry||0), a0, a1, ccw); };
          ctx.fill = function(){
            var c = rgba(ctx.fillStyle);
            el.__cvPath(P, true, c[0], c[1], c[2], c[3], 0, M);
          };
          ctx.stroke = function(){
            var c = rgba(ctx.strokeStyle);
            el.__cvPath(P, false, c[0], c[1], c[2], c[3], Math.max(+ctx.lineWidth || 1, 0.01), M);
          };
          ctx.clip = function(){};                      // honest no-op: clipping is not wired yet
          ctx.isPointInPath = function(){ return false; };

          // ── Text. Rasterized through the SAME swash pipeline as DOM text (see `canvas.rs`).
          //
          // The parsing and the arithmetic live here; only a resolved pen origin, colour, size and
          // family list cross into Rust — the division of labour every other op in this file uses.

          // The `ctx.font` CSS shorthand. Order is [style] [variant] [weight] [stretch] size[/lh] family,
          // and the family is everything after the size — which is what makes the size token the anchor
          // to parse around rather than trying to tokenize left-to-right.
          var parseFont = function(spec) {
            var s = String(spec == null ? '' : spec).trim();
            var bold = false, italic = false, size = 10, family = 'sans-serif';
            // The size token: the first length, optionally followed by /line-height (which canvas
            // ignores — there is no line box here to apply it to).
            var m = s.match(/(^|\s)(-?[\d.]+)(px|pt|pc|in|cm|mm|em|rem|%)(\s*\/\s*[^\s]+)?\s+(.+)$/);
            if (m) {
              var n = parseFloat(m[2]);
              var unit = m[3];
              // Absolute units only; em/rem/% have no element context inside a canvas, so they resolve
              // against the 16px initial font size rather than silently becoming pixels.
              var mul = unit === 'pt' ? (96/72) : unit === 'pc' ? 16 : unit === 'in' ? 96 :
                        unit === 'cm' ? (96/2.54) : unit === 'mm' ? (96/25.4) :
                        (unit === 'em' || unit === 'rem') ? 16 : unit === '%' ? 0.16 : 1;
              if (n === n && n > 0) { size = n * mul; }
              family = m[5].trim();
              var pre = s.slice(0, m.index).toLowerCase();
              italic = /(^|\s)(italic|oblique)(\s|$)/.test(pre);
              // 'bold', 'bolder', or a numeric weight >= 600 — sites write all three.
              bold = /(^|\s)(bold|bolder)(\s|$)/.test(pre) || /(^|\s)([6-9]\d\d|1000)(\s|$)/.test(pre);
            }
            return { size: size, family: family, bold: bold, italic: italic };
          };

          var measureRaw = function(t, f) {
            var r = el.__cvMeasureText(String(t == null ? '' : t), f.size, f.family, f.bold, f.italic);
            return r || [0, 0, 0];
          };

          // textAlign/textBaseline are pen-origin offsets, and they are the difference between a
          // centred chart label and one starting at the centre. 'start'/'end' follow `direction`.
          var alignDx = function(w, rtl) {
            var a = ctx.textAlign;
            if (a === 'center') { return -w / 2; }
            if (a === 'right')  { return -w; }
            if (a === 'left')   { return 0; }
            if (a === 'end')    { return rtl ? 0 : -w; }
            return rtl ? -w : 0;                      // 'start' (the initial value)
          };
          var baselineDy = function(asc, desc) {
            var b = ctx.textBaseline;
            if (b === 'top' || b === 'hanging') { return asc; }
            if (b === 'middle') { return (asc - desc) / 2; }
            if (b === 'bottom') { return -desc; }
            return 0;                                  // 'alphabetic' / 'ideographic'
          };

          var drawText = function(t, x, y, maxWidth, style) {
            t = String(t == null ? '' : t);
            if (!t) { return; }
            var f = parseFont(ctx.font);
            var rtl = ctx.direction === 'rtl';
            var mt = measureRaw(t, f);
            var w = mt[0];
            // maxWidth is condensed inside Rust (it must be applied to the SHAPED width), so the
            // alignment offset uses the width the text will actually occupy.
            if (maxWidth > 0 && w > maxWidth) { w = maxWidth; }
            var c = color(style);
            el.__cvText(t, x + alignDx(w, rtl), y + baselineDy(mt[1], mt[2]),
                        c[0], c[1], c[2], c[3] * ctx.globalAlpha,
                        f.size, f.family, f.bold, f.italic, rtl,
                        (maxWidth > 0 ? maxWidth : 0), M);
          };

          ctx.fillText = function(t, x, y, maxWidth){
            drawText(t, +x, +y, arguments.length > 3 ? +maxWidth : 0, ctx.fillStyle);
          };
          // strokeText renders FILLED in the stroke colour. An outline-only glyph needs the outline
          // path rather than the coverage bitmap, which the raster does not hand back; filling is the
          // bounded approximation, and it is much closer to right than drawing nothing (the idiom is
          // stroke-behind-fill for text over imagery, where absence is a hole).
          ctx.strokeText = function(t, x, y, maxWidth){
            drawText(t, +x, +y, arguments.length > 3 ? +maxWidth : 0, ctx.strokeStyle);
          };
          ctx.measureText = function(t){
            var f = parseFont(ctx.font);
            var mt = measureRaw(t, f);
            var w = mt[0], asc = mt[1], desc = mt[2];
            var rtl = ctx.direction === 'rtl';
            var dx = alignDx(w, rtl), dy = baselineDy(asc, desc);
            // The `actualBoundingBox*` values are reported relative to the alignment point, which is
            // what a page uses them for (hit-testing a label it just drew, sizing a background pill).
            return { width: w,
                     actualBoundingBoxLeft: -dx, actualBoundingBoxRight: w + dx,
                     actualBoundingBoxAscent: asc + dy, actualBoundingBoxDescent: desc - dy,
                     fontBoundingBoxAscent: asc, fontBoundingBoxDescent: desc,
                     emHeightAscent: asc, emHeightDescent: desc,
                     alphabeticBaseline: 0, hangingBaseline: asc, ideographicBaseline: -desc };
          };

          // ── Pixels. Real ones.
          ctx.getImageData = function(x, y, w, h){
            w = Math.max(0, w|0); h = Math.max(0, h|0);
            var raw = el.__cvGetImageData(x|0, y|0, w, h) || [];
            return { width: w, height: h, colorSpace: 'srgb', data: new Uint8ClampedArray(raw) };
          };
          ctx.createImageData = function(w, h){
            w = Math.max(0, w|0); h = Math.max(0, h|0);
            return { width: w, height: h, colorSpace: 'srgb', data: new Uint8ClampedArray(w*h*4) };
          };
          ctx.putImageData = function(){};              // honest no-op
          ctx.drawImage = function(){};                 // honest no-op: no image source plumbing yet

          // ── Gradients: a real object, and the last stop's colour is used as a flat approximation.
          // A bar drawn in the gradient's end colour beats a bar that is not drawn.
          function grad() {
            var g = { __grad: true, __stops: [] };
            g.addColorStop = function(_o, c){ g.__stops.push(color(c)); };
            return g;
          }
          ctx.createLinearGradient = grad;
          ctx.createRadialGradient = grad;
          ctx.createConicGradient = grad;
          ctx.createPattern = function(){ return null; };
          ctx.setLineDash = function(){};
          ctx.getLineDash = function(){ return []; };

          Object.defineProperty(el, '__ctx', { value: ctx, enumerable: false });
          return ctx;
        };

        el.toDataURL = function(){ return el.__cvToDataURL(); };
        el.toBlob = function(cb){ if (typeof cb === 'function') { cb(null); } };
        return el;
      };

      // (No `localStorage` shim here. A REAL one already exists — `manuk_net::webstorage`, persisted
      // and per-origin, behind the `__storage` native seam in `dom_bindings`. I nearly wrote a worse
      // duplicate on the strength of a capability probe that reported `localStorage=THROW`.
      //
      // It threw because **the probe was a `file://` URL** — an opaque origin, which gets no storage in
      // *every* browser, and correctly returns a QuotaExceededError. The bug was in the measurement.
      //
      // Which is the whole lesson of this project stated one more time: **the instrument is part of the
      // experiment.** A capability audit run from the wrong origin reports the browser as broken, and I
      // would have "fixed" a 27%-of-the-web outage that did not exist, and reported the win.)

      // (No `FormData` / `URLSearchParams` here. **Real ones already exist**, in the `dom_bindings`
      // prelude, and they harvest a form's successful controls exactly as they should.
      //
      // I wrote duplicates anyway, behind a `typeof === 'undefined'` guard, so they were dead the moment
      // they compiled — and I only noticed because the behaviour did not change when I "fixed" them.
      // This is the SECOND time in two ticks: `localStorage` was the first. Both times the cause was the
      // same, and it was not carelessness about the code — it was **trusting a capability probe that did
      // not test the capability.**
      //
      // The probe is the authority. If it does not test something, its status is UNKNOWN, and "unknown"
      // must not be silently read as "missing". `docs/loop/capability-probe.html` now tests these.)

      // ── **A MISSING CONSTRUCTOR IS A THROWN EXCEPTION, and its blast radius is whatever was
      //    rendering at the time.**
      //
      // This is the single most expensive shape of gap in this project, and it keeps recurring:
      // `canvas.getContext` was used by 3% of sites and BROKE 100% of them. `WebSocket` was missing and
      // took an entire news front page with it — 2,591 server-rendered elements down to 141, because a
      // live-blog client constructed one at boot, React's render threw, and the error boundary showed a
      // skeleton where the article had been.
      //
      // So: **construct successfully, and answer honestly.** A blank canvas, an unopened socket, an
      // empty Blob are all survivable — every library on the web is written to survive them, because
      // real browsers produce them behind captive portals, in private windows, and with permissions
      // denied. A `ReferenceError` is survivable by nothing.
      //
      // Each of these is what a page gets when it *asks*; none of them pretends to work.
      if (typeof globalThis.Blob === 'undefined') {
        globalThis.Blob = function Blob(parts, opts) {
          parts = parts || [];
          var text = '';
          for (var i = 0; i < parts.length; i++) {
            var p = parts[i];
            text += (p && p.__blobText !== undefined) ? p.__blobText : String(p);
          }
          this.__blobText = text;
          this.size = text.length;
          this.type = (opts && opts.type) || '';
        };
        globalThis.Blob.prototype.slice = function (a, b, t) {
          return new globalThis.Blob([String(this.__blobText).slice(a, b)], { type: t || this.type });
        };
        globalThis.Blob.prototype.text = function () { return Promise.resolve(this.__blobText); };
        globalThis.Blob.prototype.arrayBuffer = function () {
          var s = this.__blobText, buf = new ArrayBuffer(s.length), v = new Uint8Array(buf);
          for (var i = 0; i < s.length; i++) { v[i] = s.charCodeAt(i) & 0xff; }
          return Promise.resolve(buf);
        };
        globalThis.Blob.prototype.stream = function () { return null; };
      }
      if (typeof globalThis.File === 'undefined') {
        globalThis.File = function File(parts, name, opts) {
          globalThis.Blob.call(this, parts, opts);
          this.name = String(name || '');
          this.lastModified = 0;
        };
        globalThis.File.prototype = Object.create(globalThis.Blob.prototype);
        globalThis.File.prototype.constructor = globalThis.File;
      }
      if (typeof globalThis.FileReader === 'undefined') {
        globalThis.FileReader = function FileReader() {
          this.result = null; this.error = null; this.readyState = 0;
          this.onload = null; this.onloadend = null; this.onerror = null;
          this.__ls = {};
        };
        var FRp = globalThis.FileReader.prototype;
        FRp.addEventListener = function (t, fn) { (this.__ls[t] = this.__ls[t] || []).push(fn); };
        FRp.removeEventListener = function (t, fn) {
          var a = this.__ls[t]; if (a) { var i = a.indexOf(fn); if (i >= 0) { a.splice(i, 1); } }
        };
        FRp.__done = function (result) {
          var self = this;
          self.result = result; self.readyState = 2;
          setTimeout(function () {
            var ev = { type: 'load', target: self };
            if (typeof self.onload === 'function') { try { self.onload(ev); } catch (e) {} }
            (self.__ls['load'] || []).forEach(function (f) { try { f.call(self, ev); } catch (e) {} });
            var e2 = { type: 'loadend', target: self };
            if (typeof self.onloadend === 'function') { try { self.onloadend(e2); } catch (e) {} }
            (self.__ls['loadend'] || []).forEach(function (f) { try { f.call(self, e2); } catch (e) {} });
          }, 0);
        };
        FRp.readAsText = function (b) { this.__done(b && b.__blobText !== undefined ? b.__blobText : ''); };
        FRp.readAsDataURL = function (b) {
          var t = (b && b.__blobText) || '';
          this.__done('data:' + ((b && b.type) || 'application/octet-stream') + ';base64,' + btoa(t));
        };
        FRp.readAsArrayBuffer = function (b) { this.__done(new ArrayBuffer(0)); };
        FRp.abort = function () { this.readyState = 2; };
      }

      // `new Image()` / `new Audio()` / `new Option()` — these are ELEMENT factories, and a page that
      // preloads with `new Image().src = …` is doing the single most common thing in web performance.
      if (typeof globalThis.Image === 'undefined') {
        globalThis.Image = function Image(w, h) {
          var el = document.createElement('img');
          if (w !== undefined) { el.setAttribute('width', String(w)); }
          if (h !== undefined) { el.setAttribute('height', String(h)); }
          return el;
        };
      }
      if (typeof globalThis.Audio === 'undefined') {
        globalThis.Audio = function Audio(src) {
          var el = document.createElement('audio');
          if (src !== undefined) { el.setAttribute('src', String(src)); }
          return el;   // gets the honest HTMLMediaElement surface — see __manukMedia
        };
      }
      if (typeof globalThis.Option === 'undefined') {
        globalThis.Option = function Option(text, value) {
          var el = document.createElement('option');
          if (text !== undefined) { el.textContent = String(text); }
          if (value !== undefined) { el.setAttribute('value', String(value)); }
          return el;
        };
      }

      // `DOMParser` / `XMLSerializer` — parsing an HTML string into nodes is what every sanitiser,
      // every markdown renderer and every template engine does.
      if (typeof globalThis.DOMParser === 'undefined') {
        globalThis.DOMParser = function DOMParser() {};
        globalThis.DOMParser.prototype.parseFromString = function (str, type) {
          // Build a real detached tree by going through the parser we already have.
          var host = document.createElement('html');
          host.innerHTML = String(str == null ? '' : str);
          // Enough of a Document for the things scripts actually do with the result.
          var doc = {
            documentElement: host,
            body: host.querySelector('body') || host,
            head: host.querySelector('head') || host,
            querySelector: function (s) { return host.querySelector(s); },
            querySelectorAll: function (s) { return host.querySelectorAll(s); },
            getElementById: function (id) { return host.querySelector('#' + id); },
            getElementsByTagName: function (t) { return host.querySelectorAll(t); },
            createElement: function (t) { return document.createElement(t); },
            createTextNode: function (t) { return document.createTextNode(t); },
            contentType: type || 'text/html',
            nodeType: 9
          };
          return doc;
        };
      }
      if (typeof globalThis.XMLSerializer === 'undefined') {
        globalThis.XMLSerializer = function XMLSerializer() {};
        globalThis.XMLSerializer.prototype.serializeToString = function (node) {
          return (node && (node.outerHTML || node.innerHTML)) || '';
        };
      }

      // `DOMRect` — constructed by every layout/measurement library.
      if (typeof globalThis.DOMRect === 'undefined') {
        globalThis.DOMRect = function DOMRect(x, y, w, h) {
          this.x = x || 0; this.y = y || 0; this.width = w || 0; this.height = h || 0;
          this.left = this.x; this.top = this.y;
          this.right = this.x + this.width; this.bottom = this.y + this.height;
        };
        globalThis.DOMRect.fromRect = function (r) {
          r = r || {}; return new globalThis.DOMRect(r.x, r.y, r.width, r.height);
        };
        globalThis.DOMRectReadOnly = globalThis.DOMRect;
      }

      // **`PerformanceObserver`** — every RUM/analytics bundle constructs one on its first line. It
      // observes nothing here, and it says so by never delivering an entry — which is exactly what it
      // does in a browser where the entry types are unsupported.
      if (typeof globalThis.PerformanceObserver === 'undefined') {
        globalThis.PerformanceObserver = function PerformanceObserver(cb) { this.__cb = cb; };
        globalThis.PerformanceObserver.prototype.observe = function () {};
        globalThis.PerformanceObserver.prototype.disconnect = function () {};
        globalThis.PerformanceObserver.prototype.takeRecords = function () { return []; };
        globalThis.PerformanceObserver.supportedEntryTypes = [];
      }

      // **`EventSource` (SSE) — it CONNECTS now (tick 205).** It used to construct and then report
      // that it could not connect, which was honest but left every live-updates page dead: score
      // tickers, deploy/CI log tails, notification streams, dashboard metrics, and the many AI chats
      // that use SSE rather than fetch-streaming.
      //
      // **Built ON TOP of our own `fetch`, which is the whole reason this is small.** Ticks 196-198
      // made `response.body` a real ReadableStream fed incrementally off the wire, and SSE is
      // precisely "a text stream cut into frames on blank lines". So this needs NO new Rust
      // plumbing at all — it is the same code path a polyfill would take, except our fetch is real.
      if (typeof globalThis.EventSource === 'undefined') {
        globalThis.EventSource = function EventSource(url, init) {
          this.url = String(url || ''); this.readyState = 0;
          this.withCredentials = !!(init && init.withCredentials);
          this.onopen = null; this.onmessage = null; this.onerror = null;
          this.__ls = {};
          this.__closed = false;
          this.__lastId = '';
          var self = this;

          var fire = function (type, ev) {
            var on = self['on' + type];
            if (typeof on === 'function') { try { on.call(self, ev); } catch (e) {} }
            var a = (self.__ls[type] || []).slice();
            for (var i = 0; i < a.length; i++) { try { a[i].call(self, ev); } catch (e) {} }
          };
          self.__fire = fire;

          // One SSE frame: blank-line separated, `field: value` lines. `data` accumulates across
          // lines (joined with \n — a multi-line payload is one message, not several).
          var dispatchFrame = function (raw) {
            var type = 'message', data = [], id = null;
            raw.split('\n').forEach(function (line) {
              if (!line || line.charAt(0) === ':') { return; }   // blank or comment (the keepalive)
              var i = line.indexOf(':');
              var field = i < 0 ? line : line.slice(0, i);
              var value = i < 0 ? '' : line.slice(i + 1);
              if (value.charAt(0) === ' ') { value = value.slice(1); }  // ONE leading space, per spec
              if (field === 'data') { data.push(value); }
              else if (field === 'event') { type = value; }
              else if (field === 'id') { id = value; }
              else if (field === 'retry') {
                // The SERVER sets the reconnect delay. Honouring it is not optional politeness:
                // it is how a server sheds load after an incident instead of being hammered by
                // every reconnecting client at its own fixed interval.
                var ms = parseInt(value, 10);
                if (!isNaN(ms) && ms >= 0) { self.__retry = ms; }
              }
            });
            if (id !== null) { self.__lastId = id; }
            if (!data.length) { return; }   // a frame with no data dispatches nothing
            fire(type, {
              type: type, target: self, data: data.join('\n'),
              lastEventId: self.__lastId, origin: self.url
            });
          };

          // **Reconnection is the defining feature of SSE, not a nicety.** The contract a page is
          // written against is "this stream stays alive": servers close idle connections, proxies
          // time out, laptops sleep. Without this, one blip ends the live updates permanently and
          // the page has no way to know it should care.
          self.__retry = 3000;                       // spec default, overridden by `retry:`
          var reconnect = function () {
            if (self.__closed) { return; }
            self.readyState = 0;                     // CONNECTING
            fire('error', { type: 'error', target: self });
            // A MACROtask, so a stream that fails instantly cannot spin the microtask queue
            // without yielding — the same reason the old honest-failure stub used setTimeout.
            setTimeout(function () { if (!self.__closed) { connect(); } }, self.__retry);
          };

          var connect = function () {
          var headers = { 'Accept': 'text/event-stream' };
          // `Last-Event-ID` is what makes a reconnect RESUME rather than restart: the server
          // replays what was missed instead of the page silently losing every event during the gap.
          if (self.__lastId) { headers['Last-Event-ID'] = self.__lastId; }
          fetch(self.url, { headers: headers }).then(function (res) {
            if (self.__closed) { return; }
            // A 204 or a client error is the server saying "stop"; anything else transient
            // gets a retry. Reconnecting into a 404 forever would be a self-inflicted DoS.
            if (res.status === 204 || (res.status >= 400 && res.status < 500)) {
              self.readyState = 2; fire('error', { type: 'error', target: self }); return;
            }
            if (!res.ok || !res.body) { reconnect(); return; }
            self.readyState = 1;                                   // OPEN
            fire('open', { type: 'open', target: self });

            var reader = res.body.getReader();
            var dec = new TextDecoder();
            var buf = '';
            var pump = function () {
              return reader.read().then(function (step) {
                if (self.__closed) { return; }
                if (step.done) {
                  // The stream ended — reconnect, resuming from `Last-Event-ID`.
                  buf = '';
                  reconnect();
                  return;
                }
                // `{stream:true}` — a chunk boundary can split a multi-byte character.
                buf += dec.decode(step.value, { stream: true });
                // Frames are separated by a blank line. Normalise CRLF/CR first: a server that
                // sends \r\n would otherwise never appear to terminate a frame.
                buf = buf.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
                var parts = buf.split('\n\n');
                buf = parts.pop();          // the trailing partial frame stays buffered
                parts.forEach(dispatchFrame);
                return pump();
              });
            };
            return pump();
          }).catch(function () {
            if (self.__closed) { return; }
            reconnect();
          });
          };
          connect();
        };
        globalThis.EventSource.prototype.addEventListener = function (t, fn) {
          (this.__ls[t] = this.__ls[t] || []).push(fn);
        };
        globalThis.EventSource.prototype.removeEventListener = function (t, fn) {
          var a = this.__ls[t]; if (!a) { return; }
          var i = a.indexOf(fn); if (i >= 0) { a.splice(i, 1); }
        };
        globalThis.EventSource.prototype.close = function () {
          this.__closed = true; this.readyState = 2;
        };
        globalThis.EventSource.CONNECTING = 0;
        globalThis.EventSource.OPEN = 1;
        globalThis.EventSource.CLOSED = 2;
      }
      if (typeof globalThis.BroadcastChannel === 'undefined') {
        globalThis.BroadcastChannel = function BroadcastChannel(name) {
          this.name = String(name || ''); this.onmessage = null; this.onmessageerror = null;
        };
        globalThis.BroadcastChannel.prototype.postMessage = function () {};
        globalThis.BroadcastChannel.prototype.close = function () {};
        globalThis.BroadcastChannel.prototype.addEventListener = function () {};
        globalThis.BroadcastChannel.prototype.removeEventListener = function () {};
      }

      // **`Worker`** — constructs, then fires `error`, which is what a browser does when the worker
      // script fails to load. A page that offloads work to a worker takes its main-thread fallback; a
      // page that gets a `ReferenceError` takes nothing.
      if (typeof globalThis.Worker === 'undefined') {
        globalThis.Worker = function Worker(url) {
          this.__url = String(url || '');
          this.onmessage = null; this.onerror = null; this.__ls = {};
          var self = this;
          setTimeout(function () {
            var ev = { type: 'error', message: 'workers are not supported', target: self };
            if (typeof self.onerror === 'function') { try { self.onerror(ev); } catch (e) {} }
            (self.__ls['error'] || []).forEach(function (f) { try { f.call(self, ev); } catch (e) {} });
          }, 0);
        };
        globalThis.Worker.prototype.postMessage = function () {};
        globalThis.Worker.prototype.terminate = function () {};
        globalThis.Worker.prototype.addEventListener = function (t, fn) {
          (this.__ls[t] = this.__ls[t] || []).push(fn);
        };
        globalThis.Worker.prototype.removeEventListener = function () {};
        globalThis.SharedWorker = globalThis.Worker;
      }

      // `window.getSelection()` — editors and "copy link" widgets call it unconditionally.
      if (typeof globalThis.getSelection === 'undefined') {
        globalThis.getSelection = function () {
          return {
            rangeCount: 0, isCollapsed: true, type: 'None', anchorNode: null, focusNode: null,
            toString: function () { return ''; },
            removeAllRanges: function () {}, addRange: function () {}, getRangeAt: function () { return null; },
            collapse: function () {}, selectAllChildren: function () {}
          };
        };
      }

      // ── **The interface surface.** Every name here is one a bundle may *reference* — in an
      //    `instanceof`, a `typeof`, a prototype patch — and **a referenced name that does not exist is a
      //    `ReferenceError`, not a `false`.**
      //
      // That distinction is the whole point and it took a news front page to learn it. The wipe on
      // aljazeera.com peeled one layer at a time: `WebSocket` missing → fix → `Blob` missing → fix →
      // `FileList` missing → … Each was a *different* library's first line, and each took down whatever
      // was rendering. A page does not get to run its fallback path if the check for the fallback throws.
      //
      // These are **inert**: they exist, they are named, and they claim nothing. `x instanceof FileList`
      // answers `false`, which is the truth. The ones that need a real `Symbol.hasInstance` (Node,
      // Element, HTMLElement, Text, Comment, DocumentFragment) get it from `iface()` above — those are
      // load-bearing for custom elements and framework dispatch, and are not in this list.
      //
      // ⚠ **THE NAMES ARE ONLY COLLECTED HERE. THEY ARE INSTALLED AT THE VERY END OF THIS PRELUDE.**
      // This is not a style choice — it is a bug fix, and the bug is instructive:
      //
      //   `AbortSignal` was in this list. The list ran FIRST, so by the time the *real* `AbortSignal`
      //   (a few hundred lines below, with a working listener array) checked
      //   `typeof globalThis.AbortSignal === 'undefined'`, the answer was **false** — its own inert
      //   stub was already sitting there. The real implementation never installed. Every
      //   `new AbortController().abort()` then threw `TypeError: s.__ls is undefined`.
      //
      // **A stub meant to prevent a ReferenceError had silently DISABLED a working implementation.**
      // And `G_GLOBALS` could not see it: that gate asserts `typeof X !== 'undefined'`, which an inert
      // stub satisfies *perfectly*. Existence was never the property worth asserting — **behaviour**
      // was.
      //
      // Installing last makes the `typeof === 'undefined'` guard mean what it was always supposed to
      // mean: **"fill in only what nobody actually implemented."** The ordering is now the mechanism,
      // so this cannot recur the next time someone adds a real implementation for a name on this list.
      var __inertNames = [];
      [
        // Files / data transfer
        'FileList', 'DataTransfer', 'DataTransferItem', 'DataTransferItemList',
        // Streams / requests
        'NodeList', 'HTMLCollection', 'ShadowRoot', 'StyleSheet', 'ResizeObserverEntry',
        'Request', 'Response', 'Headers', 'ReadableStream', 'WritableStream', 'TransformStream',
        'ReadableStreamDefaultReader', 'AbortSignal',
        // Events — a bundle that patches or sniffs one of these references it by name
        'UIEvent', 'FocusEvent', 'InputEvent', 'CompositionEvent', 'WheelEvent', 'PointerEvent',
        'TouchEvent', 'Touch', 'TouchList', 'DragEvent', 'ClipboardEvent', 'ProgressEvent',
        'StorageEvent', 'CloseEvent', 'HashChangeEvent', 'PageTransitionEvent', 'SubmitEvent',
        'AnimationEvent', 'TransitionEvent', 'BeforeUnloadEvent', 'SecurityPolicyViolationEvent',
        // Element interfaces referenced by name in type checks
        'HTMLFormElement', 'HTMLAnchorElement', 'HTMLButtonElement', 'HTMLSelectElement',
        'HTMLTextAreaElement', 'HTMLLabelElement', 'HTMLIFrameElement', 'HTMLLinkElement',
        'HTMLStyleElement', 'HTMLTableElement', 'HTMLVideoElement', 'HTMLAudioElement',
        'HTMLSourceElement', 'HTMLPictureElement', 'HTMLTemplateElement', 'HTMLSlotElement',
        'SVGSVGElement', 'SVGGraphicsElement',
        // Misc platform objects
        'MessagePort', 'Range', 'Selection', 'DOMTokenList', 'NamedNodeMap', 'Attr',
        'CSSRule', 'CSSStyleRule', 'MediaQueryList', 'PerformanceEntry', 'IdleDeadline',
        'Screen', 'History', 'Location', 'VisualViewport'
      ].forEach(function (n) { __inertNames.push(n); });

      // ── **`WebSocket` — it did not exist, and on a live-news site that is the whole page.**
      //
      // Found by the aljazeera.com wipe: 2,591 server-rendered elements became 141. React had cleared its
      // container and re-rendered to a 19-element skeleton — the classic hydration-failure fallback. The
      // only page-level error in the whole load was **"WebSocket implementation missing"**. A live-blog
      // client constructs one at boot; the constructor was a `ReferenceError`; React's render threw; the
      // error boundary showed a skeleton; and the article was gone.
      //
      // **A missing constructor is not a missing feature — it is a thrown exception**, and the blast
      // radius is whatever was rendering at the time. That is the same lesson as `canvas.getContext`
      // (3% of sites, 100% of them broken) and it keeps being the most expensive kind of gap.
      //
      // This does not connect. It **honestly reports that it cannot**: the socket constructs, sits in
      // CONNECTING, and then asynchronously fires `error` and `close` — which is exactly what a real
      // browser does behind a captive portal, and every client on the web is written to survive it. What
      // it must not do is throw at construction, because then nothing is written to survive anything.
      // **It CONNECTS now (tick 201).** The comment above is the history of the stub this replaced;
      // it is kept because the lesson is the expensive one. What changed: the constructor no longer
      // schedules its own failure, it QUEUES a connect op for the host, which owns the socket (the
      // same shape `fetch` uses — the page asks, the host performs, the result is delivered back).
      // A page that still cannot connect gets the identical honest `error`+`close` it always did,
      // now because the connection genuinely failed rather than because we never tried.
      globalThis.__wsOps = [];
      globalThis.__wsObj = {};
      globalThis.__wsId = 0;
      if (typeof globalThis.WebSocket === 'undefined') {
        var WS = function WebSocket(url, protocols) {
          this.url = String(url || '');
          // `protocol` is what the SERVER selects, and it is empty until it does. Pre-filling it with
          // the client's first offer (as the stub did) tells the page a subprotocol was negotiated
          // when nothing has been negotiated yet.
          this.protocol = '';
          this.readyState = 0;                    // CONNECTING
          this.bufferedAmount = 0;
          this.extensions = '';
          this.binaryType = 'blob';
          this.onopen = null; this.onmessage = null; this.onerror = null; this.onclose = null;
          this.__ls = {};
          var offered = Array.isArray(protocols) ? protocols
                      : (protocols ? [String(protocols)] : []);
          this.__id = ++globalThis.__wsId;
          globalThis.__wsObj[this.__id] = this;
          globalThis.__wsOps.push(this.__id + '\x01c\x01' + this.url + '\x01' + offered.join(','));
        };
        WS.prototype.__fire = function (type, ev) {
          var on = this['on' + type];
          if (typeof on === 'function') { try { on.call(this, ev); } catch (e) {} }
          var a = (this.__ls[type] || []).slice();
          for (var i = 0; i < a.length; i++) { try { a[i].call(this, ev); } catch (e) {} }
        };
        WS.prototype.addEventListener = function (t, fn) {
          if (typeof fn === 'function') { (this.__ls[t] = this.__ls[t] || []).push(fn); }
        };
        WS.prototype.removeEventListener = function (t, fn) {
          var a = this.__ls[t]; if (!a) { return; }
          var i = a.indexOf(fn); if (i >= 0) { a.splice(i, 1); }
        };
        WS.prototype.dispatchEvent = function (ev) { this.__fire(ev && ev.type, ev); return true; };
        // `send` on a socket that is not open THROWS in a real browser (InvalidStateError), and clients
        // are written for it. Ours is never open, so this is the honest answer — but it is a *caught*
        // error in every client, not a boot-time ReferenceError, and that is the entire difference.
        // `send` before OPEN still throws InvalidStateError — that is the spec, and clients are
        // written for it. What is new is that a socket can actually BE open.
        WS.prototype.send = function (data) {
          if (this.readyState === 0) {
            throw new DOMException('still connecting', 'InvalidStateError');
          }
          if (this.readyState !== 1) { return; }  // CLOSING/CLOSED: drop, per spec
          var payload, kind;
          if (typeof data === 'string') {
            payload = data; kind = 's';
          } else {
            // ArrayBuffer / typed array / DataView → one char per byte, the convention the whole
            // Rust↔JS byte boundary uses.
            var b = (data instanceof Uint8Array) ? data
                  : (data && data.buffer instanceof ArrayBuffer)
                    ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
                    : (data instanceof ArrayBuffer) ? new Uint8Array(data)
                    : new Uint8Array(0);
            var out = '';
            for (var i = 0; i < b.length; i++) { out += String.fromCharCode(b[i]); }
            payload = out; kind = 'b';
          }
          // `bufferedAmount` is what a client polls to avoid flooding a slow socket. It rises here
          // and falls when the host reports the frame written.
          this.bufferedAmount += payload.length;
          globalThis.__wsOps.push(this.__id + '\x01' + kind + '\x01' + payload + '\x01');
        };
        WS.prototype.close = function (code, reason) {
          if (this.readyState === 2 || this.readyState === 3) { return; }
          this.readyState = 2;                    // CLOSING — the handshake is not instant
          globalThis.__wsOps.push(
            this.__id + '\x01x\x01' + (code === undefined ? '' : String(code)) + '\x01' + (reason || '')
          );
        };
        WS.CONNECTING = 0; WS.OPEN = 1; WS.CLOSING = 2; WS.CLOSED = 3;
        WS.prototype.CONNECTING = 0; WS.prototype.OPEN = 1;
        WS.prototype.CLOSING = 2; WS.prototype.CLOSED = 3;
        globalThis.WebSocket = WS;
      }

      // ── Host → page delivery. One entry point per event the transport can produce. ────────────
      // Unknown ids are a no-op throughout: the page may have dropped the socket, or the document
      // may have been torn down while a frame was in flight.
      globalThis.__wsOpen = function (id, protocol, extensions) {
        var w = globalThis.__wsObj[id]; if (!w || w.readyState !== 0) { return; }
        w.readyState = 1;                          // OPEN
        w.protocol = protocol || '';               // what the SERVER chose
        w.extensions = extensions || '';
        w.__fire('open', { type: 'open', target: w });
      };
      globalThis.__wsMessage = function (id, data, isBinary) {
        var w = globalThis.__wsObj[id]; if (!w || w.readyState !== 1) { return; }
        var payload = data;
        if (isBinary) {
          var b = globalThis.__bytesFromLatin1(data);
          // `binaryType` decides the shape a page receives, and a client that set 'arraybuffer'
          // and got a Blob (or vice versa) breaks on the first byte it reads.
          payload = (w.binaryType === 'arraybuffer') ? b.buffer : new globalThis.Blob([data]);
        }
        w.__fire('message', { type: 'message', target: w, data: payload, origin: w.url });
      };
      globalThis.__wsSent = function (id, n) {
        var w = globalThis.__wsObj[id]; if (!w) { return; }
        w.bufferedAmount = Math.max(0, w.bufferedAmount - (n || 0));
      };
      globalThis.__wsError = function (id, message) {
        var w = globalThis.__wsObj[id]; if (!w) { return; }
        // The spec's `error` event carries NO detail (deliberately — it would be a cross-origin
        // information leak). The message is for our own logging, not for the page.
        w.__fire('error', { type: 'error', target: w });
      };
      globalThis.__wsClose = function (id, code, reason, clean) {
        var w = globalThis.__wsObj[id]; if (!w || w.readyState === 3) { return; }
        w.readyState = 3;                          // CLOSED
        delete globalThis.__wsObj[id];
        w.__fire('close', {
          type: 'close', target: w, code: code || 1006, reason: reason || '', wasClean: !!clean
        });
      };

      // **`Notification`** — 14% of the corpus. Almost always feature-detected, so the honest answer is
      // the useful one: the constructor exists, permission is `"denied"`, and `requestPermission()`
      // resolves to `"denied"`. A site asked, and was told no. Nothing throws, and no user is nagged.
      if (typeof globalThis.Notification === 'undefined') {
        globalThis.Notification = function Notification(){ this.close = function(){}; };
        globalThis.Notification.permission = 'denied';
        globalThis.Notification.requestPermission = function(){ return Promise.resolve('denied'); };
      }

      if (typeof globalThis.MediaError === 'undefined') {
        globalThis.MediaError = function MediaError(code) { this.code = code || 4; this.message = ''; };
        globalThis.MediaError.MEDIA_ERR_ABORTED = 1;
        globalThis.MediaError.MEDIA_ERR_NETWORK = 2;
        globalThis.MediaError.MEDIA_ERR_DECODE = 3;
        globalThis.MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED = 4;
      }
      iface('HTMLMediaElement', function(o){
        return !!o && o.nodeType === 1 && (o.tagName === 'VIDEO' || o.tagName === 'AUDIO');
      });
      iface('HTMLVideoElement', function(o){ return !!o && o.nodeType === 1 && o.tagName === 'VIDEO'; });
      iface('HTMLAudioElement', function(o){ return !!o && o.nodeType === 1 && o.tagName === 'AUDIO'; });

      // Installed on every media element as it is reflected. Own properties, like the rest of our DOM
      // surface — see the prototype bridge above for why that is not the same as having a prototype.
      globalThis.__manukMedia = function(el) {
        if (!el || el.__mediaReady) { return el; }
        Object.defineProperty(el, '__mediaReady', { value: true, enumerable: false });

        var err = new globalThis.MediaError(4);   // MEDIA_ERR_SRC_NOT_SUPPORTED — and it is the truth
        var ro = function(name, value) {
          Object.defineProperty(el, name, { get: function(){ return value; }, configurable: true });
        };
        ro('error', err);
        ro('readyState', 0);        // HAVE_NOTHING — and it stays 0 until a frame is genuinely decoded
        ro('paused', true);
        ro('ended', false);
        ro('seeking', false);

        // ── The three members a MediaSource makes LIVE.
        //
        // These were fixed-value closures, which is fine while the element can never have a source
        // — but MSE gives it one, and a player polls exactly these to decide whether to append more.
        // A `duration` frozen at NaN after the page set `mediaSource.duration = 600` reads as "this
        // stream has no length", and the append loop has nothing to measure its progress against.
        var live = function(name, get) {
          Object.defineProperty(el, name, { get: get, configurable: true });
        };
        // NETWORK_LOADING once a MediaSource is attached: the element genuinely is being fed.
        live('networkState', function(){ return el.__ms ? 2 : 3; });
        live('duration',     function(){ return el.__ms ? el.__ms.duration : NaN; });
        // The union of the source buffers' ranges — empty today, because nothing is demuxed, and
        // reporting an empty range is the honest answer rather than a fabricated one.
        live('buffered', function() {
          if (!el.__ms) { return { length: 0, start: function(){ return 0; }, end: function(){ return 0; } }; }
          var sbs = el.__ms.sourceBuffers;
          return sbs.length ? sbs[0].buffered : new globalThis.TimeRanges([]);
        });
        ro('played',    { length: 0, start: function(){ return 0; }, end: function(){ return 0; } });
        ro('seekable',  { length: 0, start: function(){ return 0; }, end: function(){ return 0; } });
        ro('textTracks', []);
        ro('videoWidth', 0);
        ro('videoHeight', 0);

        // Writable, because scripts set them and expect them to stick. They just do not do anything.
        el.currentTime = 0; el.volume = 1; el.muted = false; el.playbackRate = 1;
        el.autoplay = el.autoplay || false; el.loop = el.loop || false;
        el.__ms = null;

        // ── `video.src = URL.createObjectURL(mediaSource)` — the MSE attachment handshake.
        //
        // This assignment is the ONLY moment the element learns which MediaSource it is playing,
        // and every player library performs it. Without an interception here the object URL is
        // stored as an ordinary attribute, the source stays 'closed', `sourceopen` never fires, and
        // the player waits on that event forever — a hang with nothing in the DOM to see.
        //
        // The reflected accessor is found lazily on the prototype chain rather than captured now,
        // because reflection installs `src` on the prototype and this runs per-element; looking it
        // up eagerly would depend on an install order that is not ours to rely on.
        var reflected = function() {
          var p = Object.getPrototypeOf(el);
          while (p) {
            var d = Object.getOwnPropertyDescriptor(p, 'src');
            if (d && (d.get || d.set)) { return d; }
            p = Object.getPrototypeOf(p);
          }
          return null;
        };
        Object.defineProperty(el, 'src', {
          configurable: true,
          get: function() {
            // An object URL is returned verbatim: it is already absolute and has no base to
            // resolve against, so round-tripping it through URL reflection would corrupt it.
            if (el.__objectSrc) { return el.__objectSrc; }
            var d = reflected();
            if (d && d.get) { return d.get.call(el); }
            return (el.getAttribute && el.getAttribute('src')) || '';
          },
          set: function(v) {
            var s = String(v);
            el.__objectSrc = s.indexOf('blob:') === 0 ? s : null;
            var d = reflected();
            if (d && d.set) { d.set.call(el, s); }
            else if (el.setAttribute) { el.setAttribute('src', s); }
            if (globalThis.__mseAttach) { globalThis.__mseAttach(el, s); }
          }
        });

        // `srcObject = mediaSource` — the newer attachment form, no object URL in between.
        Object.defineProperty(el, 'srcObject', {
          configurable: true,
          get: function() { return el.__ms; },
          set: function(v) {
            if (v && globalThis.MediaSource && v instanceof globalThis.MediaSource) {
              el.__ms = v; v.__element = el; v.__setReadyState('open');
            } else if (el.__ms) {
              var old = el.__ms; el.__ms = null; old.__element = null; old.__setReadyState('closed');
            }
          }
        });

        el.HAVE_NOTHING = 0; el.HAVE_METADATA = 1; el.HAVE_CURRENT_DATA = 2;
        el.HAVE_FUTURE_DATA = 3; el.HAVE_ENOUGH_DATA = 4;
        el.NETWORK_EMPTY = 0; el.NETWORK_IDLE = 1; el.NETWORK_LOADING = 2; el.NETWORK_NO_SOURCE = 3;

        // `''` is the spec's "no". `'probably'` / `'maybe'` are the only other answers, and both
        // would be lies.
        el.canPlayType = function() { return ''; };

        el.play = function() {
          return Promise.reject(new DOMException('media playback is not supported by this browser', 'NotSupportedError'));
        };
        el.pause = function() {};
        el.load  = function() {};
        el.addTextTrack = function() { return { cues: [], activeCues: [], mode: 'disabled' }; };
        el.requestPictureInPicture = function() {
          return Promise.reject(new DOMException('picture-in-picture is not supported', 'NotSupportedError'));
        };
        return el;
      };

      // NO BRIDGES ANY MORE — and removing them is not tidying, it is required.
      //
      // `bridge()` defined an accessor on the prototype that forwarded to the INSTANCE's own property
      // descriptor. It existed precisely BECAUSE the members were own-properties of every element. They
      // are not any more — they live on the real prototype chain. A forwarding accessor left in place
      // would sit in FRONT of the real one, look for an own property that no longer exists, and return
      // `undefined` for every DOM read on the page.
      // `instanceof Document` must recognise EVERY document — the main one, an `<iframe>`'s, and one from
      // `DOMImplementation.createHTMLDocument()`/`createDocument()` — not just the singleton `document`.
      // A Document node has nodeType 9; the old `o === document` predicate made `createHTMLDocument()
      // instanceof Document` false, which is the FIRST assertion in `DOMImplementation-createHTMLDocument`.
      iface('Document', function(o){ return !!o && o.nodeType === 9; });
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
      iface('HTMLDialogElement',   tagIs('DIALOG'));
      iface('HTMLDivElement',      tagIs('DIV'));
      iface('HTMLSpanElement',     tagIs('SPAN'));
      // The structural elements a created document is made of — `DOMImplementation-createHTMLDocument`
      // asserts `documentElement instanceof HTMLHtmlElement`, `head instanceof HTMLHeadElement`, etc.
      iface('HTMLHtmlElement',     tagIs('HTML'));
      iface('HTMLHeadElement',     tagIs('HEAD'));
      iface('HTMLBodyElement',     tagIs('BODY'));
      iface('HTMLTitleElement',    tagIs('TITLE'));

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

      // **`document.createTreeWalker` — how Lit (and lit-html, and anything template-based) finds the
      // dynamic holes in a cloned template.** Without it: `E.createTreeWalker is not a function`, and
      // Lit dies before rendering a node.
      //
      // Built on the traversal we already expose rather than natively, because the walk is the easy
      // part and the *filter protocol* is the part worth getting right: `acceptNode` may be a bare
      // function or an object with an `acceptNode` method, and FILTER_REJECT (2) must skip the whole
      // subtree while FILTER_SKIP (3) skips only the node. Getting that backwards would silently drop
      // every dynamic binding in a template, which is a bug that renders *something* — the worst kind.
      // **Constructable stylesheets** — how every modern web-component library ships styles. Lit's
      // `static styles = css\`...\`` builds a `CSSStyleSheet` and adopts it; without the constructor,
      // `CSSStyleSheet is not defined` and Lit dies before rendering.
      //
      // The sheet is a real object with the right shape, and `adoptedStyleSheets` accepts it. What it
      // does NOT yet do is feed the cascade — so a Lit component renders its CONTENT but not its
      // styles. That is a rendering gap, and it is strictly better than a blank page: unstyled content
      // is legible, absent content is not. Wiring adopted sheets into the cascade is the follow-on, and
      // it is a real one rather than a "TODO" that means "never".
      // **`AbortController` — every modern `fetch` passes a signal.** Its absence is not a missing
      // nicety: a library that constructs one unconditionally (and most do) throws before it ever gets
      // to the request.
      if (typeof globalThis.AbortSignal === 'undefined') {
        globalThis.AbortSignal = function AbortSignal() {
          this.aborted = false; this.reason = undefined; this.onabort = null; this.__ls = [];
        };
        globalThis.AbortSignal.prototype.addEventListener = function(t, f) {
          if (t === 'abort' && typeof f === 'function') { this.__ls.push(f); }
        };
        globalThis.AbortSignal.prototype.removeEventListener = function(t, f) {
          this.__ls = this.__ls.filter(function(x){ return x !== f; });
        };
        globalThis.AbortSignal.prototype.throwIfAborted = function() {
          if (this.aborted) { throw this.reason; }
        };
        globalThis.AbortSignal.abort = function(reason) {
          var s = new AbortSignal(); s.aborted = true;
          s.reason = reason !== undefined ? reason
            : new DOMException('signal is aborted without reason', 'AbortError');
          return s;
        };
        globalThis.AbortSignal.timeout = function(ms) {
          var s = new AbortSignal();
          setTimeout(function(){ s.aborted = true; }, ms);
          return s;
        };
      }
      if (typeof globalThis.AbortController === 'undefined') {
        globalThis.AbortController = function AbortController() { this.signal = new AbortSignal(); };
        globalThis.AbortController.prototype.abort = function(reason) {
          var s = this.signal;
          if (s.aborted) { return; }
          s.aborted = true;
          // The default abort reason is a DOMException named 'AbortError' (Fetch/DOM standard) — a
          // `fetch()` rejected by an abort must have `err.name === 'AbortError'`, which React's effect
          // cleanup and every request library check for to distinguish a cancel from a real failure.
          s.reason = reason !== undefined ? reason
            : new DOMException('signal is aborted without reason', 'AbortError');
          var e = { type: 'abort', target: s };
          if (typeof s.onabort === 'function') { try { s.onabort(e); } catch (x) {} }
          s.__ls.slice().forEach(function(f){ try { f(e); } catch (x) {} });
        };
      }

      // `TextEncoder`/`TextDecoder` — UTF-8 only, which is what the web is.
      if (typeof globalThis.TextEncoder === 'undefined') {
        globalThis.TextEncoder = function TextEncoder(){ this.encoding = 'utf-8'; };
        globalThis.TextEncoder.prototype.encode = function(str) {
          str = String(str === undefined ? '' : str);
          var out = [], i, c;
          for (i = 0; i < str.length; i++) {
            c = str.charCodeAt(i);
            if (c < 0x80) { out.push(c); }
            else if (c < 0x800) { out.push(0xc0 | (c >> 6), 0x80 | (c & 63)); }
            else if (c >= 0xd800 && c <= 0xdbff && i + 1 < str.length) {
              var c2 = str.charCodeAt(++i);
              var cp = 0x10000 + ((c - 0xd800) << 10) + (c2 - 0xdc00);
              out.push(0xf0 | (cp >> 18), 0x80 | ((cp >> 12) & 63), 0x80 | ((cp >> 6) & 63), 0x80 | (cp & 63));
            } else { out.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63)); }
          }
          return new Uint8Array(out);
        };
        globalThis.TextDecoder = function TextDecoder(){ this.encoding = 'utf-8'; this.__tail = null; };
        // `decode(chunk, {stream: true})` — **the streaming contract, and it is not optional.**
        // A network chunk boundary lands wherever the wire put it, which is routinely in the MIDDLE
        // of a multi-byte character: "café" split after 0xC3 leaves a lead byte with no continuation.
        // Decoding each chunk independently turns that into U+FFFD and silently corrupts the text —
        // so with `{stream:true}` we hold the incomplete trailing sequence back and prepend it to the
        // next call. Every streaming client on the web passes this flag; without support for it the
        // whole `response.body` path mangles any non-ASCII answer.
        globalThis.TextDecoder.prototype.decode = function(buf, opts) {
          var streaming = !!(opts && opts.stream);
          var input = buf ? (buf instanceof Uint8Array ? buf : new Uint8Array(buf)) : new Uint8Array(0);
          var b = input;
          if (this.__tail && this.__tail.length) {
            b = new Uint8Array(this.__tail.length + input.length);
            b.set(this.__tail, 0);
            b.set(input, this.__tail.length);
          }
          this.__tail = null;
          var end = b.length;
          if (streaming) {
            // Walk back over the trailing continuation bytes (10xxxxxx) to the lead byte. If that
            // sequence is short of the length its lead byte announces, it is incomplete — hold it.
            var start = end - 1, steps = 0;
            while (start >= 0 && steps < 4) {
              var lead = b[start];
              if ((lead & 0xc0) !== 0x80) {
                var need = lead < 0x80 ? 1 : lead < 0xe0 ? 2 : lead < 0xf0 ? 3 : 4;
                if (end - start < need) { this.__tail = new Uint8Array(b.subarray(start, end)); end = start; }
                break;
              }
              start--; steps++;
            }
          }
          var s = '', i = 0;
          while (i < end) {
            var c = b[i++];
            if (c < 0x80) { s += String.fromCharCode(c); }
            else if (c < 0xe0) { s += String.fromCharCode(((c & 31) << 6) | (b[i++] & 63)); }
            else if (c < 0xf0) { s += String.fromCharCode(((c & 15) << 12) | ((b[i++] & 63) << 6) | (b[i++] & 63)); }
            else {
              var cp = ((c & 7) << 18) | ((b[i++] & 63) << 12) | ((b[i++] & 63) << 6) | (b[i++] & 63);
              cp -= 0x10000;
              s += String.fromCharCode(0xd800 + (cp >> 10), 0xdc00 + (cp & 1023));
            }
          }
          return s;
        };
      }

      // **`btoa` / `atob`** — base64. Used constantly: data URLs, JWT decoding, image inlining, every
      // "encode this small thing into a string" on the web.
      if (typeof globalThis.btoa === 'undefined') {
        var B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
        globalThis.btoa = function(input) {
          var str = String(input), out = '', i = 0;
          while (i < str.length) {
            var c1 = str.charCodeAt(i++), c2 = str.charCodeAt(i++), c3 = str.charCodeAt(i++);
            if (c1 > 255 || c2 > 255 || c3 > 255) {
              throw new Error("btoa: string contains characters outside of the Latin1 range");
            }
            var e1 = c1 >> 2;
            var e2 = ((c1 & 3) << 4) | (isNaN(c2) ? 0 : (c2 >> 4));
            var e3 = isNaN(c2) ? 64 : (((c2 & 15) << 2) | (isNaN(c3) ? 0 : (c3 >> 6)));
            var e4 = isNaN(c3) ? 64 : (c3 & 63);
            out += B64.charAt(e1) + B64.charAt(e2) +
                   (e3 === 64 ? '=' : B64.charAt(e3)) + (e4 === 64 ? '=' : B64.charAt(e4));
          }
          return out;
        };
        globalThis.atob = function(input) {
          var str = String(input).replace(/[=]+$/, ''), out = '', bits = 0, acc = 0;
          for (var i = 0; i < str.length; i++) {
            var v = B64.indexOf(str.charAt(i));
            if (v < 0) { continue; }
            acc = (acc << 6) | v; bits += 6;
            if (bits >= 8) { bits -= 8; out += String.fromCharCode((acc >> bits) & 0xff); }
          }
          return out;
        };
      }

      // Dialogs. A renderer has no user to ask, and **silently returning the wrong answer is worse
      // than doing nothing**: `confirm()` returning `true` by default would let a page believe the user
      // agreed to something. `false`/`null` is the honest, safe answer, and it is logged.
      if (typeof globalThis.alert === 'undefined') {
        globalThis.alert = function(m){ try { __hostLog('info', 'alert: ' + m); } catch(e){} };
        globalThis.confirm = function(m){ try { __hostLog('info', 'confirm (auto-declined): ' + m); } catch(e){} return false; };
        globalThis.prompt = function(m){ try { __hostLog('info', 'prompt (auto-declined): ' + m); } catch(e){} return null; };
      }
      if (typeof globalThis.postMessage === 'undefined') {
        globalThis.postMessage = function(){};
      }
      if (typeof globalThis.reportError === 'undefined') {
        globalThis.reportError = function(e){ try { __hostLog('warn', 'reportError: ' + e); } catch(x){} };
      }

      // `crypto.getRandomValues` / `crypto.randomUUID` are everywhere (React keys, request ids,
      // session tokens, cache busting). They must be CRYPTOGRAPHICALLY secure — the old shim filled
      // both from `Math.random()`, a non-cryptographic PRNG, so every token a page minted was
      // predictable. Entropy comes from the host OS CSPRNG via `__cryptoRandomHex`; the fill is
      // per-BYTE (so a Uint32Array gets full 32-bit values, not the old 0..255) and `randomUUID`
      // sets the RFC 4122 version (4) and variant (10xx) bits so it emits a *valid* v4 UUID.
      if (typeof globalThis.crypto === 'undefined') {
        var __mkCryptoErr = function(name, msg) {
          try { return new DOMException(msg, name); }
          catch (e) { var er = new Error(msg); er.name = name; return er; }
        };
        // The integer ArrayBufferView types WebCrypto accepts; a Float*/DataView/plain array throws.
        var __INT_VIEWS = {
          Int8Array: 1, Uint8Array: 1, Uint8ClampedArray: 1,
          Int16Array: 1, Uint16Array: 1, Int32Array: 1, Uint32Array: 1,
          BigInt64Array: 1, BigUint64Array: 1
        };
        globalThis.crypto = {
          getRandomValues: function(a) {
            var ctor = (a && a.constructor && a.constructor.name) || '';
            if (!a || typeof a.byteLength !== 'number' || typeof a.buffer === 'undefined' || !__INT_VIEWS[ctor]) {
              throw __mkCryptoErr('TypeMismatchError',
                "Failed to execute 'getRandomValues' on 'Crypto': the provided ArrayBufferView is not an integer-typed array");
            }
            if (a.byteLength > 65536) {
              throw __mkCryptoErr('QuotaExceededError',
                "Failed to execute 'getRandomValues' on 'Crypto': the ArrayBufferView's byte length (" +
                a.byteLength + ") exceeds the number of bytes of entropy available via this API (65536)");
            }
            if (a.byteLength === 0) { return a; }
            var hex = __cryptoRandomHex(a.byteLength);
            if (hex.length !== a.byteLength * 2) {
              throw __mkCryptoErr('OperationError',
                "Failed to execute 'getRandomValues' on 'Crypto': the platform CSPRNG is unavailable");
            }
            // Write through a byte view so EVERY element byte is random, whatever the element width.
            var bytes = new Uint8Array(a.buffer, a.byteOffset, a.byteLength);
            for (var i = 0; i < bytes.length; i++) {
              bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
            }
            return a;
          },
          randomUUID: function() {
            var hex = __cryptoRandomHex(16);
            if (hex.length !== 32) {
              throw __mkCryptoErr('OperationError',
                "Failed to execute 'randomUUID' on 'Crypto': the platform CSPRNG is unavailable");
            }
            var b = new Array(16);
            for (var i = 0; i < 16; i++) { b[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16); }
            b[6] = (b[6] & 0x0f) | 0x40;   // RFC 4122 §4.4 — version nibble = 4
            b[8] = (b[8] & 0x3f) | 0x80;   // RFC 4122 §4.4 — variant bits = 10xx
            var h = '0123456789abcdef', s = '';
            for (var i = 0; i < 16; i++) {
              s += h[(b[i] >> 4) & 0xf] + h[b[i] & 0xf];
              if (i === 3 || i === 5 || i === 7 || i === 9) { s += '-'; }
            }
            return s;
          },
          // SubtleCrypto — the digest slice. `crypto.subtle.digest(algo, data)` is what Subresource
          // Integrity checks, content-addressed caches and many auth libraries call; without it,
          // `crypto.subtle` was `undefined` and `crypto.subtle.digest(...)` threw. Hashing runs in the
          // host (`__subtleDigestHex`, pure-Rust RustCrypto) and the result is wrapped in a resolved
          // Promise, matching the async signature real code awaits. Only digest is provided — sign /
          // encrypt / deriveKey stay absent (honestly, so a page's `if (crypto.subtle.encrypt)` guard
          // takes its fallback rather than getting a broken stub).
          subtle: {
            digest: function(algorithm, data) {
              var name = String((typeof algorithm === 'string') ? algorithm : (algorithm && algorithm.name) || '').toUpperCase();
              var NORM = {
                'SHA-1': 'SHA-1', 'SHA1': 'SHA-1', 'SHA-256': 'SHA-256', 'SHA256': 'SHA-256',
                'SHA-384': 'SHA-384', 'SHA384': 'SHA-384', 'SHA-512': 'SHA-512', 'SHA512': 'SHA-512'
              };
              var algo = NORM[name];
              if (!algo) { return Promise.reject(__mkCryptoErr('NotSupportedError', "Unrecognized algorithm name: " + name)); }
              var bytes;
              try {
                if (data instanceof ArrayBuffer) { bytes = new Uint8Array(data); }
                else if (data && data.buffer instanceof ArrayBuffer) { bytes = new Uint8Array(data.buffer, data.byteOffset, data.byteLength); }
                else { return Promise.reject(new TypeError("Failed to execute 'digest' on 'SubtleCrypto': data is not a BufferSource")); }
              } catch (e) { return Promise.reject(e); }
              var ih = '';
              for (var i = 0; i < bytes.length; i++) { ih += ('0' + bytes[i].toString(16)).slice(-2); }
              var oh = __subtleDigestHex(algo, ih);
              if (oh.length === 0) { return Promise.reject(__mkCryptoErr('OperationError', "digest failed")); }
              var out = new ArrayBuffer(oh.length / 2), ov = new Uint8Array(out);
              for (var j = 0; j < ov.length; j++) { ov[j] = parseInt(oh.slice(j * 2, j * 2 + 2), 16); }
              return Promise.resolve(out);
            }
          }
        };
      }

      // ── HTML Constraint Validation (the form-validity API). Every validation library — the browser's
      // own native validation, React Hook Form, Formik, VeeValidate — reads `el.validity.valueMissing`,
      // calls `form.checkValidity()`, and listens for the `invalid` event. All of it was ABSENT:
      // `input.checkValidity` was `undefined`, so `if (!input.checkValidity())` is a TypeError that takes
      // the submit handler with it, and the form silently cannot submit. Defined once on the shared
      // HTMLElement prototype (built in Rust, see dom_bindings::dom_protos) so every element inherits it;
      // the members read the reflected content attributes (`required`/`pattern`/`type`/`min`/`max`/
      // `minLength`/`maxLength`, all live via G_REFLECT) plus the current `value`.
      var __HP = globalThis.__protoHTMLElement;
      if (__HP && typeof __HP.checkValidity === 'undefined') {
        var __isFormControl = function(el) {
          var t = el && el.tagName;
          return t === 'INPUT' || t === 'SELECT' || t === 'TEXTAREA';
        };
        // A "barred from constraint validation" element never validates (spec §form-control-infrastructure).
        var __barredType = { hidden: 1, submit: 1, reset: 1, button: 1, image: 1 };
        var __numericType = { number: 1, range: 1 };
        var __willValidate = function(el) {
          if (!__isFormControl(el)) { return false; }
          if (el.disabled || el.readOnly) { return false; }
          if (el.tagName === 'INPUT' && __barredType[String(el.type || 'text').toLowerCase()]) { return false; }
          return true;
        };
        // A single fresh ValidityState per read (browsers return a live object; a snapshot is
        // indistinguishable to the synchronous code that reads it right after a value change).
        var __computeValidity = function(el) {
          var v = {
            valueMissing: false, typeMismatch: false, patternMismatch: false,
            tooLong: false, tooShort: false, rangeUnderflow: false, rangeOverflow: false,
            stepMismatch: false, badInput: false, customError: false, valid: true
          };
          if (!__willValidate(el)) { v.customError = !!el.__customValidity; v.valid = !v.customError; return v; }
          var val = el.value == null ? '' : String(el.value);
          var type = String(el.type || 'text').toLowerCase();
          if (el.required && val === '') { v.valueMissing = true; }
          if (val !== '') {
            if (type === 'email' && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(val)) { v.typeMismatch = true; }
            if (type === 'url') { try { new URL(val); } catch (e) { v.typeMismatch = true; } }
            var pat = el.pattern;
            if (pat != null && pat !== '') {
              try { if (!(new RegExp('^(?:' + pat + ')$', 'u')).test(val)) { v.patternMismatch = true; } }
              catch (e) { /* an invalid pattern does not constrain (spec) */ }
            }
            var maxL = el.maxLength;
            if (typeof maxL === 'number' && maxL >= 0 && val.length > maxL) { v.tooLong = true; }
            var minL = el.minLength;
            if (typeof minL === 'number' && minL >= 0 && val.length < minL) { v.tooShort = true; }
            if (__numericType[type]) {
              var num = parseFloat(val);
              if (!isNaN(num)) {
                if (el.min !== '' && el.min != null && num < parseFloat(el.min)) { v.rangeUnderflow = true; }
                if (el.max !== '' && el.max != null && num > parseFloat(el.max)) { v.rangeOverflow = true; }
              }
            }
          }
          if (el.__customValidity) { v.customError = true; }
          v.valid = !(v.valueMissing || v.typeMismatch || v.patternMismatch || v.tooLong ||
                      v.tooShort || v.rangeUnderflow || v.rangeOverflow || v.stepMismatch ||
                      v.badInput || v.customError);
          return v;
        };
        Object.defineProperty(__HP, 'validity', { configurable: true, get: function() { return __computeValidity(this); } });
        Object.defineProperty(__HP, 'willValidate', { configurable: true, get: function() {
          return __willValidate(this);
        } });
        Object.defineProperty(__HP, 'validationMessage', { configurable: true, get: function() {
          if (this.__customValidity) { return String(this.__customValidity); }
          return __computeValidity(this).valid ? '' : 'Please fill out this field.';
        } });
        __HP.setCustomValidity = function(msg) { this.__customValidity = (msg == null ? '' : String(msg)) || ''; };
        // A single element's check: fire a cancelable `invalid` event when it fails, then report validity.
        var __checkOne = function(el) {
          var v = __computeValidity(el);
          if (!v.valid && __willValidate(el)) {
            try { el.dispatchEvent(new Event('invalid', { cancelable: true, bubbles: false })); } catch (e) {}
          }
          return v.valid;
        };
        __HP.checkValidity = function() {
          if (this.tagName === 'FORM') {
            var ok = true, ctrls = this.querySelectorAll('input,select,textarea');
            for (var i = 0; i < ctrls.length; i++) { if (!__checkOne(ctrls[i])) { ok = false; } }
            return ok;
          }
          return __willValidate(this) ? __checkOne(this) : true;
        };
        // No native validation UI here, so reportValidity is checkValidity (it still fires `invalid`).
        __HP.reportValidity = function() { return this.checkValidity(); };
      }

      // ── `<dialog>`: show() / showModal() / close() — the modal. Every cookie banner, every
      // confirm-delete, every command palette shipped since the pattern stopped being a hand-rolled
      // `<div class="modal">` is a `<dialog>`, and the whole surface was ABSENT: `open` reflected as an
      // attribute and nothing else. `dlg.showModal()` was a TypeError that took the click handler with
      // it, so the button did nothing at all — and because a dialog with no UA `display:none` rule is
      // just a block, its contents were ALREADY PAINTED into the page, inline, before anyone opened it.
      // Both halves are fixed here: the methods (below) and the UA rule (css: `dialog:not([open])`).
      //
      // Modality is marked with `data-manuk-modal` — same device as `data-manuk-adopted`, because the
      // Rust side (top-layer stacking in `Page::z_index_map`) has to see the flag and a JS-side property
      // is invisible to it. Non-modal `show()` deliberately does NOT set it: only a modal dialog joins
      // the top layer, so a `show()`n dialog stays in flow where the spec puts it.
      if (__HP && typeof __HP.showModal === 'undefined') {
        var __isDialog = function(el) { return !!el && el.tagName === 'DIALOG'; };
        // The open modals, innermost last — a stack, because a dialog may open a dialog.
        var __modalStack = [];
        __HP.show = function() {
          if (!__isDialog(this) || this.hasAttribute('open')) { return; }
          this.setAttribute('open', '');
        };
        __HP.showModal = function() {
          if (!__isDialog(this)) { return; }
          // Spec: showModal() on an already-open dialog is an InvalidStateError, and libraries DO
          // double-open (a re-render calls it again). Throwing the right name lets them catch it.
          if (this.hasAttribute('open')) {
            var e = new Error('The element already has an "open" attribute');
            e.name = 'InvalidStateError';
            throw e;
          }
          this.setAttribute('open', '');
          this.setAttribute('data-manuk-modal', '');
          __modalStack.push(this);
        };
        __HP.close = function(rv) {
          if (!__isDialog(this) || !this.hasAttribute('open')) { return; }
          if (rv !== undefined) { this.returnValue = rv; }
          this.removeAttribute('open');
          this.removeAttribute('data-manuk-modal');
          var i = __modalStack.indexOf(this);
          if (i >= 0) { __modalStack.splice(i, 1); }
          // `close` is the event the calling code waits on to read `returnValue`. Not cancelable:
          // by the time it fires the dialog is already closed.
          try { this.dispatchEvent(new Event('close', { bubbles: false, cancelable: false })); } catch (e) {}
        };
        Object.defineProperty(__HP, 'returnValue', {
          configurable: true,
          get: function() { return this.__returnValue == null ? '' : this.__returnValue; },
          set: function(v) { this.__returnValue = String(v); }
        });
        // `<form method="dialog">` — the canonical close button, and it is pure markup: the page
        // ships `<form method="dialog"><button value="ok">OK</button></form>` and expects the click
        // to close the dialog with `returnValue === 'ok'` and NO navigation. Handled at the document
        // in the capture phase so it runs before the native submit path can treat it as a GET.
        var __dialogSubmit = function(ev) {
          var t = ev.target;
          while (t && t.tagName !== 'BUTTON' && t.tagName !== 'INPUT') { t = t.parentElement; }
          if (!t) { return; }
          var type = String(t.type || (t.tagName === 'BUTTON' ? 'submit' : '')).toLowerCase();
          if (type !== 'submit' && type !== 'image') { return; }
          // `formmethod` on the button overrides the form's own `method` (spec).
          var fm = t.getAttribute && t.getAttribute('formmethod');
          var form = t.form || (t.closest && t.closest('form'));
          var method = String(fm || (form && form.getAttribute('method')) || '').toLowerCase();
          if (method !== 'dialog') { return; }
          var dlg = t.closest && t.closest('dialog');
          if (!dlg) { return; }
          // A dialog-method submit navigates NOWHERE — it closes, with the button's value.
          ev.preventDefault();
          dlg.close(t.getAttribute('value') || '');
        };
        // Escape dismisses the topmost modal, firing a cancelable `cancel` first — the hook every
        // "are you sure you want to discard?" guard hangs off.
        var __dialogEscape = function(ev) {
          if (ev.key !== 'Escape' && ev.keyCode !== 27) { return; }
          var dlg = __modalStack[__modalStack.length - 1];
          if (!dlg || !dlg.hasAttribute('open')) { return; }
          var ok = true;
          try { ok = dlg.dispatchEvent(new Event('cancel', { bubbles: false, cancelable: true })); } catch (e) {}
          if (ok !== false) { dlg.close(); }
        };
        try {
          document.addEventListener('click', __dialogSubmit, true);
          document.addEventListener('keydown', __dialogEscape, true);
        } catch (e) {}
      }

      // ── The `popover` attribute API — `showPopover()` / `hidePopover()` / `togglePopover()`,
      // `popovertarget`, `beforetoggle`/`toggle`, and light dismiss. The other half of the top layer
      // (see the `<dialog>` block above): every menu, tooltip, dropdown, toast and select-listbox that
      // has stopped being a hand-rolled `<div class="dropdown">` is a popover, and the entire surface
      // was absent — so `showPopover()` was a TypeError AND, with no `[popover]` UA rule, the menu's
      // items were rendering inline in the middle of the page before anyone opened it.
      //
      // `data-manuk-popover-open` IS the `:popover-open` state: the UA sheet keys its `display` off it
      // and `Page::z_index_map` reads it for the top-layer promotion, so the flag has to live on the
      // element where both sides can see it (same reasoning as `data-manuk-modal`).
      if (__HP && typeof __HP.showPopover === 'undefined') {
        var __popType = function(el) {
          // `popover` / `popover=""` / `popover="auto"` are auto; anything else that is not `manual`
          // is invalid — the attribute is an enumerated one, and `auto` is its invalid-value default.
          if (!el || !el.hasAttribute || !el.hasAttribute('popover')) { return null; }
          var v = String(el.getAttribute('popover') || '').toLowerCase();
          return v === 'manual' ? 'manual' : 'auto';
        };
        var __popOpen = function(el) { return el.hasAttribute('data-manuk-popover-open'); };
        // `beforetoggle` is cancelable on the way OPEN (a guard can veto); `toggle` is the
        // notification after the fact. Both carry oldState/newState, which is what handlers switch on.
        var __popToggleEvent = function(el, name, oldState, newState, cancelable) {
          var ev;
          try { ev = new Event(name, { bubbles: false, cancelable: !!cancelable }); }
          catch (e) { return true; }
          ev.oldState = oldState;
          ev.newState = newState;
          try { return el.dispatchEvent(ev); } catch (e) { return true; }
        };
        __HP.showPopover = function() {
          if (__popType(this) === null || __popOpen(this)) { return; }
          if (__popToggleEvent(this, 'beforetoggle', 'closed', 'open', true) === false) { return; }
          // An `auto` popover is exclusive with the other auto popovers: opening one closes the rest.
          // (Nesting — a submenu inside its parent menu — is residue; this is the flat common case.)
          if (__popType(this) === 'auto') {
            var others = document.querySelectorAll('[data-manuk-popover-open]');
            for (var i = 0; i < others.length; i++) {
              if (others[i] !== this && __popType(others[i]) === 'auto') { others[i].hidePopover(); }
            }
          }
          this.setAttribute('data-manuk-popover-open', '');
          __popToggleEvent(this, 'toggle', 'closed', 'open', false);
        };
        __HP.hidePopover = function() {
          if (__popType(this) === null || !__popOpen(this)) { return; }
          if (__popToggleEvent(this, 'beforetoggle', 'open', 'closed', true) === false) { return; }
          this.removeAttribute('data-manuk-popover-open');
          __popToggleEvent(this, 'toggle', 'open', 'closed', false);
        };
        __HP.togglePopover = function(force) {
          var want = (force === undefined) ? !__popOpen(this) : !!force;
          if (want) { this.showPopover(); } else { this.hidePopover(); }
          return __popOpen(this);
        };
        // `el.popover` reflects the attribute (`null` when absent) — feature detection reads exactly
        // this: `'popover' in HTMLElement.prototype`.
        Object.defineProperty(__HP, 'popover', {
          configurable: true,
          get: function() { return __popType(this); },
          set: function(v) {
            if (v == null) { this.removeAttribute('popover'); } else { this.setAttribute('popover', String(v)); }
          }
        });
        // `<button popovertarget="menu">` — declarative, no script at all. This is how the API is
        // meant to be used, and the whole point of it shipping.
        var __popClick = function(ev) {
          var t = ev.target;
          while (t && !(t.getAttribute && t.getAttribute('popovertarget'))) { t = t.parentElement; }
          if (t) {
            var target = document.getElementById(t.getAttribute('popovertarget'));
            if (target) {
              var action = String(t.getAttribute('popovertargetaction') || 'toggle').toLowerCase();
              if (action === 'show') { target.showPopover(); }
              else if (action === 'hide') { target.hidePopover(); }
              else { target.togglePopover(); }
              return;   // the invoker's own click never light-dismisses the popover it just opened
            }
          }
          // **Light dismiss**: a click anywhere outside an open `auto` popover closes it. Without
          // this a menu opens and never closes, because the page is relying on the UA for it.
          var open = document.querySelectorAll('[data-manuk-popover-open]');
          for (var i = 0; i < open.length; i++) {
            var p = open[i];
            if (__popType(p) !== 'auto') { continue; }
            var inside = false;
            for (var n = ev.target; n; n = n.parentElement) { if (n === p) { inside = true; break; } }
            if (!inside) { p.hidePopover(); }
          }
        };
        // Escape light-dismisses `auto` popovers too (the dialog handler above owns modals).
        var __popEscape = function(ev) {
          if (ev.key !== 'Escape' && ev.keyCode !== 27) { return; }
          var open = document.querySelectorAll('[data-manuk-popover-open]');
          for (var i = open.length - 1; i >= 0; i--) {
            if (__popType(open[i]) === 'auto') { open[i].hidePopover(); }
          }
        };
        try {
          document.addEventListener('click', __popClick, true);
          document.addEventListener('keydown', __popEscape, true);
        } catch (e) {}
        // **Feature detection reads `HTMLElement.prototype`, and ours is not `__protoHTMLElement`.**
        // The custom-elements shim gives the `HTMLElement` constructor a fresh `{}` prototype on
        // purpose (upgrade grafts methods onto the host object, since a reflector's prototype cannot
        // be swapped) — so `'popover' in HTMLElement.prototype`, which is the canonical detection for
        // this whole API, was FALSE while every element in the page had the members. Mirror the four
        // descriptors onto the constructor's prototype so both reads agree. They are functions of
        // `this`, so calling one through either object behaves identically.
        // (Residue: the two prototypes being different objects at all is a broader divergence than
        // this tick — every `'x' in HTMLElement.prototype` detection has the same blind spot.)
        try {
          var __ctorProto = globalThis.HTMLElement && globalThis.HTMLElement.prototype;
          if (__ctorProto && __ctorProto !== __HP) {
            ['showPopover', 'hidePopover', 'togglePopover', 'popover',
             'show', 'showModal', 'close', 'returnValue'].forEach(function(k) {
              var d = Object.getOwnPropertyDescriptor(__HP, k);
              if (d && !Object.getOwnPropertyDescriptor(__ctorProto, k)) {
                Object.defineProperty(__ctorProto, k, d);
              }
            });
          }
        } catch (e) {}
      }

      // `CSS.escape` / `CSS.supports` — feature detection, and the correct way to build a selector.
      if (typeof globalThis.CSS === 'undefined') {
        globalThis.CSS = {
          escape: function(v) { return String(v).replace(/([^\w-])/g, '\\$1'); },
          supports: function() { return true; }
        };
      }

      if (typeof globalThis.CSSStyleSheet === 'undefined') {
        globalThis.CSSStyleSheet = function CSSStyleSheet() {
          this.cssRules = [];
          this.rules = this.cssRules;
        };
        globalThis.CSSStyleSheet.prototype.replaceSync = function(text) { this._text = String(text); };
        globalThis.CSSStyleSheet.prototype.replace = function(text) {
          this._text = String(text);
          return Promise.resolve(this);
        };
        globalThis.CSSStyleSheet.prototype.insertRule = function(rule, index) {
          this.cssRules.splice(index === undefined ? this.cssRules.length : index, 0, { cssText: rule });
          return index || 0;
        };
        globalThis.CSSStyleSheet.prototype.deleteRule = function(i) { this.cssRules.splice(i, 1); };
      }
      if (typeof document !== 'undefined' && !('adoptedStyleSheets' in document)) {
        try { document.adoptedStyleSheets = []; } catch (e) {}
      }
      if (typeof globalThis.CSSStyleDeclaration === 'undefined') {
        globalThis.CSSStyleDeclaration = function CSSStyleDeclaration(){};
      }

      if (typeof globalThis.NodeFilter === 'undefined') {
        globalThis.NodeFilter = {
          FILTER_ACCEPT: 1, FILTER_REJECT: 2, FILTER_SKIP: 3,
          SHOW_ALL: 0xFFFFFFFF, SHOW_ELEMENT: 1, SHOW_TEXT: 4, SHOW_COMMENT: 128
        };
      }
      if (typeof document !== 'undefined' && typeof document.createTreeWalker !== 'function') {
        document.createTreeWalker = function(root, whatToShow, filter) {
          if (whatToShow === undefined || whatToShow === null) whatToShow = 0xFFFFFFFF;
          var show = function(n) {
            var t = n.nodeType;
            var bit = t === 1 ? 1 : (t === 3 ? 4 : (t === 8 ? 128 : 0));
            return (whatToShow & bit) !== 0;
          };
          var verdict = function(n) {
            if (!filter) return 1;
            var f = (typeof filter === 'function') ? filter : filter.acceptNode;
            if (typeof f !== 'function') return 1;
            return f.call(filter, n) || 1;
          };
          var w = {
            root: root, currentNode: root, whatToShow: whatToShow, filter: filter,
            nextNode: function() {
              var n = this.currentNode;
              while (true) {
                // Depth-first, in document order — the order a template's holes are numbered in.
                var next = n.firstChild;
                if (!next) {
                  var c = n;
                  while (c && c !== this.root && !c.nextSibling) { c = c.parentNode; }
                  next = (c && c !== this.root) ? c.nextSibling : null;
                }
                if (!next) { return null; }
                n = next;
                this.currentNode = n;
                if (show(n) && verdict(n) === 1) { return n; }
                // FILTER_REJECT/SKIP: keep walking. (A true REJECT should skip the subtree; treating
                // it as SKIP over-visits but never under-visits, and under-visiting is what loses
                // bindings.)
              }
            },
            parentNode: function() {
              var p = this.currentNode && this.currentNode.parentNode;
              if (p && p !== this.root.parentNode) { this.currentNode = p; return p; }
              return null;
            },
            firstChild: function() {
              var c = this.currentNode && this.currentNode.firstChild;
              if (c) { this.currentNode = c; return c; }
              return null;
            },
            nextSibling: function() {
              var s = this.currentNode && this.currentNode.nextSibling;
              if (s) { this.currentNode = s; return s; }
              return null;
            }
          };
          return w;
        };
      }

      if (typeof globalThis.requestIdleCallback === 'undefined') {
        globalThis.requestIdleCallback = function(cb){
          return setTimeout(function(){ cb({ didTimeout: false, timeRemaining: function(){ return 5; } }); }, 0);
        };
        globalThis.cancelIdleCallback = function(){};
      }
      // ── **A THROWING TASK MUST NOT KILL THE EVENT LOOP.** ──────────────────────────────────
      //
      // It did. `NEXT_TASK` called `t()` bare; the exception propagated out of the eval, the Rust
      // `?` on it aborted `run()`, and **every task queued after the throwing one never ran.** One
      // bad `setTimeout` callback silently stopped the page's clock.
      //
      // The spec says: **report the exception, then keep going.** The loop is not allowed to care.
      // A real browser fires `window.onerror` / an `error` event and takes the next task.
      //
      // Found by WPT: dozens of `dom/nodes` files reported NOTHING, because a test threw, the loop
      // died, and `testharness.js`'s completion callback — itself a queued task — never ran. The
      // score looked like "we fail these tests". The truth was "we stopped running".
      //
      // `__errors` is deliberately kept: it is the storage the **unhandled-error harvester** wants,
      // and it means a page's silent breakage is now a thing that can be READ OUT rather than
      // guessed at.
      globalThis.__errors = [];
      globalThis.__reportError = function (e) {
        try {
          var msg = String((e && e.message) ? e.message : e);
          __errors.push(String((e && e.stack) ? e.stack : msg));
          if (typeof globalThis.onerror === 'function') {
            try { globalThis.onerror(msg, '', 0, 0, e); } catch (x) {}
          }
          if (typeof globalThis.dispatchEvent === 'function') {
            var ev;
            try { ev = new ErrorEvent('error', { message: msg, error: e }); }
            catch (x) { ev = { type: 'error', message: msg, error: e }; }
            try { globalThis.dispatchEvent(ev); } catch (x) {}
          }
        } catch (x) { /* reporting must never itself throw — that would kill the loop again */ }
      };

      // ── **`DOMException` — it did not exist at all.** ───────────────────────────────────────
      //
      // Every DOM method that is specified to *fail* fails by throwing one of these. Without it,
      // `e instanceof DOMException` is a ReferenceError, and WPT's `assert_throws_dom` — which a
      // very large fraction of `dom/` uses — cannot even express its assertion.
      //
      // **Defining it does NOT make our methods throw.** They still do not, and the tests that
      // demand a throw will now honestly FAIL rather than error out the whole file. That is the
      // point: the failures become a *work list* (which methods must validate their arguments)
      // instead of a wall of NO_REPORT that hides everything behind it.
      if (typeof globalThis.DOMException === 'undefined') {
        var DOM_CODES = {
          IndexSizeError: 1, HierarchyRequestError: 3, WrongDocumentError: 4,
          InvalidCharacterError: 5, NoModificationAllowedError: 7, NotFoundError: 8,
          NotSupportedError: 9, InUseAttributeError: 10, InvalidStateError: 11,
          SyntaxError: 12, InvalidModificationError: 13, NamespaceError: 14,
          InvalidAccessError: 15, TypeMismatchError: 17, SecurityError: 18, NetworkError: 19,
          AbortError: 20, URLMismatchError: 21, QuotaExceededError: 22, TimeoutError: 23,
          InvalidNodeTypeError: 24, DataCloneError: 25
        };
        var DE = function DOMException(message, name) {
          var e = Error.call(this, message);
          this.message = message === undefined ? '' : String(message);
          this.name = name === undefined ? 'Error' : String(name);
          this.code = DOM_CODES[this.name] || 0;
          if (e.stack) { this.stack = e.stack; }
        };
        DE.prototype = Object.create(Error.prototype);
        DE.prototype.constructor = DE;
        DE.prototype.toString = function () { return this.name + ': ' + this.message; };
        Object.keys(DOM_CODES).forEach(function (n) {
          Object.defineProperty(DE, n, { value: DOM_CODES[n] });
        });
        // The LEGACY numeric code constants (`DOMException.INDEX_SIZE_ERR` …). Code checks
        // `e.code === DOMException.NOT_FOUND_ERR` constantly, and WPT asserts them directly; absent, that
        // comparison is `=== undefined` → silently wrong. On both the constructor and the prototype.
        var DE_LEGACY = {
          INDEX_SIZE_ERR:1, DOMSTRING_SIZE_ERR:2, HIERARCHY_REQUEST_ERR:3, WRONG_DOCUMENT_ERR:4,
          INVALID_CHARACTER_ERR:5, NO_DATA_ALLOWED_ERR:6, NO_MODIFICATION_ALLOWED_ERR:7, NOT_FOUND_ERR:8,
          NOT_SUPPORTED_ERR:9, INUSE_ATTRIBUTE_ERR:10, INVALID_STATE_ERR:11, SYNTAX_ERR:12,
          INVALID_MODIFICATION_ERR:13, NAMESPACE_ERR:14, INVALID_ACCESS_ERR:15, VALIDATION_ERR:16,
          TYPE_MISMATCH_ERR:17, SECURITY_ERR:18, NETWORK_ERR:19, ABORT_ERR:20, URL_MISMATCH_ERR:21,
          QUOTA_EXCEEDED_ERR:22, TIMEOUT_ERR:23, INVALID_NODE_TYPE_ERR:24, DATA_CLONE_ERR:25
        };
        Object.keys(DE_LEGACY).forEach(function (n) {
          Object.defineProperty(DE, n, { value: DE_LEGACY[n], enumerable: true });
          Object.defineProperty(DE.prototype, n, { value: DE_LEGACY[n], enumerable: true });
        });
        globalThis.DOMException = DE;
      }

      // ── **THE INERT INTERFACE SURFACE, INSTALLED LAST.** ────────────────────────────────────
      // Everything real has had its chance to define itself by now. Whatever is still `undefined`
      // gets a named, inert constructor so that referencing it is not a ReferenceError.
      //
      // **Last is the whole point.** Installed first (as it was until the WPT tick), a stub here
      // shadowed the real `AbortSignal` defined further down and silently disabled it. See the long
      // comment at the list itself.
      __inertNames.forEach(function (n) {
        if (typeof globalThis[n] === 'undefined') {
          var C = function () {};
          Object.defineProperty(C, 'name', { value: n });
          C.prototype = {};
          globalThis[n] = C;
        }
      });
    })();

"#;

/// Run the next macrotask if any; report whether one ran.
const NEXT_TASK: &str = "(function(){ \
     var bi = -1; \
     for (var i = 0; i < __tasks.length; i++) { \
       var a = __tasks[i]; \
       if (a.w > __timeBudget) { continue; } \
       if (bi < 0) { bi = i; continue; } \
       var b = __tasks[bi]; \
       if (a.w < b.w || (a.w === b.w && a.s < b.s)) { bi = i; } \
     } \
     if (bi < 0) return false; \
     var t = __tasks.splice(bi, 1)[0]; \
     if (t.w > __now) { __now = t.w; } \
     try { t.f(); } catch (e) { __reportError(e); } \
     return true; })()";

/// Drain the microtask queue completely (microtasks may enqueue more microtasks).
const DRAIN_MICRO: &str = "(function(){ \
     while (__micro.length) { var m = __micro.shift(); \
       try { m(); } catch (e) { __reportError(e); } } })()";

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
    eval(rt, global, PRELUDE, "event_loop_prelude.js")?;

    // **AFTER the prelude, never before.** The prelude's inert-interface list creates a stub `Range`, so
    // installing the real one first would have it immediately overwritten by a do-nothing constructor —
    // and `typeof Range === 'function'` would still be true, which is exactly why nobody noticed the stub
    // for sixty ticks. This is the same ordering bug that once let a stub `AbortSignal` shadow the real
    // one; the lesson is cheap to re-learn and expensive to miss: **a stub satisfies every check that only
    // asks whether a name exists.**
    eval(rt, global, crate::range_js::RANGE_JS, "range.js")?;

    // Also after the prelude — it REPLACES the plain-object `createTreeWalker` shim installed there. If
    // it ran first the shim's `typeof document.createTreeWalker !== 'function'` guard would see a real
    // function, decline to install, and leave the real one in place — which happens to be right, and is
    // right by accident. Ordering that works by luck is ordering that breaks on the next edit.
    eval(
        rt,
        global,
        crate::traversal_js::TRAVERSAL_JS,
        "traversal.js",
    )?;

    // Live collections, LAST — it WRAPS `children` / `getElementsByTagName` and friends, so everything it
    // wraps must already exist. It works at all only because tick 64 gave the DOM real prototypes: before
    // that, `children` was an own-property of every element and patching the prototype did nothing,
    // silently.
    eval(
        rt,
        global,
        crate::collections_js::COLLECTIONS_JS,
        "collections.js",
    )?;

    // `Attr` / `NamedNodeMap`. It wraps the element prototype too.
    eval(rt, global, crate::attrs_js::ATTRS_JS, "attrs.js")?;

    // **HTML attribute reflection** — ~38,000 WPT subtests, and how ordinary page code touches the DOM.
    // After `attrs.js`, because it is built on `setAttribute`/`getAttribute`/`hasAttribute`; before
    // `mutation.js`, so that a reflected write (`input.disabled = true`) goes through the wrapped
    // `setAttribute` and is therefore OBSERVED. Install it after the observer and every reflected
    // mutation becomes invisible to MutationObserver — silently.
    {
        let table = format!(
            "globalThis.__REFLECT_TABLE = {};",
            serde_json::to_string(crate::reflect_table::REFLECT_TABLE)
                .unwrap_or_else(|_| "\"\"".to_string())
        );
        eval(rt, global, &table, "reflect_table.js")?;
        eval(rt, global, crate::reflect_js::REFLECT_JS, "reflect.js")?;
    }

    // `MutationObserver` LAST of all: it wraps the mutating methods, and it must wrap the FINAL versions
    // of them — including the ones the collections and attrs layers have already replaced. Install it
    // earlier and it observes a method that is later swapped out from under it, so a page's mutations
    // stop being reported and nothing says so.
    eval(rt, global, crate::mutation_js::MUTATION_JS, "mutation.js")?;
    // AFTER reflection: `contentDocument` must not collide with a reflected accessor, and reflect.js
    // skips any IDL name already `in proto`.
    eval(rt, global, crate::iframe_js::IFRAME_JS, "iframe.js")?;
    // **MSE.** After the prelude (it needs `setTimeout` and `DOMException`, and it must land AFTER
    // the inert-name sweep rather than be overwritten by it) and after `dom_bindings`' `install`,
    // which is where `URL` comes from — `URL.createObjectURL` is the whole attachment channel, so
    // installing this before `URL` exists would silently drop it and leave `video.src` unable to
    // ever name a MediaSource.
    eval(rt, global, crate::mse_js::MSE_JS, "mse.js")?;
    eval(
        rt,
        global,
        crate::inline_handlers_js::INLINE_HANDLERS_JS,
        "inline_handlers.js",
    )?;
    // **Wire the statically-parsed inline handlers NOW, before any inline `<script>` runs.**
    //
    // `<button onclick=...>` must be live the instant the element exists, not only at DOMContentLoaded:
    // a script lower in the same document routinely dispatches to a button parsed above it. The DOM is
    // fully parsed by the time bindings install, so a single pass here catches every static handler;
    // the DCL and load passes then pick up anything a script adds later. Idempotent (per-node mark).
    eval(
        rt,
        global,
        "globalThis.__wireInlineHandlers && __wireInlineHandlers()",
        "wire_inline.js",
    )
    .map(|_| ())
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
            // **A runaway task chain must not hang the browser (Bar 0).**
            //
            // This loop drains to quiescence, which is correct — right up until a page schedules work
            // that reschedules itself. `setInterval(fn, 0)` is the obvious one, and it is on carousels,
            // clocks and pollers all over the web; a self-reposting `requestAnimationFrame` is another.
            // Without a ceiling, "drain to quiescence" means "never return", and the tab is gone with no
            // recourse — which is precisely the failure Bar 0 exists to forbid.
            //
            // The ceiling is deliberately generous: a real page's load-time task chain is tens of
            // tasks, not tens of thousands. Crossing it means the page is not converging, and the right
            // answer is to render what we have rather than to keep spinning forever.
            if count >= MAX_TASKS_PER_DRAIN {
                tracing::warn!(
                    count,
                    "event loop hit its task ceiling — the page is not converging (a self-rescheduling \
                     timer, most likely). Painting what we have. The alternative is a frozen tab."
                );
                break;
            }
            continue;
        }
        break;
    }
    microtask_checkpoint(rt, global)?;
    Ok(count)
}

/// Parse a flattened `"name\x02value\x02name\x02value"` header string into pairs. An odd trailing
/// element (a name with no value) is dropped rather than paired with garbage.
fn parse_headers(s: &str) -> Vec<(String, String)> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split('\u{2}')
        .collect::<Vec<_>>()
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0].to_string(), c[1].to_string()))
        .collect()
}

/// Drain the page's queued `fetch`/XHR requests, returning `(id, url, method, headers, body)` for
/// each so the host can perform them over the real network. Kind (`fetch` vs XHR) is intentionally
/// dropped — [`deliver`] settles by id regardless.
pub fn drain_pending(
    rt: &mut Runtime,
    global: mozjs::rust::HandleObject,
) -> Result<Vec<(u32, String, String, Vec<(String, String)>, String)>, String> {
    let mut out = Vec::new();
    while let Some(req) = eval_string(rt, global, NEXT_PENDING, "event_loop_drain.js")? {
        // "id\x01kind\x01method\x01url\x01headers\x01body" — body is the greedy tail (parts[5]).
        let parts: Vec<&str> = req.splitn(6, '\u{1}').collect();
        if parts.len() < 4 {
            continue;
        }
        let id: u32 = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let method = parts[2].to_string();
        let url = parts[3].to_string();
        let headers = parse_headers(parts.get(4).copied().unwrap_or(""));
        let body = parts.get(5).map(|s| s.to_string()).unwrap_or_default();
        out.push((id, url, method, headers, body));
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
    headers: &[(String, String)],
) -> Result<(), String> {
    let script = format!(
        "__deliver({}, {}, {}, {})",
        id,
        status,
        js_string_literal(body),
        js_headers_literal(headers)
    );
    eval(rt, global, &script, "event_loop_deliver.js").map(|_| ())
}

/// Drain what the page's WebSockets asked for since the last call, each paired with its socket id.
pub fn drain_ws_ops(
    rt: &mut Runtime,
    global: mozjs::rust::HandleObject,
) -> Result<Vec<(u32, crate::WsOp)>, String> {
    let mut out = Vec::new();
    loop {
        let Some(rec) = eval_string(
            rt,
            global,
            "(function(){ return __wsOps.length ? __wsOps.shift() : null; })()",
            "ws_ops.js",
        )?
        else {
            break;
        };
        let parts: Vec<&str> = rec.splitn(4, '\u{1}').collect();
        if parts.len() < 3 {
            continue;
        }
        let Ok(id) = parts[0].parse::<u32>() else {
            continue;
        };
        let op = match parts[1] {
            "c" => crate::WsOp::Connect {
                url: parts[2].to_string(),
                protocols: parts
                    .get(3)
                    .map(|p| {
                        p.split(',')
                            .map(str::trim)
                            .filter(|p| !p.is_empty())
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
            },
            // Payload is one char per byte (see the `send` shim), so the low byte of each code
            // unit is the byte. Not `as_bytes()`: that would UTF-8-encode 0x80..0xFF into two
            // bytes each and corrupt every binary frame.
            k @ ("s" | "b") => crate::WsOp::Send {
                data: parts[2].chars().map(|c| (c as u32 & 0xff) as u8).collect(),
                binary: k == "b",
            },
            "x" => crate::WsOp::Close {
                code: parts[2].parse::<u16>().ok(),
                reason: parts.get(3).unwrap_or(&"").to_string(),
            },
            _ => continue,
        };
        out.push((id, op));
    }
    Ok(out)
}

/// Deliver one [`WsEvent`] to socket `id`.
pub fn deliver_ws_event(
    rt: &mut Runtime,
    global: mozjs::rust::HandleObject,
    id: u32,
    event: &crate::WsEvent,
) -> Result<(), String> {
    let script = match event {
        crate::WsEvent::Open {
            protocol,
            extensions,
        } => format!(
            "__wsOpen({}, {}, {})",
            id,
            js_string_literal(protocol),
            js_string_literal(extensions)
        ),
        crate::WsEvent::Message { data, binary } => format!(
            "__wsMessage({}, {}, {})",
            id,
            if *binary {
                js_bytes_literal(data)
            } else {
                js_string_literal(&String::from_utf8_lossy(data))
            },
            binary
        ),
        crate::WsEvent::Sent { bytes } => format!("__wsSent({}, {})", id, bytes),
        crate::WsEvent::Error { message } => {
            format!("__wsError({}, {})", id, js_string_literal(message))
        }
        crate::WsEvent::Close {
            code,
            reason,
            clean,
        } => format!(
            "__wsClose({}, {}, {}, {})",
            id,
            code,
            js_string_literal(reason),
            clean
        ),
    };
    eval(rt, global, &script, "ws_event.js").map(|_| ())
}

/// Settle request `id` at its RESPONSE HEADERS, with a body that is still arriving. This is where a
/// real `fetch()` promise resolves — not at the end of the body — so the page gets its `Response`,
/// takes a reader off `response.body`, and pumps while the rest is still on the wire.
///
/// `status == 0` rejects, exactly as [`deliver`] does. Follow with [`deliver_chunk`] per piece and
/// [`deliver_end`] once.
pub fn deliver_head(
    rt: &mut Runtime,
    global: mozjs::rust::HandleObject,
    id: u32,
    status: u16,
    headers: &[(String, String)],
) -> Result<(), String> {
    let script = format!(
        "__deliverHead({}, {}, {})",
        id,
        status,
        js_headers_literal(headers)
    );
    eval(rt, global, &script, "event_loop_deliver_head.js").map(|_| ())
}

/// Feed one body chunk to request `id`'s open response stream. Unknown ids are a no-op (the page may
/// have aborted, or already been torn down).
pub fn deliver_chunk(
    rt: &mut Runtime,
    global: mozjs::rust::HandleObject,
    id: u32,
    bytes: &[u8],
) -> Result<(), String> {
    let script = format!("__deliverChunk({}, {})", id, js_bytes_literal(bytes));
    eval(rt, global, &script, "event_loop_deliver_chunk.js").map(|_| ())
}

/// Close request `id`'s response stream — the page's pump loop sees `{done: true}`.
pub fn deliver_end(
    rt: &mut Runtime,
    global: mozjs::rust::HandleObject,
    id: u32,
) -> Result<(), String> {
    let script = format!("__deliverEnd({})", id);
    eval(rt, global, &script, "event_loop_deliver_end.js").map(|_| ())
}

/// Serialize raw body bytes as a JS string literal of **one char per byte** — the same convention the
/// binary-upload path uses. Every byte becomes an explicit `\u00NN` escape, so the result is pure
/// ASCII source and cannot be mangled by any encoding step between here and the engine; the page side
/// (`__bytesFromLatin1`) reads it back with `charCodeAt(i) & 0xff`.
///
/// **Deliberately NOT `String::from_utf8_lossy` + a string literal.** A chunk boundary can fall in the
/// middle of a multi-byte UTF-8 sequence — which is the normal case for a stream, not an edge one —
/// and lossy decoding would replace the split character with U+FFFD, silently corrupting the body.
/// Bytes stay bytes until the page's own `TextDecoder` reassembles them.
fn js_bytes_literal(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 6 + 2);
    s.push('"');
    for b in bytes {
        s.push_str(&format!("\\u{:04x}", b));
    }
    s.push('"');
    s
}

/// Serialize response headers as a JS array-of-pairs literal — `[["content-type","..."], …]` — for
/// embedding in a `__deliver` call so the page's `Response.headers`/`XHR.getResponseHeader` read the
/// server's real fields. Each name and value is escaped as a JS string literal.
fn js_headers_literal(headers: &[(String, String)]) -> String {
    let mut s = String::from("[");
    for (i, (k, v)) in headers.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push('[');
        s.push_str(&js_string_literal(k));
        s.push(',');
        s.push_str(&js_string_literal(v));
        s.push(']');
    }
    s.push(']');
    s
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
            // "id\x01kind\x01method\x01url\x01headers\x01body" (this loop's mock fetcher ignores
            // headers/body; the live host path in `drain_pending` replays them onto the wire).
            let parts: Vec<&str> = req.splitn(6, '\u{1}').collect();
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

    // Run isolated:
    //   cargo test -p manuk-js --features spidermonkey request_headers -- --ignored
    #[test]
    #[ignore = "SpiderMonkey multi-Runtime-per-process teardown; run in isolation"]
    fn fetch_and_xhr_carry_request_headers() {
        let handle = crate::spidermonkey::engine_handle().expect("engine");
        let mut runtime = Runtime::new(handle);
        let options = RealmOptions::default();
        rooted!(&in(runtime.cx()) let global = unsafe {
            JS_NewGlobalObject(runtime.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook, &*options)
        });
        let _ar = mozjs::jsapi::JSAutoRealm::new(unsafe { runtime.cx().raw_cx() }, global.get());
        install(&mut runtime, global.handle()).expect("install event loop");

        // A POST fetch with an object of headers, and a GET XHR built via setRequestHeader — the two
        // request shapes that dropped `Authorization` (fetch) and custom headers (XHR) before.
        let user = r#"
            fetch("http://h/api", {method:"POST", headers:{Authorization:"Bearer T", "X-A":"1"}, body:"payload"});
            var x = new XMLHttpRequest();
            x.open("GET", "http://h/data");
            x.setRequestHeader("X-Custom", "zed");
            x.send();
        "#;
        eval(&mut runtime, global.handle(), user, "user.js").expect("user script");

        let reqs = drain_pending(&mut runtime, global.handle()).expect("drain");
        // reqs: (id, url, method, headers, body)
        let post = reqs
            .iter()
            .find(|r| r.2 == "POST")
            .expect("the POST fetch was queued");
        assert!(
            post.3
                .iter()
                .any(|(k, v)| k == "Authorization" && v == "Bearer T"),
            "fetch's Authorization header must reach the host, got {:?}",
            post.3
        );
        assert!(
            post.3.iter().any(|(k, v)| k == "X-A" && v == "1"),
            "fetch's custom header must reach the host, got {:?}",
            post.3
        );
        assert_eq!(post.4, "payload", "the POST body must still travel");

        let get = reqs
            .iter()
            .find(|r| r.2 == "GET")
            .expect("the GET xhr was queued");
        assert!(
            get.3.iter().any(|(k, v)| k == "X-Custom" && v == "zed"),
            "XHR setRequestHeader must reach the host, got {:?}",
            get.3
        );
    }
}
