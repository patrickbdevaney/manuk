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
        // `pipeTo(writable)` — drain this stream into a WritableStream, chunk by chunk, resolving when
        // the source closes (and the destination is closed). This is how `response.body.pipeTo(sink)`
        // and the tail of every `pipeThrough` chain actually move bytes.
        RS.prototype.pipeTo = function(dest) {
            var reader = this.getReader();
            var writer = dest.getWriter();
            var pump = function() {
                return reader.read().then(function(step) {
                    if (step.done) { return writer.close(); }
                    return Promise.resolve(writer.write(step.value)).then(pump);
                });
            };
            return pump().then(null, function(e) {
                try { writer.abort(e); } catch (x) {}
                throw e;
            });
        };
        // `pipeThrough(transform)` — feed this stream into `transform.writable` and hand back
        // `transform.readable`, so `body.pipeThrough(new TextDecoderStream())` returns a decoded stream.
        RS.prototype.pipeThrough = function(transform) {
            this.pipeTo(transform.writable);
            return transform.readable;
        };
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

    // `WritableStream` — the write half of the streams API, and it was an INERT NAME (`typeof` said
    // 'function' but `new WritableStream(...).getWriter` was undefined, so any real use threw). A
    // `fetch` upload body, a `pipeTo` destination, a file/socket sink all need a working one. This is
    // a real implementation over the underlying-sink protocol: `getWriter()` hands back a writer whose
    // `write(chunk)` actually delivers the chunk to `sink.write`, `close()` to `sink.close`, `abort()`
    // to `sink.abort`. (Backpressure is simplified — `ready`/`desiredSize` are always ready; the honest
    // limit noted in the wiki. The DELIVERY of chunks, which is the point, is real.)
    if (typeof globalThis.WritableStream === 'undefined') {
        var WS = function WritableStream(sink) {
            this.__sink = sink || {};
            this.__locked = false;
            this.__state = 'writable';   // 'writable' | 'closed' | 'errored'
            this.__err = null;
            var self = this;
            this.__controller = { error: function(e) { self.__state = 'errored'; self.__err = e; }, signal: undefined };
            if (typeof this.__sink.start === 'function') {
                try { this.__sink.start(this.__controller); }
                catch (e) { this.__state = 'errored'; this.__err = e; }
            }
        };
        WS.prototype.getWriter = function() {
            if (this.__locked) { throw new TypeError('WritableStream is locked to a writer'); }
            this.__locked = true;
            return new globalThis.WritableStreamDefaultWriter(this);
        };
        WS.prototype.abort = function(reason) {
            this.__state = 'errored'; this.__err = reason;
            if (typeof this.__sink.abort === 'function') { try { return Promise.resolve(this.__sink.abort(reason)); } catch (e) { return Promise.reject(e); } }
            return Promise.resolve(undefined);
        };
        WS.prototype.close = function() {
            this.__state = 'closed';
            if (typeof this.__sink.close === 'function') { try { return Promise.resolve(this.__sink.close()); } catch (e) { return Promise.reject(e); } }
            return Promise.resolve(undefined);
        };
        Object.defineProperty(WS.prototype, 'locked', {
            get: function() { return this.__locked; }, enumerable: true, configurable: true
        });
        globalThis.WritableStream = WS;

        var WSW = function WritableStreamDefaultWriter(stream) { this.__s = stream; };
        WSW.prototype.write = function(chunk) {
            var s = this.__s;
            if (!s) { return Promise.reject(new TypeError('writer has been released')); }
            if (s.__state === 'errored') { return Promise.reject(s.__err); }
            try {
                var p = (typeof s.__sink.write === 'function') ? s.__sink.write(chunk, s.__controller) : undefined;
                return Promise.resolve(p);
            } catch (e) { s.__state = 'errored'; s.__err = e; return Promise.reject(e); }
        };
        WSW.prototype.close = function() { return this.__s ? this.__s.close() : Promise.reject(new TypeError('released')); };
        WSW.prototype.abort = function(reason) { return this.__s ? this.__s.abort(reason) : Promise.resolve(undefined); };
        WSW.prototype.releaseLock = function() { if (this.__s) { this.__s.__locked = false; this.__s = null; } };
        Object.defineProperty(WSW.prototype, 'ready', { get: function() { return Promise.resolve(undefined); }, enumerable: true, configurable: true });
        Object.defineProperty(WSW.prototype, 'closed', { get: function() { return Promise.resolve(undefined); }, enumerable: true, configurable: true });
        Object.defineProperty(WSW.prototype, 'desiredSize', { get: function() { return 1; }, enumerable: true, configurable: true });
        globalThis.WritableStreamDefaultWriter = WSW;
    }

    // `TransformStream` — the middle of a stream pipeline (`body.pipeThrough(ts)`), and it too was an
    // INERT NAME (`new TransformStream(...).readable` was undefined). Now a real one over the real
    // ReadableStream + WritableStream above: a chunk written to `.writable` is passed to the
    // transformer's `transform(chunk, controller)`, which `controller.enqueue()`s onto `.readable`; a
    // transformer with no `transform` is the identity stream. This is what makes `TextDecoderStream`-
    // style wrappers and `pipeThrough` chains actually move (and reshape) data.
    if (typeof globalThis.TransformStream === 'undefined') {
        var TS = function TransformStream(transformer) {
            transformer = transformer || {};
            var rsCtl = null;
            var readable = new globalThis.ReadableStream({ start: function(c) { rsCtl = c; } });
            var tctl = {
                enqueue: function(chunk) { rsCtl.enqueue(chunk); },
                terminate: function() { rsCtl.close(); },
                error: function(e) { rsCtl.error(e); }
            };
            var writable = new globalThis.WritableStream({
                write: function(chunk) {
                    if (typeof transformer.transform === 'function') { return transformer.transform(chunk, tctl); }
                    tctl.enqueue(chunk);   // identity transform
                },
                close: function() {
                    var p = (typeof transformer.flush === 'function') ? transformer.flush(tctl) : undefined;
                    return Promise.resolve(p).then(function() { rsCtl.close(); });
                },
                abort: function(e) { rsCtl.error(e); }
            });
            if (typeof transformer.start === 'function') { try { transformer.start(tctl); } catch (e) { tctl.error(e); } }
            this.readable = readable;
            this.writable = writable;
        };
        globalThis.TransformStream = TS;
    }

    // `TextDecoderStream` / `TextEncoderStream` — the streaming text codecs that ride a fetch pipeline:
    // `for await (const s of res.body.pipeThrough(new TextDecoderStream())) …` turns a stream of byte
    // chunks into a stream of decoded strings WITHOUT buffering the whole body, and correctly across a
    // multi-byte character split over a chunk boundary (the `{stream:true}` contract). They were absent.
    // Real wrappers over the now-real TransformStream + the existing TextDecoder/TextEncoder.
    if (typeof globalThis.TextDecoderStream === 'undefined') {
        globalThis.TextDecoderStream = function TextDecoderStream(label, options) {
            var dec = new globalThis.TextDecoder(label, options);
            var ts = new globalThis.TransformStream({
                transform: function (chunk, ctrl) {
                    var text = dec.decode(chunk, { stream: true });
                    if (text) { ctrl.enqueue(text); }
                },
                flush: function (ctrl) {
                    var tail = dec.decode();   // flush any held partial sequence
                    if (tail) { ctrl.enqueue(tail); }
                }
            });
            this.readable = ts.readable;
            this.writable = ts.writable;
            this.encoding = dec.encoding;
            this.fatal = !!(options && options.fatal);
            this.ignoreBOM = !!(options && options.ignoreBOM);
        };
    }
    if (typeof globalThis.TextEncoderStream === 'undefined') {
        globalThis.TextEncoderStream = function TextEncoderStream() {
            var enc = new globalThis.TextEncoder();
            var ts = new globalThis.TransformStream({
                transform: function (chunk, ctrl) { ctrl.enqueue(enc.encode(String(chunk))); }
            });
            this.readable = ts.readable;
            this.writable = ts.writable;
            this.encoding = 'utf-8';
        };
    }

    // `raw` is the response body as a BINARY STRING — one code unit per byte, values 0..255 — and
    // it is what `arrayBuffer()`/`bytes()`/`body` read. `text` remains the host's charset-decoded
    // string and is what `text()`/`json()` read, unchanged.
    //
    // Two channels rather than one, because a `Response` genuinely has two readings and deriving
    // either from the other loses. Re-encoding `text` to UTF-8 (what this did until tick 228)
    // INFLATES every byte above 0x7F into two — a 260-byte media segment came back as 407, so no
    // demuxer could ever parse it. Going the other way and decoding `raw` as UTF-8 in JS would throw
    // away the host's charset sniffing, which is what makes a legacy-encoded page readable.
    //
    // `raw` is optional: an older call site that omits it falls back to encoding `text`, which is
    // exactly right for a body that really was text.
    globalThis.__makeResponse = function(status, text, headers, raw) {
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
            arrayBuffer: function(){ used = true; return Promise.resolve(globalThis.__bodyBytes(text, raw).buffer); },
            bytes: function(){ used = true; return Promise.resolve(globalThis.__bodyBytes(text, raw)); },
            blob: function(){ used = true; return Promise.resolve(new globalThis.Blob([text])); },
            clone: function(){ return globalThis.__makeResponse(status, text, headers, raw); }
        };
        Object.defineProperty(res, 'bodyUsed', {
            get: function(){ return used; }, enumerable: true, configurable: true
        });
        // **Lazy, and one per response.** Constructing the stream eagerly would allocate a byte copy
        // for every response a page ever fetches, including the ones it only calls `.json()` on.
        Object.defineProperty(res, 'body', {
            get: function() {
                if (stream === null) {
                    var bytes = globalThis.__bodyBytes(text, raw);
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
    globalThis.__bodyBytes = function(text, raw) {
        // The binary-string channel: each code unit IS a byte, so mask and copy. No encoder is
        // involved, which is the entire point — an encoder is what corrupted this.
        if (typeof raw === 'string') {
            var out = new Uint8Array(raw.length);
            for (var i = 0; i < raw.length; i++) { out[i] = raw.charCodeAt(i) & 0xff; }
            return out;
        }
        return new globalThis.TextEncoder().encode(String(text));
    };

    // ── **`new Response(...)` and `new Request(...)` — CONSTRUCTIBLE, not merely nameable.** ────
    //
    // Both were on the inert interface-surface list, which made `typeof Response === 'function'`
    // true while `new Response('x')` produced an object with no `status`, no `headers` and no
    // `clone()`. That is the worst shape a failure takes: the feature detection passes and the
    // first real use fails somewhere else entirely.
    //
    // A page constructs responses for real reasons — it is how a Service Worker synthesises an
    // offline page, how a test double stands in for the network, and how anything gets INTO the
    // Cache API without a fetch. Building them on `__makeResponse` means a constructed response and
    // a fetched one are the same object shape, so nothing downstream has to care which it got.
    globalThis.Response = function Response(body, init) {
        init = init || {};
        var text = '', raw;
        if (body === undefined || body === null) {
            text = '';
        } else if (typeof body === 'string') {
            // A text body has no byte channel — `__bodyBytes` encodes it as UTF-8, which is right.
            text = body;
        } else {
            // A binary body keeps ONE CHAR PER BYTE. Decoding it to text here and re-encoding on
            // read is what corrupts every byte above 0x7F.
            var bytes = (body instanceof Uint8Array) ? body
                      : (body && body.buffer instanceof ArrayBuffer) ? new Uint8Array(body.buffer)
                      : (body instanceof ArrayBuffer) ? new Uint8Array(body)
                      : null;
            if (bytes) {
                raw = '';
                for (var i = 0; i < bytes.length; i++) { raw += String.fromCharCode(bytes[i] & 0xff); }
                try { text = new globalThis.TextDecoder().decode(bytes); } catch (e) { text = raw; }
            } else {
                text = String(body);
            }
        }
        var pairs = [];
        var h = init.headers;
        if (h) {
            if (Array.isArray(h)) {
                for (var j = 0; j < h.length; j++) {
                    if (h[j] && h[j].length >= 2) pairs.push([String(h[j][0]), String(h[j][1])]);
                }
            } else if (typeof h.forEach === 'function') {
                h.forEach(function (v, k) { pairs.push([String(k), String(v)]); });
            } else {
                for (var k in h) { if (Object.prototype.hasOwnProperty.call(h, k)) pairs.push([k, String(h[k])]); }
            }
        }
        var status = (init.status === undefined) ? 200 : Number(init.status);
        var res = globalThis.__makeResponse(status, text, pairs, raw);
        res.statusText = (init.statusText === undefined) ? '' : String(init.statusText);
        if (init.url) res.url = String(init.url);
        return res;
    };
    // `Response.json(data, init)` — the static that builds a JSON Response in one call:
    // `return Response.json({ ok: true })` in a Service Worker `fetch` handler or an app route. It
    // JSON-serialises `data`, defaults the `Content-Type` to `application/json` (unless the caller set
    // one), and is the read-symmetric of `res.json()`. It was missing, so that idiom threw
    // `Response.json is not a function`.
    globalThis.Response.json = function (data, init) {
        init = init || {};
        var body = JSON.stringify(data);
        if (body === undefined) { throw new TypeError('The data is not JSON-serializable'); }
        // Copy caller headers into a plain object, then default the content-type.
        var hdrs = {}, ih = init.headers, k;
        if (ih) {
            if (typeof ih.forEach === 'function') { ih.forEach(function (v, key) { hdrs[key] = v; }); }
            else if (Array.isArray(ih)) { for (var i = 0; i < ih.length; i++) { if (ih[i]) hdrs[ih[i][0]] = ih[i][1]; } }
            else { for (k in ih) { if (Object.prototype.hasOwnProperty.call(ih, k)) hdrs[k] = ih[k]; } }
        }
        var hasCT = false;
        for (k in hdrs) { if (String(k).toLowerCase() === 'content-type') { hasCT = true; } }
        if (!hasCT) { hdrs['content-type'] = 'application/json'; }
        return new globalThis.Response(body, {
            status: (init.status === undefined) ? 200 : init.status,
            statusText: init.statusText,
            headers: hdrs
        });
    };

    globalThis.Request = function Request(input, init) {
        init = init || {};
        var url = (input && typeof input === 'object' && typeof input.url === 'string')
            ? String(input.url) : String(input);
        try { url = new globalThis.URL(url, (globalThis.location && globalThis.location.href) || undefined).href; }
        catch (e) { /* a relative URL with no base stays as written */ }
        var method = String(init.method
            || (input && typeof input === 'object' && input.method) || 'GET').toUpperCase();
        var pairs = [];
        var h = init.headers;
        if (h) {
            if (Array.isArray(h)) {
                for (var j = 0; j < h.length; j++) {
                    if (h[j] && h[j].length >= 2) pairs.push([String(h[j][0]), String(h[j][1])]);
                }
            } else if (typeof h.forEach === 'function') {
                h.forEach(function (v, k) { pairs.push([String(k), String(v)]); });
            } else {
                for (var k in h) { if (Object.prototype.hasOwnProperty.call(h, k)) pairs.push([k, String(h[k])]); }
            }
        }
        var req = {
            url: url, method: method, headers: globalThis.__makeHeaders(pairs),
            credentials: init.credentials || 'same-origin',
            mode: init.mode || 'cors', cache: init.cache || 'default',
            redirect: init.redirect || 'follow', body: init.body,
            clone: function () { return new globalThis.Request(url, init); }
        };
        return req;
    };

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
        // A `blob:` URL is not a network resource — it names an in-process Blob minted by
        // `URL.createObjectURL`, and it resolves ENTIRELY here without a host round-trip. This is the
        // second half of `canvas.toBlob`/file-upload preview: `fetch(URL.createObjectURL(blob))` reads
        // the bytes back. It consults the SAME registry the MSE attachment and the Worker `sourceOf`
        // already read (`__mseLookup`) — one object-URL store, never a second that could drift.
        if (typeof url === 'string' && url.indexOf('blob:') === 0) {
            var _bo = (typeof globalThis.__mseLookup === 'function') ? globalThis.__mseLookup(url) : undefined;
            if (_bo && typeof _bo.__blobText === 'string') {
                var _ct = _bo.type || 'application/octet-stream';
                // Pass the byte-string as BOTH `text` and `raw`: `raw` is the binary channel
                // (`__bodyBytes` copies each code unit as a byte, no encoder), so a PNG survives the
                // round-trip unmangled through `.arrayBuffer()`/`.bytes()`/`.blob()`.
                return Promise.resolve(globalThis.__makeResponse(200, _bo.__blobText,
                    'content-type: ' + _ct, _bo.__blobText));
            }
            // Revoked, never registered, or naming a non-Blob (a MediaSource) — a stale object URL is
            // a network error in a real browser, not an empty 200.
            return Promise.reject(new TypeError('Failed to fetch'));
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
    globalThis.__deliverFetch = function(id, status, text, headers, raw) {
        var cb = __fetchCb[id]; if (!cb) return; delete __fetchCb[id];
        if (status === 0) { cb.reject(new TypeError("Failed to fetch")); return; }
        cb.resolve(globalThis.__makeResponse(status, text, headers, raw));
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
    globalThis.__deliverXhr = function(id, status, text, headers, raw) {
        var x = __xhrObj[id]; if (!x) return; delete __xhrObj[id];
        x._respHeaders = headers || [];
        x.status = status; x.statusText = ""; x.responseText = text;
        // `responseType = 'arraybuffer'` is how every adaptive player fetches a media segment, so it
        // reads the binary channel rather than the decoded text.
        x.response = (x.responseType === "json")
            ? (function(){ try { return JSON.parse(text); } catch (e) { return null; } })()
            : (x.responseType === "arraybuffer")
                ? globalThis.__bodyBytes(text, raw).buffer
                : (x.responseType === "blob")
                    ? new globalThis.Blob([text])
                    : text;
        x.readyState = 4;
        if (typeof x.onreadystatechange === 'function') { try { x.onreadystatechange(); } catch (e) {} }
        if (status === 0) { if (typeof x.onerror === 'function') { try { x.onerror(new Error("network")); } catch (e) {} } }
        else if (typeof x.onload === 'function') { try { x.onload(); } catch (e) {} }
    };

    // Kind-agnostic delivery: the host settles a request by id without tracking whether it was
    // a fetch or an XHR.
    globalThis.__deliver = function(id, status, text, headers, raw) {
        if (__fetchCb[id]) { globalThis.__deliverFetch(id, status, text, headers, raw); return; }
        if (__xhrObj[id]) { globalThis.__deliverXhr(id, status, text, headers, raw); return; }
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
          // A gradient used where a flat colour is required (conic, or a gradient with <2 stops) falls
          // back to its last stop. `__stops` are `[offset, r, g, b, a]` tuples (see `makeGrad`).
          if (v && v.__grad) { var st = v.__stops.length ? v.__stops[v.__stops.length-1] : null;
                               return st ? [st[1], st[2], st[3], st[4]] : [0,0,0,1]; }
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

          // A real (linear/radial) gradient with >=2 stops rasterizes through the native shader. Conic
          // gradients (no tiny-skia SweepGradient in this build) and single-stop gradients fall back to
          // the flat last-stop colour via `color()`, so they are excluded here.
          function isGrad(style) {
            return !!(style && style.__grad && style.__stops && style.__stops.length >= 2);
          }
          function isPat(style) { return !!(style && style.__pattern); }
          function curAlpha() { return ctx.globalAlpha == null ? 1 : ctx.globalAlpha; }
          // Flatten a gradient to the spec Rust reads: [kind, x0,y0,r0, x1,y1,r1, off,r,g,b,a, …].
          // globalAlpha folds into each stop's alpha, matching how it modulates a flat fill.
          function gradSpec(g) {
            var ga = (ctx.globalAlpha == null ? 1 : ctx.globalAlpha);
            var spec = [g.__kind, g.__geo[0], g.__geo[1], g.__geo[2], g.__geo[3], g.__geo[4], g.__geo[5]];
            for (var i = 0; i < g.__stops.length; i++) {
              var s = g.__stops[i];
              spec.push(s[0], s[1], s[2], s[3], s[4] * ga);
            }
            return spec;
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
            var fs = ctx.fillStyle, rc = [5, +x||0, +y||0, +w||0, +h||0];
            if (isPat(fs)) { el.__cvPathPattern(rc, true, fs.__nodeId, fs.__rep, curAlpha(), 0, M); return; }
            if (isGrad(fs)) { el.__cvPathGradient(rc, true, gradSpec(fs), 0, M); return; }
            var c = rgba(fs);
            el.__cvRect(+x||0, +y||0, +w||0, +h||0, c[0], c[1], c[2], c[3], 0, M);
          };
          ctx.strokeRect = function(x, y, w, h){
            var lw = Math.max(+ctx.lineWidth || 1, 0.01);
            var ss = ctx.strokeStyle, rc = [5, +x||0, +y||0, +w||0, +h||0];
            if (isPat(ss)) { el.__cvPathPattern(rc, false, ss.__nodeId, ss.__rep, curAlpha(), lw, M); return; }
            if (isGrad(ss)) { el.__cvPathGradient(rc, false, gradSpec(ss), lw, M); return; }
            var c = rgba(ss);
            el.__cvRect(+x||0, +y||0, +w||0, +h||0, c[0], c[1], c[2], c[3], lw, M);
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
          // `fill(path?, rule?)` / `stroke(path?)` — a `Path2D` first argument rasterizes ITS command
          // stream instead of the context's current path, which is how charting/graphics libraries
          // reuse a pre-built shape (`ctx.fill(new Path2D(iconD))`) without re-issuing every segment.
          // Duck-typed on `__cmds` so a bare fill-rule string (`ctx.fill('evenodd')`) still falls to `P`.
          ctx.fill = function(arg){
            var cmds = (arg && arg.__cmds) ? arg.__cmds : P;
            var fs = ctx.fillStyle;
            if (isPat(fs)) { el.__cvPathPattern(cmds, true, fs.__nodeId, fs.__rep, curAlpha(), 0, M); return; }
            if (isGrad(fs)) { el.__cvPathGradient(cmds, true, gradSpec(fs), 0, M); return; }
            var c = rgba(fs);
            el.__cvPath(cmds, true, c[0], c[1], c[2], c[3], 0, M);
          };
          ctx.stroke = function(arg){
            var cmds = (arg && arg.__cmds) ? arg.__cmds : P;
            var lw = Math.max(+ctx.lineWidth || 1, 0.01);
            var ss = ctx.strokeStyle;
            if (isPat(ss)) { el.__cvPathPattern(cmds, false, ss.__nodeId, ss.__rep, curAlpha(), lw, M); return; }
            if (isGrad(ss)) { el.__cvPathGradient(cmds, false, gradSpec(ss), lw, M); return; }
            var c = rgba(ss);
            el.__cvPath(cmds, false, c[0], c[1], c[2], c[3], lw, M);
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
            w = Math.max(1, w|0); h = Math.max(1, h|0);
            var raw = el.__cvGetImageData(x|0, y|0, w, h) || [];
            return new globalThis.ImageData(new Uint8ClampedArray(raw), w, h);
          };
          ctx.createImageData = function(w, h){
            // createImageData(w, h) — or createImageData(existingImageData), copying its dimensions.
            if (w && typeof w === 'object') { h = w.height; w = w.width; }
            w = Math.max(1, w|0); h = Math.max(1, h|0);
            return new globalThis.ImageData(w, h);
          };
          // putImageData — a raw pixel REPLACE (no transform, globalAlpha or compositing, per spec).
          // Two overloads: (img, dx, dy) writes the whole ImageData; the 7-arg form writes only the
          // (dirtyX, dirtyY, dirtyW, dirtyH) sub-rectangle of the source. `__cvPutImageData` reads a
          // plain Array (not a typed array), so the source bytes are copied out with slice/loop.
          ctx.putImageData = function(img, dx, dy, dirtyX, dirtyY, dirtyW, dirtyH){
            if (!img || !img.data) return;
            dx = dx|0; dy = dy|0;
            var iw = img.width|0, ih = img.height|0, src = img.data;
            if (arguments.length >= 7) {
              dirtyX = dirtyX|0; dirtyY = dirtyY|0; dirtyW = dirtyW|0; dirtyH = dirtyH|0;
              if (dirtyW < 0) { dirtyX += dirtyW; dirtyW = -dirtyW; }
              if (dirtyH < 0) { dirtyY += dirtyH; dirtyH = -dirtyH; }
              if (dirtyX < 0) { dirtyW += dirtyX; dirtyX = 0; }
              if (dirtyY < 0) { dirtyH += dirtyY; dirtyY = 0; }
              if (dirtyX + dirtyW > iw) dirtyW = iw - dirtyX;
              if (dirtyY + dirtyH > ih) dirtyH = ih - dirtyY;
              if (dirtyW <= 0 || dirtyH <= 0) return;
              var sub = new Array(dirtyW * dirtyH * 4);
              for (var ry = 0; ry < dirtyH; ry++) {
                for (var rx = 0; rx < dirtyW; rx++) {
                  var so = ((dirtyY + ry) * iw + (dirtyX + rx)) * 4;
                  var to = (ry * dirtyW + rx) * 4;
                  sub[to] = src[so]; sub[to+1] = src[so+1]; sub[to+2] = src[so+2]; sub[to+3] = src[so+3];
                }
              }
              el.__cvPutImageData(dx + dirtyX, dy + dirtyY, dirtyW, dirtyH, sub);
            } else {
              el.__cvPutImageData(dx, dy, iw, ih, Array.prototype.slice.call(src));
            }
          };

          // ── drawImage. Three overloads, normalised HERE to one nine-argument shape so the FFI has a
          // single signature — argument COUNT is the only thing that distinguishes them, and that is
          // knowable only on this side of the boundary.
          //   (img, dx, dy)                          — intrinsic size
          //   (img, dx, dy, dw, dh)                  — scaled
          //   (img, sx, sy, sw, sh, dx, dy, dw, dh)  — cropped and scaled
          ctx.drawImage = function(src, a, b, c, d, e, f, g, h){
            if (!src) return;
            // A source is identified by NODE, not by its pixels: an image is megabytes and a sprite
            // blitted every animation frame would otherwise copy it 60 times a second.
            var id = src.__nodeId;
            if (id == null) return;   // OffscreenCanvas / <video>: no pixels of ours yet
            var sz = el.__cvSourceSize(id);
            // Not decoded yet. Per spec drawing an unloaded image is a silent no-op, and a chart that
            // draws on every frame will simply land it once the bytes arrive.
            if (!sz) return;
            // A `createImageBitmap` result carries a crop rect into the underlying node's pixels; its
            // intrinsic size is the crop, and any explicit source rect is relative to that crop.
            var iw = sz[0], ih = sz[1], cropX = 0, cropY = 0;
            if (src.__crop) { cropX = src.__crop[0]; cropY = src.__crop[1]; iw = src.__crop[2]; ih = src.__crop[3]; }
            var n = arguments.length;
            var sx, sy, sw, sh, dx, dy, dw, dh;
            if (n >= 9)      { sx=+a; sy=+b; sw=+c; sh=+d; dx=+e; dy=+f; dw=+g; dh=+h; }
            else if (n >= 5) { sx=0; sy=0; sw=iw; sh=ih; dx=+a; dy=+b; dw=+c; dh=+d; }
            else             { sx=0; sy=0; sw=iw; sh=ih; dx=+a; dy=+b; dw=iw; dh=ih; }
            sx += cropX; sy += cropY;   // shift the source rect into the crop's origin
            // `M`, the closure's live transform — NOT `ctx.M`, which does not exist. Reading the
            // wrong one hands the native `undefined`, the matrix decodes as identity, and every
            // transformed draw silently lands at the untransformed origin.
            el.__cvDrawImage(id, sx, sy, sw, sh, dx, dy, dw, dh,
                             ctx.globalAlpha == null ? 1 : ctx.globalAlpha, M);
          };

          // ── Gradients. A gradient carries its geometry and `[offset, r, g, b, a]` stops, and
          // `ctx.fill`/`fillRect`/`stroke` rasterize them through a real tiny-skia shader
          // (`__cvPathGradient`). `kind` 0 = linear `(x0,y0)→(x1,y1)`; 1 = radial focal `(x0,y0,r0)` →
          // outer circle `(x1,y1,r1)`; 2 = conic/sweep, centre `(x0,y0)`, start angle in the `r0` slot.
          function makeGrad(kind, x0, y0, r0, x1, y1, r1) {
            var g = { __grad: true, __kind: kind,
                      __geo: [+x0||0, +y0||0, +r0||0, +x1||0, +y1||0, +r1||0], __stops: [] };
            g.addColorStop = function(off, c){
              var col = color(c);
              g.__stops.push([+off||0, col[0], col[1], col[2], (col.length > 3 ? col[3] : 1)]);
            };
            return g;
          }
          ctx.createLinearGradient = function(x0, y0, x1, y1){ return makeGrad(0, x0, y0, 0, x1, y1, 0); };
          ctx.createRadialGradient = function(x0, y0, r0, x1, y1, r1){ return makeGrad(1, x0, y0, r0, x1, y1, r1); };
          // Conic (sweep): kind 2, centre (x,y), r0 slot carries the start angle (radians).
          ctx.createConicGradient = function(startAngle, x, y){ return makeGrad(2, x, y, +startAngle||0, x, y, 0); };
          // A pattern tiles a source image (an `<img>` or `<canvas>` — anything with published pixels)
          // across the filled shape. It is identified by NODE, the same handle `drawImage` uses, so no
          // pixels cross here. `null` when the source is not a usable image yet (spec behaviour).
          ctx.createPattern = function(image, repetition){
            if (!image || image.__nodeId == null) { return null; }
            var rep = ({ 'repeat': 0, '': 0, 'repeat-x': 1, 'repeat-y': 2, 'no-repeat': 3 })[
                       repetition == null ? 'repeat' : String(repetition)];
            if (rep == null) { rep = 0; }
            return { __pattern: true, __nodeId: image.__nodeId, __rep: rep };
          };
          ctx.setLineDash = function(){};
          ctx.getLineDash = function(){ return []; };

          Object.defineProperty(el, '__ctx', { value: ctx, enumerable: false });
          return ctx;
        };

        el.toDataURL = function(){ return el.__cvToDataURL(); };
        // `toBlob` is the export half of the canvas: chart-download buttons, image editors' "save",
        // and every "upload this canvas" flow do `canvas.toBlob(b => fd.append('file', b))`. The old
        // stub handed back `null`, which is what a real browser gives a *tainted* canvas — so a page
        // testing for that took the cross-origin-taint branch and silently refused to export a canvas
        // it had every right to. The bytes already exist: `__cvToDataURL` rasterises what was drawn to
        // a real `data:image/png;base64,…`. Decode that ONE representation into a Blob rather than mint
        // a second raster path that could disagree with `toDataURL`.
        el.toBlob = function(cb, type){
            if (typeof cb !== 'function') { return; }
            var out = null;
            try {
                var url = el.__cvToDataURL();               // data:image/png;base64,....
                var comma = url.indexOf(',');
                if (comma >= 0 && url.slice(0, 5) === 'data:') {
                    var meta = url.slice(5, comma);          // e.g. "image/png;base64"
                    // We only ever encode PNG (that is what `toDataURL` produces). Report the type we
                    // ACTUALLY made, not the `type` argument — a Blob labelled image/jpeg whose bytes
                    // are a PNG is the exact lie this project refuses. `type` is accepted and ignored.
                    var mt = (meta.split(';')[0]) || 'image/png';
                    var bin = (typeof globalThis.atob === 'function') ? globalThis.atob(url.slice(comma + 1)) : '';
                    out = new globalThis.Blob([bin], { type: mt });
                }
            } catch (e) { out = null; }
            // Asynchronous by spec — the callback must land on a later turn, never inline, or a page
            // that reads a variable the callback sets finds it still undefined.
            var run = function(){ cb(out); };
            if (typeof globalThis.queueMicrotask === 'function') { globalThis.queueMicrotask(run); }
            else { setTimeout(run, 0); }
        };
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
            // A BINARY part (ArrayBuffer / typed-array view / DataView) is its raw BYTES, not a string.
            // `String(new Uint8Array([1,2,3]))` is `"1,2,3"` (5 chars) — so a Blob built from decoded
            // image/audio bytes, a file upload, or `canvas.toBlob` silently held the wrong data and the
            // wrong `size`. Represent bytes as a binary string (one char per byte, 0-255): `arrayBuffer`,
            // `size`, `slice`, `stream` and `readAs*` all already read `__blobText` that way, and STRING
            // parts keep their exact prior char-per-char behaviour (no change to text-blob consumers).
            if (p && p.__blobText !== undefined) { text += p.__blobText; }        // nested Blob/File
            else if (p instanceof ArrayBuffer) {
              var ua = new Uint8Array(p);
              for (var j = 0; j < ua.length; j++) { text += String.fromCharCode(ua[j]); }
            } else if (ArrayBuffer.isView(p) && p.buffer instanceof ArrayBuffer) {
              var uv = new Uint8Array(p.buffer, p.byteOffset, p.byteLength);
              for (var k = 0; k < uv.length; k++) { text += String.fromCharCode(uv[k]); }
            } else { text += String(p); }
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
        // `blob.stream()` — a real ReadableStream of the blob's BYTES (a `Uint8Array` chunk), not the
        // `null` it used to return. This is what lets `blob.stream().pipeThrough(new TextDecoderStream())`
        // read a File/Blob incrementally, and it composes with the streams made real in ticks 298/299.
        globalThis.Blob.prototype.stream = function () {
          var s = String(this.__blobText);
          return new globalThis.ReadableStream({
            start: function (c) {
              var buf = new Uint8Array(s.length);
              for (var i = 0; i < s.length; i++) { buf[i] = s.charCodeAt(i) & 0xff; }
              c.enqueue(buf);
              c.close();
            }
          });
        };
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
        FRp.readAsArrayBuffer = function (b) {
          // Was a `new ArrayBuffer(0)` stub — every reader of a File/Blob's bytes (an upload preview,
          // a WASM/image decoder fed from a drop, `FileReader` polyfills) got an EMPTY buffer. Read the
          // blob's binary string into a real byte buffer, matching `Blob.prototype.arrayBuffer`.
          var s = (b && b.__blobText !== undefined) ? String(b.__blobText) : '';
          var buf = new ArrayBuffer(s.length), v = new Uint8Array(buf);
          for (var i = 0; i < s.length; i++) { v[i] = s.charCodeAt(i) & 0xff; }
          this.__done(buf);
        };
        FRp.abort = function () { this.readyState = 2; };
      }

      // `new ImageData(w, h)` / `new ImageData(Uint8ClampedArray, w [, h])` — the pixel buffer every
      // image-processing library, filter, histogram and WebGL software fallback constructs to hand to
      // `putImageData`. Absent, `new ImageData(...)` threw "not defined" and the whole pixel pipeline
      // died on the first line. The data is always straight-alpha RGBA, `w*h*4` bytes; the array
      // overload lets a library build pixels itself and wrap them without a canvas.
      if (typeof globalThis.ImageData === 'undefined') {
        globalThis.ImageData = function ImageData(a, b, c) {
          var data, w, h;
          if (a instanceof Uint8ClampedArray) {
            data = a; w = b | 0;
            if (w <= 0) { throw new RangeError('ImageData: width must be positive'); }
            if (data.length === 0 || data.length % 4 !== 0) { throw new RangeError('ImageData: data length must be a positive multiple of 4'); }
            var px = data.length / 4;
            if (px % w !== 0) { throw new RangeError('ImageData: data length is not a multiple of (width * 4)'); }
            h = (c !== undefined) ? (c | 0) : (px / w);
            if (h <= 0 || px !== w * h) { throw new RangeError('ImageData: dimensions do not match data length'); }
          } else {
            w = a | 0; h = b | 0;
            if (w <= 0 || h <= 0) { throw new RangeError('ImageData: dimensions must be positive'); }
            data = new Uint8ClampedArray(w * h * 4);
          }
          this.width = w; this.height = h; this.colorSpace = 'srgb'; this.data = data;
        };
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
        globalThis.Option = function Option(text, value, defaultSelected) {
          var el = document.createElement('option');
          if (text !== undefined) { el.textContent = String(text); }
          if (value !== undefined) { el.setAttribute('value', String(value)); }
          // The 3rd argument is `defaultSelected` — it reflects the `selected` content attribute (which is
          // what `.selected` reads for an option that has not yet been dirtied in a rendered select). The
          // 4th `selected` (selectedness) argument agrees with it in every non-pathological call, so the
          // attribute captures both: `new Option('t','v',true)` comes back selected, as it must.
          if (defaultSelected) { el.setAttribute('selected', ''); }
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

      // **`Worker`** — the worker script actually RUNS, and messages actually round-trip.
      //
      // It used to construct and then immediately fire `error`, which is what a browser does when
      // the worker script 404s. That was honest, and it was also the end of the road for every page
      // that does its real work in a worker: the parse never happens, the search index never
      // builds, the diff never comes back, and the page sits on a spinner forever because its
      // fallback path is usually "show the error" rather than "do it on the main thread".
      //
      // What runs here is a **dedicated worker in its own scope, on this thread**. The script is
      // evaluated inside a `with (scope)` whose scope object has a NULL PROTOTYPE — so no
      // `Object.prototype` name (`constructor`, `toString`, `valueOf`) silently shadows a global —
      // and messages cross in both directions as macrotasks carrying a structured clone. The
      // honest divergence, written down rather than discovered: there is no second thread, so a
      // worker that spins does not keep the UI responsive. What it buys is that the work COMPLETES
      // and the answer ARRIVES, which is the difference between a page that loads and one that
      // does not.
      //
      // The scope shadows the DOM-only globals with `undefined` rather than leaving them to fall
      // through the `with` to the real page globals. That is not tidiness: `typeof document ===
      // 'undefined'` is the single most common way a shared module decides whether it is running in
      // a worker, and a worker that can see `document` takes the main-thread branch and then
      // touches a DOM that must not exist there. Everything NOT on that list falls through on
      // purpose — `fetch`, `Promise`, `TextDecoder`, `crypto`, even a nested `Worker` — so the
      // scope stays a deny-list of what a worker must not have, not an ever-stale allow-list of
      // what it may.
      if (typeof globalThis.Worker === 'undefined' || globalThis.Worker.__manukErrorStub) {
        (function () {
          var G = globalThis;

          // A worker script's source, when we can get it without touching the network. Blob URLs
          // are how every bundler ships a worker (`new Worker(URL.createObjectURL(new Blob([src])))`)
          // and `data:` is how small inline ones travel; both resolve synchronously, so the worker
          // starts in the same turn it was constructed.
          function sourceOf(url) {
            var u = String(url);
            if (u.indexOf('blob:') === 0 && typeof G.__mseLookup === 'function') {
              var o = G.__mseLookup(u);
              if (o && typeof o.__blobText === 'string') { return o.__blobText; }
              if (o && typeof o.__blobText !== 'undefined') { return String(o.__blobText); }
            }
            if (u.indexOf('data:') === 0) {
              var c = u.indexOf(',');
              if (c >= 0) {
                var head = u.slice(0, c), body = u.slice(c + 1);
                if (/;base64/i.test(head)) { return typeof G.atob === 'function' ? G.atob(body) : null; }
                try { return decodeURIComponent(body); } catch (e) { return body; }
              }
            }
            return null;
          }

          function clone(v) {
            if (typeof G.structuredClone === 'function') {
              try { return G.structuredClone(v); } catch (e) { /* fall through */ }
            }
            try { return JSON.parse(JSON.stringify(v)); } catch (e) { return v; }
          }

          function messageEvent(data) {
            return { type: 'message', data: data, origin: '', lastEventId: '', source: null, ports: [] };
          }

          // Fire a DOM-ish event at an object carrying `on<type>` plus an `__ls` listener map.
          // A listener that throws must not stop the ones after it, exactly as in a real dispatch.
          function fire(target, type, ev) {
            ev.target = ev.currentTarget = target;
            var on = target['on' + type];
            if (typeof on === 'function') { try { on.call(target, ev); } catch (e) { reportError(target, e); } }
            var ls = (target.__ls && target.__ls[type]) || [];
            for (var i = 0; i < ls.length; i++) {
              try { ls[i].call(target, ev); } catch (e) { reportError(target, e); }
            }
          }

          // An uncaught error inside the worker surfaces on the WORKER OBJECT as `error`, because
          // that is the only place the page can see it. Swallowing it is how a page waits forever.
          function reportError(target, e) {
            var w = target.__worker || target;
            if (!w || !w.__isWorker) { return; }
            setTimeout(function () {
              fire(w, 'error', {
                type: 'error', message: (e && e.message) ? String(e.message) : String(e),
                filename: w.__url, lineno: 0, colno: 0, error: e
              });
            }, 0);
          }

          function listenerAdd(t, fn) {
            if (typeof fn !== 'function') { return; }
            (this.__ls[t] = this.__ls[t] || []).push(fn);
          }
          function listenerRemove(t, fn) {
            var ls = this.__ls[t]; if (!ls) { return; }
            var i = ls.indexOf(fn); if (i >= 0) { ls.splice(i, 1); }
          }

          // The globals a worker MUST NOT see. Each one is something a library feature-detects on
          // to decide "am I on the main thread"; leaving any of them visible makes the answer wrong.
          var DENIED = [
            'document', 'window', 'parent', 'top', 'opener', 'frames', 'frameElement',
            'localStorage', 'sessionStorage', 'history', 'screen', 'alert', 'confirm', 'prompt',
            'matchMedia', 'getComputedStyle', 'requestAnimationFrame', 'cancelAnimationFrame',
            'getSelection', 'print', 'scrollTo', 'scrollBy', 'moveTo', 'resizeTo',
            'HTMLElement', 'Element', 'Node', 'Document', 'Image', 'DOMParser', 'CSS'
          ];

          function buildScope(worker, url, name) {
            // Null prototype: a `with` over a plain object resolves `constructor`, `toString` and
            // every other Object.prototype name to the object, shadowing the real globals inside
            // the worker script. That is a bug you would find weeks later inside someone's library.
            var scope = Object.create(null);

            scope.self = scope;
            scope.globalThis = scope;
            scope.name = String(name || '');
            scope.__ls = {};
            scope.__worker = worker;
            scope.onmessage = null;
            scope.onmessageerror = null;
            scope.onerror = null;
            scope.location = {
              href: String(url), origin: (G.location && G.location.origin) || 'null',
              protocol: (G.location && G.location.protocol) || '', host: (G.location && G.location.host) || '',
              hostname: (G.location && G.location.hostname) || '', port: '', pathname: String(url),
              search: '', hash: '', toString: function () { return String(url); }
            };

            for (var i = 0; i < DENIED.length; i++) { scope[DENIED[i]] = undefined; }

            scope.addEventListener = listenerAdd;
            scope.removeEventListener = listenerRemove;
            scope.dispatchEvent = function (ev) { fire(scope, ev && ev.type, ev || {}); return true; };

            // Worker -> page. The clone happens at POST time, not at delivery: mutating the object
            // after posting must not change what the other side receives, which is the whole
            // observable point of structured cloning rather than passing the reference.
            scope.postMessage = function (data) {
              if (worker.__dead) { return; }
              var copy = clone(data);
              setTimeout(function () {
                if (worker.__dead) { return; }
                fire(worker, 'message', messageEvent(copy));
              }, 0);
            };

            scope.close = function () { worker.__dead = true; };

            // `importScripts` is synchronous by spec and we have no synchronous network. Sources
            // that resolve without one (blob:, data:, and anything already pulled in by the
            // pre-scan below) run; anything else throws a NetworkError, which is what a real worker
            // throws when the import fails — not a silent no-op that leaves the symbol undefined.
            scope.importScripts = function () {
              for (var i = 0; i < arguments.length; i++) {
                var u = String(arguments[i]);
                var src = (worker.__imports && worker.__imports[u] != null)
                  ? worker.__imports[u] : sourceOf(u);
                if (src == null) {
                  var err = new Error("importScripts: could not load '" + u + "' synchronously");
                  err.name = 'NetworkError';
                  throw err;
                }
                evaluateIn(scope, src, worker);
              }
            };

            return scope;
          }

          // The one place worker code is evaluated. `with` needs sloppy mode, and a Function body
          // gets it as long as the body itself does not open with a directive — a `"use strict"` at
          // the top of the worker script lands INSIDE the with-block, where it is an expression
          // statement rather than a directive, so a strict worker script does not break the scope.
          function evaluateIn(scope, src, worker) {
            var f = Function('__scope', 'with (__scope) {\n' + src + '\n}');
            f.call(undefined, scope);
          }

          function Worker(url, options) {
            if (!(this instanceof Worker)) { throw new TypeError("Failed to construct 'Worker': please use the 'new' operator"); }
            var worker = this;
            this.__isWorker = true;
            this.__url = String(url == null ? '' : url);
            this.__ls = {};
            this.__dead = false;
            this.__ready = false;
            this.__pending = [];
            this.__imports = Object.create(null);
            this.onmessage = null;
            this.onmessageerror = null;
            this.onerror = null;

            var name = (options && options.name) || '';

            var boot = function (src) {
              if (worker.__dead) { return; }
              var scope = buildScope(worker, worker.__url, name);
              worker.__scope = scope;

              // Pre-scan for `importScripts('...')` with literal URLs and resolve what we can
              // BEFORE the script runs, because by the time the call happens it is too late to go
              // to the network. The common shape — a literal list at the top of the file — works;
              // a computed URL does not, and says so.
              var re = /importScripts\s*\(([^)]*)\)/g, m;
              while ((m = re.exec(src)) !== null) {
                var lits = m[1].match(/'[^']*'|"[^"]*"/g) || [];
                for (var i = 0; i < lits.length; i++) {
                  var u = lits[i].slice(1, -1);
                  var s = sourceOf(u);
                  if (s != null) { worker.__imports[u] = s; }
                }
              }

              try {
                evaluateIn(scope, src, worker);
              } catch (e) {
                worker.__dead = true;
                setTimeout(function () {
                  fire(worker, 'error', {
                    type: 'error', message: (e && e.message) ? String(e.message) : String(e),
                    filename: worker.__url, lineno: 0, colno: 0, error: e
                  });
                }, 0);
                return;
              }

              // Messages posted between `new Worker(...)` and the script finishing evaluation are
              // not lost — a page that posts its job on the very next line is the normal case, not
              // a race the author got wrong.
              worker.__ready = true;
              var q = worker.__pending; worker.__pending = [];
              for (var j = 0; j < q.length; j++) { deliverIn(worker, q[j]); }
            };

            var fail = function (e) {
              worker.__dead = true;
              setTimeout(function () {
                fire(worker, 'error', {
                  type: 'error', message: 'could not load worker script ' + worker.__url,
                  filename: worker.__url, lineno: 0, colno: 0, error: e || null
                });
              }, 0);
            };

            var src = sourceOf(this.__url);
            if (src != null) {
              boot(src);
            } else if (typeof G.fetch === 'function' && this.__url) {
              // A same-origin `new Worker('/w.js')` goes over the network like any subresource.
              G.fetch(this.__url).then(function (r) {
                if (!r || (r.status && r.status >= 400)) { throw new Error('HTTP ' + (r && r.status)); }
                return r.text();
              }).then(boot, fail);
            } else {
              fail(new Error('no loader'));
            }
          }

          function deliverIn(worker, copy) {
            setTimeout(function () {
              if (worker.__dead || !worker.__scope) { return; }
              fire(worker.__scope, 'message', messageEvent(copy));
            }, 0);
          }

          // Page -> worker.
          Worker.prototype.postMessage = function (data) {
            if (this.__dead) { return; }
            var copy = clone(data);
            if (!this.__ready) { this.__pending.push(copy); return; }
            deliverIn(this, copy);
          };
          // `terminate()` is immediate and final: nothing queued in either direction is delivered
          // after it. A terminate that let one more message through would resurrect exactly the
          // work the page just cancelled.
          Worker.prototype.terminate = function () { this.__dead = true; this.__pending.length = 0; };
          Worker.prototype.addEventListener = listenerAdd;
          Worker.prototype.removeEventListener = listenerRemove;
          Worker.prototype.dispatchEvent = function (ev) { fire(this, ev && ev.type, ev || {}); return true; };

          G.Worker = Worker;

          // The internals a Service Worker needs, published on ONE object rather than copied. A
          // service worker is a worker scope plus a lifecycle plus fetch interception; if it grew
          // its own `sourceOf`/`evaluate`/deny-list, the two would drift and the DOM deny-list —
          // the load-bearing half of both — would end up enforced in one place and not the other.
          G.__manukWorkerInternals = {
            source: sourceOf, evaluate: evaluateIn, denied: DENIED, clone: clone,
            fire: fire, listenerAdd: listenerAdd, listenerRemove: listenerRemove
          };

          // `SharedWorker` is NOT this, and must not pretend to be. It is deliberately left as the
          // honest load-failure stub — but one that carries a real `port` object, because a shim
          // that fires `error` and then TypeErrors on `sw.port.postMessage` fails in the wrong
          // place, before the page's own error path ever gets to run.
          if (typeof G.SharedWorker === 'undefined' || G.SharedWorker === Worker) {
            G.SharedWorker = function SharedWorker(url) {
              this.__url = String(url || ''); this.onerror = null; this.__ls = {};
              this.port = {
                onmessage: null, onmessageerror: null, __ls: {},
                postMessage: function () {}, start: function () {}, close: function () {},
                addEventListener: listenerAdd, removeEventListener: listenerRemove
              };
              var sw = this;
              setTimeout(function () {
                fire(sw, 'error', { type: 'error', message: 'SharedWorker is not supported', filename: sw.__url, lineno: 0, colno: 0 });
              }, 0);
            };
            G.SharedWorker.prototype.addEventListener = listenerAdd;
            G.SharedWorker.prototype.removeEventListener = listenerRemove;
          }
        })();
      }

      // **`navigator.serviceWorker`** — registration, the install/activate lifecycle, and `fetch`
      // interception. Tick 279 built the service worker's STORE (the Cache API) and tick 280 built
      // the scope a worker script runs in; this is the third side, and it is the one that makes the
      // other two do anything on their own.
      //
      // What a page loses without it is not "offline mode". It is that `navigator.serviceWorker`
      // being absent takes the *whole* PWA branch off the table — no offline shell, no instant
      // repeat visit, and on a growing number of sites no first render at all, because the page
      // waits on `.ready` before it will paint. That wait is the failure shape: nothing throws.
      //
      // The bounded slice: registration, `install` -> `activate` with `waitUntil` **awaited between
      // them**, `controller`, `ready`, and interception of `fetch()` from the page via
      // `respondWith`. NOT here: navigation interception, update/redundant lifecycle, `clients`,
      // push, background sync, or scope matching beyond a path prefix.
      if (typeof globalThis.navigator === 'object' && globalThis.navigator &&
          typeof globalThis.navigator.serviceWorker === 'undefined' &&
          globalThis.__manukWorkerInternals) {
        (function () {
          var G = globalThis;
          var W = G.__manukWorkerInternals;

          // Captured BEFORE the wrapper is installed. A service worker that calls `fetch` must
          // reach the network, not re-enter its own handler — that is an infinite recursion whose
          // symptom is a hang, and it is the single easiest way to get interception wrong.
          var networkFetch = G.fetch;

          var active = null;      // the scope of the activated worker, or null
          var registration = null;
          var readyResolve = null;
          var readyPromise = new Promise(function (res) { readyResolve = res; });

          function buildSwScope(url, scopePath) {
            var scope = Object.create(null);
            scope.self = scope;
            scope.globalThis = scope;
            scope.__ls = {};
            scope.oninstall = null; scope.onactivate = null; scope.onfetch = null; scope.onmessage = null;
            // The same deny-list the dedicated worker uses, from the same array — a service worker
            // is even more emphatically not on the main thread than a dedicated one is.
            for (var i = 0; i < W.denied.length; i++) { scope[W.denied[i]] = undefined; }
            scope.addEventListener = W.listenerAdd;
            scope.removeEventListener = W.listenerRemove;
            scope.dispatchEvent = function (ev) { W.fire(scope, ev && ev.type, ev || {}); return true; };
            scope.location = { href: String(url), toString: function () { return String(url); } };
            scope.fetch = networkFetch;   // see above: NOT the wrapper
            scope.skipWaiting = function () { return Promise.resolve(); };
            scope.clients = {
              claim: function () { return Promise.resolve(); },
              matchAll: function () { return Promise.resolve([]); },
              get: function () { return Promise.resolve(undefined); }
            };
            return scope;
          }

          // Dispatch a lifecycle event and return a promise for everything its handlers passed to
          // `waitUntil`. This is the part that must not be faked: `install` extending its own
          // lifetime until the cache is filled is the ENTIRE contract of an offline install step.
          // Fire `activate` without awaiting it and the worker starts serving from a cache it has
          // not finished writing — which fails as a miss on the first offline load, long after.
          function dispatchLifecycle(scope, type) {
            var waits = [];
            var ev = {
              type: type, target: scope,
              waitUntil: function (p) { waits.push(Promise.resolve(p)); }
            };
            W.fire(scope, type, ev);
            return Promise.all(waits);
          }

          function ServiceWorkerRegistration(scope, url, scopePath) {
            this.scope = scopePath;
            this.installing = null;
            this.waiting = null;
            this.active = null;
            this.__scope = scope;
            this.__url = url;
          }
          ServiceWorkerRegistration.prototype.update = function () { return Promise.resolve(this); };
          ServiceWorkerRegistration.prototype.unregister = function () {
            if (active === this.__scope) { active = null; sw.controller = null; }
            this.active = null;
            return Promise.resolve(true);
          };

          function register(url, opts) {
            var u = String(url == null ? '' : url);
            var scopePath = (opts && opts.scope) ? String(opts.scope) : '/';

            return new Promise(function (resolve, reject) {
              var boot = function (src) {
                var scope = buildSwScope(u, scopePath);
                try {
                  W.evaluate(scope, src);
                } catch (e) {
                  reject(e);
                  return;
                }
                var reg = new ServiceWorkerRegistration(scope, u, scopePath);
                registration = reg;
                var worker = { scriptURL: u, state: 'installing', postMessage: function () {} };
                reg.installing = worker;

                // install -> (await every waitUntil) -> activate -> controlling. The ordering IS
                // the capability; anything that collapses it passes an API-shaped test and serves
                // an empty cache in production.
                dispatchLifecycle(scope, 'install').then(function () {
                  worker.state = 'installed';
                  reg.installing = null; reg.waiting = worker;
                  return dispatchLifecycle(scope, 'activate');
                }).then(function () {
                  worker.state = 'activated';
                  reg.waiting = null; reg.active = worker;
                  active = scope;
                  sw.controller = worker;
                  if (readyResolve) { readyResolve(reg); readyResolve = null; }
                  resolve(reg);
                }, reject);
              };

              var src = W.source(u);
              if (src != null) { boot(src); return; }
              if (typeof networkFetch === 'function' && u) {
                networkFetch(u).then(function (r) {
                  if (!r || (r.status && r.status >= 400)) { throw new Error('HTTP ' + (r && r.status)); }
                  return r.text();
                }).then(boot, reject);
              } else {
                reject(new TypeError('could not load service worker script ' + u));
              }
            });
          }

          var sw = {
            controller: null,
            register: register,
            getRegistration: function () { return Promise.resolve(registration || undefined); },
            getRegistrations: function () { return Promise.resolve(registration ? [registration] : []); },
            addEventListener: function () {}, removeEventListener: function () {},
            oncontrollerchange: null, onmessage: null,
            startMessages: function () {}
          };
          Object.defineProperty(sw, 'ready', { get: function () { return readyPromise; } });

          try { G.navigator.serviceWorker = sw; } catch (e) { /* frozen navigator */ }

          // ── Interception. Every `fetch()` from the page passes through the active worker's
          // `fetch` handlers first; if one calls `respondWith`, that is the response and the
          // network is never touched.
          G.fetch = function (url, opts) {
            if (!active) { return networkFetch.apply(this, arguments); }
            var ls = (active.__ls && active.__ls['fetch']) || [];
            var hasHandler = ls.length > 0 || typeof active.onfetch === 'function';
            if (!hasHandler) { return networkFetch.apply(this, arguments); }

            var responded = null;
            var request;
            try {
              request = (typeof G.Request === 'function' && !(url && url.__isRequest))
                ? new G.Request(String(url), opts || {})
                : url;
            } catch (e) { request = { url: String(url), method: (opts && opts.method) || 'GET' }; }

            var ev = {
              type: 'fetch', request: request, target: active,
              // `respondWith` must be recorded SYNCHRONOUSLY during dispatch. A handler that calls
              // it after an await has already lost the race in a real browser too, so deferring the
              // check would make us pass code that is broken everywhere else.
              respondWith: function (r) { responded = Promise.resolve(r); }
            };
            W.fire(active, 'fetch', ev);

            if (responded === null) { return networkFetch.apply(this, arguments); }
            // A handler that responds with `undefined` is a bug in the page, not a reason to
            // silently fall back — falling back would hide it and make the cache look like it
            // worked. Surface it the way a browser does.
            return responded.then(function (r) {
              if (r === undefined || r === null) {
                throw new TypeError('ServiceWorker responded with an invalid response');
              }
              return r;
            });
          };
        })();
      }

      // `window.getSelection()` — editors and "copy link" widgets call it unconditionally.
      //
      // The old stub returned a FRESH inert object on every call: `rangeCount` was 0 forever, every
      // mutator a no-op, and `getSelection() !== getSelection()`. The silent-failure shape is a "copy
      // this code block" button that runs `sel.selectAllChildren(pre); navigator.clipboard.writeText(
      // sel.toString())` — `toString()` answered `''`, so the button copied nothing and threw nothing.
      //
      // Now it is a SINGLE persistent Selection per window, backed by `document.createRange()`
      // (range_js.rs is a real Range). The programmatic surface every editor and share widget drives —
      // `selectAllChildren` / `addRange` / `collapse` / `extend` / `setBaseAndExtent` / `toString` —
      // reflects and mutates a live range. What is NOT modelled: the geometry of a USER mouse-drag
      // selection, which is a layout/hit-test concern, not a scripting one. A Selection is directional
      // (anchor is the fixed end, focus the moving one) where a Range is not, so the direction is
      // tracked here and `extend()` to the left of the anchor is an honest backwards selection.
      (function () {
        if (typeof globalThis.Selection !== 'undefined') { return; }
        var Selection = function Selection() { this._r = null; this._dir = 'fwd'; };

        // Rebuild the backing range from an anchor point and a focus point, deciding order HERE (a
        // Range auto-normalises start<=end and would silently swap a backwards selection's ends).
        Selection.prototype.__set = function (an, ao, fn, fo) {
          if (an == null || fn == null) { this._r = null; return; }
          var probe = document.createRange();
          probe.setStart(an, ao || 0);
          var fwd = true;
          try { fwd = probe.comparePoint(fn, fo || 0) >= 0; } catch (e) { fwd = true; }
          var r = document.createRange();
          if (fwd) { r.setStart(an, ao || 0); r.setEnd(fn, fo || 0); this._dir = 'fwd'; }
          else     { r.setStart(fn, fo || 0); r.setEnd(an, ao || 0); this._dir = 'bwd'; }
          this._r = r;
        };
        var anchor = function (s) { return s._dir === 'fwd'
          ? { n: s._r.startContainer, o: s._r.startOffset }
          : { n: s._r.endContainer,   o: s._r.endOffset }; };
        var focus = function (s) { return s._dir === 'fwd'
          ? { n: s._r.endContainer,   o: s._r.endOffset }
          : { n: s._r.startContainer, o: s._r.startOffset }; };

        var sdef = function (name, get) {
          Object.defineProperty(Selection.prototype, name, { get: get, configurable: true });
        };
        sdef('rangeCount',   function () { return this._r ? 1 : 0; });
        sdef('isCollapsed',  function () { return !this._r || this._r.collapsed; });
        sdef('type',         function () { return !this._r ? 'None' : (this._r.collapsed ? 'Caret' : 'Range'); });
        sdef('anchorNode',   function () { return this._r ? anchor(this).n : null; });
        sdef('anchorOffset', function () { return this._r ? anchor(this).o : 0; });
        sdef('focusNode',    function () { return this._r ? focus(this).n : null; });
        sdef('focusOffset',  function () { return this._r ? focus(this).o : 0; });

        Selection.prototype.getRangeAt = function (i) {
          if (i !== 0 || !this._r) { throw new DOMException('index out of range', 'IndexSizeError'); }
          return this._r;
        };
        // Chrome keeps at most one range; a second addRange is ignored rather than throwing.
        Selection.prototype.addRange = function (range) {
          if (this._r || !range) { return; }
          this._r = range.cloneRange ? range.cloneRange() : range;
          this._dir = 'fwd';
        };
        Selection.prototype.removeAllRanges = function () { this._r = null; };
        Selection.prototype.empty = function () { this._r = null; };
        Selection.prototype.removeRange = function (range) { if (range === this._r) { this._r = null; } };
        Selection.prototype.collapse = function (node, offset) {
          if (node == null) { this._r = null; return; }
          this.__set(node, offset || 0, node, offset || 0);
        };
        Selection.prototype.setPosition = Selection.prototype.collapse;
        Selection.prototype.collapseToStart = function () {
          if (!this._r) { throw new DOMException('no range to collapse', 'InvalidStateError'); }
          this.__set(this._r.startContainer, this._r.startOffset, this._r.startContainer, this._r.startOffset);
        };
        Selection.prototype.collapseToEnd = function () {
          if (!this._r) { throw new DOMException('no range to collapse', 'InvalidStateError'); }
          this.__set(this._r.endContainer, this._r.endOffset, this._r.endContainer, this._r.endOffset);
        };
        Selection.prototype.extend = function (node, offset) {
          var a = this._r ? anchor(this) : { n: node, o: offset || 0 };
          this.__set(a.n, a.o, node, offset || 0);
        };
        Selection.prototype.setBaseAndExtent = function (an, ao, fn, fo) { this.__set(an, ao, fn, fo); };
        Selection.prototype.selectAllChildren = function (node) {
          var r = document.createRange();
          r.selectNodeContents(node);
          this._r = r; this._dir = 'fwd';
        };
        Selection.prototype.deleteFromDocument = function () { if (this._r) { this._r.deleteContents(); } };
        Selection.prototype.toString = function () { return this._r ? this._r.toString() : ''; };

        globalThis.Selection = Selection;
      })();

      if (typeof globalThis.getSelection === 'undefined') {
        globalThis.getSelection = function () {
          if (!globalThis.__selection) { globalThis.__selection = new globalThis.Selection(); }
          return globalThis.__selection;
        };
      }
      // `document.getSelection()` is the same object as `window.getSelection()` per spec.
      if (typeof document.getSelection !== 'function') { document.getSelection = globalThis.getSelection; }

      // `document.execCommand` — the LEGACY editing/clipboard command API. Deprecated, but still the
      // DOMINANT "copy to clipboard" implementation on the web: countless sites and pre-2020 libraries
      // (clipboard.js and its clones) select a hidden <textarea> and call `document.execCommand('copy')`,
      // often as the fallback when the async Clipboard API is unavailable. Absent, that call was a
      // `TypeError: document.execCommand is not a function` that took the copy handler — and whatever ran
      // after it — down. We honour the commands that need NO editable DOM mutation: `copy` (copy the
      // current selection to the clipboard, synchronously, through the same host bridge as
      // `navigator.clipboard.writeText`) and `selectAll` (select the document / focused editable).
      // `insertText` is the FIRST brick of the contenteditable EDITING subsystem — it inserts text at
      // the caret inside an editing host and fires the `beforeinput`/`input` (`inputType:'insertText'`)
      // pair every rich editor keys its model + undo stack on. The remaining FORMATTING commands
      // (bold/italic/…) and `cut` still mutate editable content in ways not yet built — they honestly
      // return `false`, and `queryCommandSupported` says so, so a page feature-detects the truth instead
      // of believing a lie.
      // The shared EDITING primitive: insert `text` at the caret inside editing `host`, firing the
      // `beforeinput`→(mutate)→`input` pair with `inputType`. Both `execCommand('insertText')` and the
      // default typed-character action (a printable keydown into a contenteditable) funnel through it,
      // so the two paths cannot drift. A cancelled `beforeinput` VETOES: the DOM is left untouched and
      // `input` never fires (the spec contract a framework editor runs its own model on). Returns
      // whether the command "ran" (true even on a veto — it ran, the insert was declined).
      if (typeof globalThis.__insertTextAtCaret !== 'function') {
        globalThis.__insertTextAtCaret = function (host, text, inputType) {
          if (!host) { return false; }
          text = String(text == null ? '' : text);
          inputType = inputType || 'insertText';
          var bi = new Event('beforeinput', { bubbles: true, cancelable: true });
          bi.inputType = inputType; bi.data = text;
          host.dispatchEvent(bi);
          if (bi.defaultPrevented) { return true; } // ran, but the insert was vetoed — no mutation

          // Caret: a live selection inside the host (replacing any non-collapsed run), else host end.
          var sel = globalThis.getSelection();
          var anchor = (sel && sel._r) ? sel._r.startContainer : null;
          var inHost = false;
          for (var a = anchor; a; a = a.parentNode) { if (a === host) { inHost = true; break; } }
          var r;
          if (inHost && sel && sel._r) {
            r = sel._r;
            if (!r.collapsed) { r.deleteContents(); }
          } else {
            r = document.createRange();
            r.selectNodeContents(host);
            r.collapse(false);
          }
          var sc = r.startContainer, so = r.startOffset;
          if (sc && sc.nodeType === 3) {
            sc.insertData(so, text);                       // merge into the existing text run
            r.setStart(sc, so + text.length); r.collapse(true);
          } else {
            var tn = document.createTextNode(text);
            r.insertNode(tn);
            r.setStartAfter(tn); r.collapse(true);
          }
          if (sel && sel.__set) {
            sel.__set(r.startContainer, r.startOffset, r.startContainer, r.startOffset);
          }
          var inp = new Event('input', { bubbles: true, cancelable: false });
          inp.inputType = inputType; inp.data = text;
          host.dispatchEvent(inp);
          return true;
        };
      }

      // The DELETE counterpart to `__insertTextAtCaret`: remove one grapheme adjacent to the caret (or
      // the current non-collapsed selection) inside editing `host`, firing `beforeinput`→(mutate)→`input`.
      // `forward===false` is Backspace (delete BEFORE the caret, `inputType:'deleteContentBackward'`);
      // `forward===true` is the Delete key (delete AFTER the caret, `inputType:'deleteContentForward'`).
      // A cancelled `beforeinput` vetoes; when there is genuinely nothing to delete (caret at the very
      // start/end of the run, or no caret in host) the DOM is untouched and `input` does not fire, exactly
      // as a browser leaves a no-op Backspace/Delete. Cross-node boundary deletion (merging an adjacent
      // block) is a later, larger brick — this handles collapsed-caret-in-a-text-run and delete-selection.
      if (typeof globalThis.__deleteAtCaret !== 'function') {
        globalThis.__deleteAtCaret = function (host, forward) {
          if (!host) { return false; }
          var inputType = forward ? 'deleteContentForward' : 'deleteContentBackward';
          var bi = new Event('beforeinput', { bubbles: true, cancelable: true });
          bi.inputType = inputType; bi.data = null;
          host.dispatchEvent(bi);
          if (bi.defaultPrevented) { return true; } // ran, but the delete was vetoed

          var sel = globalThis.getSelection();
          var r = sel && sel._r;
          var anchor = r ? r.startContainer : null;
          var inHost = false;
          for (var a = anchor; a; a = a.parentNode) { if (a === host) { inHost = true; break; } }
          if (!inHost || !r) { return true; } // no caret in host — nothing to delete, no `input`

          var deleted = false;
          if (!r.collapsed) {
            r.deleteContents();
            deleted = true;
          } else {
            var sc = r.startContainer, so = r.startOffset;
            if (sc && sc.nodeType === 3) {
              var len = sc.data.length;
              if (forward && so < len) {
                // Delete a whole code point AFTER the caret; step over a leading high surrogate.
                var n = 1, hi = sc.data.charCodeAt(so);
                if (so + 1 < len && hi >= 0xD800 && hi <= 0xDBFF) {
                  var lo2 = sc.data.charCodeAt(so + 1);
                  if (lo2 >= 0xDC00 && lo2 <= 0xDFFF) { n = 2; }
                }
                sc.deleteData(so, n);
                r.setStart(sc, so); r.collapse(true);          // caret stays put
                if (sel.__set) { sel.__set(sc, so, sc, so); }
                deleted = true;
              } else if (!forward && so > 0) {
                // Delete a whole code point BEFORE the caret; back up over a trailing low surrogate.
                var m = 1, lo = sc.data.charCodeAt(so - 1);
                if (so >= 2 && lo >= 0xDC00 && lo <= 0xDFFF) {
                  var hi2 = sc.data.charCodeAt(so - 2);
                  if (hi2 >= 0xD800 && hi2 <= 0xDBFF) { m = 2; }
                }
                sc.deleteData(so - m, m);
                r.setStart(sc, so - m); r.collapse(true);
                if (sel.__set) { sel.__set(sc, so - m, sc, so - m); }
                deleted = true;
              }
            }
            // caret at a text-node/element boundary — a no-op for this brick (cross-node merge is later)
          }
          if (deleted) {
            var inp = new Event('input', { bubbles: true, cancelable: false });
            inp.inputType = inputType; inp.data = null;
            host.dispatchEvent(inp);
          }
          return true;
        };
      }

      // Insert a hard LINE BREAK (`<br>`) at the caret inside editing `host`, firing
      // `beforeinput`→(mutate)→`input` with `inputType:'insertLineBreak'` — the soft newline a
      // `<pre>`-style editor / an "insert line break" toolbar button produces (and the eventual
      // Shift+Enter default). Same caret-resolution + veto contract as `__insertTextAtCaret`; it inserts a
      // `<br>` element rather than text, splitting the current text run if the caret sits inside one.
      if (typeof globalThis.__insertLineBreakAtCaret !== 'function') {
        globalThis.__insertLineBreakAtCaret = function (host) {
          if (!host) { return false; }
          var bi = new Event('beforeinput', { bubbles: true, cancelable: true });
          bi.inputType = 'insertLineBreak'; bi.data = null;
          host.dispatchEvent(bi);
          if (bi.defaultPrevented) { return true; } // ran, but the break was vetoed

          var sel = globalThis.getSelection();
          var anchor = (sel && sel._r) ? sel._r.startContainer : null;
          var inHost = false;
          for (var a = anchor; a; a = a.parentNode) { if (a === host) { inHost = true; break; } }
          var r;
          if (inHost && sel && sel._r) {
            r = sel._r;
            if (!r.collapsed) { r.deleteContents(); }
          } else {
            r = document.createRange();
            r.selectNodeContents(host);
            r.collapse(false);
          }
          var br = document.createElement('br');
          r.insertNode(br);
          r.setStartAfter(br); r.collapse(true);
          if (sel && sel.__set) {
            sel.__set(r.startContainer, r.startOffset, r.startContainer, r.startOffset);
          }
          var inp = new Event('input', { bubbles: true, cancelable: false });
          inp.inputType = 'insertLineBreak'; inp.data = null;
          host.dispatchEvent(inp);
          return true;
        };
      }

      if (typeof document.execCommand !== 'function') {
        var __EXEC_SUPPORTED = { copy: 1, selectall: 1, inserttext: 1, insertlinebreak: 1 };
        document.execCommand = function (cmd) {
          cmd = String(cmd || '').toLowerCase();
          if (cmd === 'copy') {
            var text = '';
            try { text = String(globalThis.getSelection().toString() || ''); } catch (e) {}
            if (!text) { return false; }
            var nav = globalThis.navigator;
            if (nav && nav.clipboard && typeof nav.clipboard.writeText === 'function') {
              nav.clipboard.writeText(text); // synchronously queues the host write; the Promise is ignored
              return true;
            }
            if (typeof globalThis.__clipboardWrite === 'function') {
              globalThis.__clipboardText = text; globalThis.__clipboardWrite(text); return true;
            }
            return false;
          }
          if (cmd === 'selectall') {
            try {
              var ae = document.activeElement;
              var target = (ae && ae.isContentEditable) ? ae : document.body;
              globalThis.getSelection().selectAllChildren(target);
              return true;
            } catch (e) { return false; }
          }
          if (cmd === 'inserttext') {
            // The editing primitive: insert `value` at the caret inside the editing host, firing the
            // `beforeinput`→(mutate)→`input` pair, `inputType:'insertText'`. A page/editor that vetoes
            // the insert does it by cancelling `beforeinput` (the only cancelable step); on a veto the
            // DOM is left untouched and `input` never fires, per the UI Events spec. This is the path
            // an "insert emoji/snippet" toolbar button, a paste-as-plaintext handler, and the default
            // typed-character action all funnel through.
            var text = arguments.length > 2 && arguments[2] != null ? String(arguments[2]) : '';
            try {
              var sel = globalThis.getSelection();
              var anchor = (sel && sel._r) ? sel._r.startContainer : null;
              // The editing host = the nearest editable element ancestor of the caret; failing that
              // (no selection yet) the focused editable, then designMode's <body>.
              var host = null;
              for (var n = anchor; n; n = n.parentNode) {
                if (n.nodeType === 1 && n.isContentEditable) { host = n; break; }
              }
              if (!host) {
                var ae = document.activeElement;
                if (ae && ae.isContentEditable) { host = ae; }
                else if (document.designMode === 'on') { host = document.body; }
              }
              if (!host) { return false; }
              return globalThis.__insertTextAtCaret(host, text, 'insertText');
            } catch (e) { return false; }
          }
          if (cmd === 'insertlinebreak') {
            // Insert a `<br>` at the caret — the `execCommand` entry to `__insertLineBreakAtCaret`.
            try {
              var lsel = globalThis.getSelection();
              var lanchor = (lsel && lsel._r) ? lsel._r.startContainer : null;
              var lhost = null;
              for (var ln = lanchor; ln; ln = ln.parentNode) {
                if (ln.nodeType === 1 && ln.isContentEditable) { lhost = ln; break; }
              }
              if (!lhost) {
                var lae = document.activeElement;
                if (lae && lae.isContentEditable) { lhost = lae; }
                else if (document.designMode === 'on') { lhost = document.body; }
              }
              if (!lhost) { return false; }
              return globalThis.__insertLineBreakAtCaret(lhost);
            } catch (e) { return false; }
          }
          return false; // every other command is the editing subsystem — honestly not built
        };
        document.queryCommandSupported = function (cmd) {
          return !!__EXEC_SUPPORTED[String(cmd || '').toLowerCase()];
        };
        document.queryCommandEnabled = document.queryCommandSupported;
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
        'MessagePort', 'Range', 'DOMTokenList', 'NamedNodeMap', 'Attr',
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

        var ro = function(name, value) {
          Object.defineProperty(el, name, { get: function(){ return value; }, configurable: true });
        };

        // ── `error` / `readyState` — REPORTED BY THE HOST, not guessed here.
        //
        // `error` was eagerly `MediaError(4)` (MEDIA_ERR_SRC_NOT_SUPPORTED) on every media element
        // the moment it was reflected. That was the honest signal while nothing could decode: a
        // player read it, gave up immediately, and showed its fallback. Once tick 263 made
        // Constrained-Baseline MP4 genuinely play, it became a lie that CONTRADICTS `canPlayType`
        // saying `'probably'` — and a player that checks `error` first (most do) still gave up on a
        // video that was about to work.
        //
        // **The fix could not be "default it to null" alone, and that is the whole point of this
        // bridge.** Spec-initial `null` is right for a fresh element, but on its own it means a
        // `<video src="x.webm">` we genuinely cannot decode reports NO error and simply hangs —
        // trading a false negative for a false positive, which is worse: a site that would have
        // shown a fallback now shows a dead player. So neither fixed value is honest. **The only
        // honest answer is the real one**, and the host is the only layer that has it — the shell
        // fetches the bytes and knows whether they decoded (`MediaSet` already records exactly
        // this). `__setOutcome` is that report arriving.
        //
        // Until it arrives, `null` is correct in the spec's own terms: no load has been ATTEMPTED,
        // so there is no error to report yet.
        var err = null;
        var readyState = 0;         // HAVE_NOTHING until a frame is genuinely decoded
        var netState = null;        // null = defer to the MediaSource-aware getter below
        Object.defineProperty(el, 'error', { configurable: true, get: function(){ return err; } });
        Object.defineProperty(el, 'readyState', { configurable: true, get: function(){ return readyState; } });

        // **The host's verdict on this element's media.** `ok === true` means bytes arrived and
        // decoded; `false` means they arrived and did not, or could not be fetched at all.
        //
        // Fires the events too, because a state change no event announces is a state change no
        // player notices — every one of them binds `onerror`/`oncanplay` rather than polling.
        Object.defineProperty(el, '__setOutcome', {
          configurable: true,
          value: function(ok) {
            if (ok) {
              err = null; readyState = 4; netState = 1;   // HAVE_ENOUGH_DATA / NETWORK_IDLE
              el.dispatchEvent && el.dispatchEvent(new globalThis.Event('loadedmetadata'));
              el.dispatchEvent && el.dispatchEvent(new globalThis.Event('loadeddata'));
              el.dispatchEvent && el.dispatchEvent(new globalThis.Event('canplay'));
            } else {
              err = new globalThis.MediaError(4);         // MEDIA_ERR_SRC_NOT_SUPPORTED
              readyState = 0; netState = 3;              // HAVE_NOTHING / NETWORK_NO_SOURCE
              el.dispatchEvent && el.dispatchEvent(new globalThis.Event('error'));
            }
          }
        });
        // `paused` was a getter-only `true`, which was correct while `play()` could only reject.
        // Now that it resolves, an assignment to a getter-only property is a SILENT no-op in sloppy
        // mode (and a TypeError in strict), so `play()` would have flipped nothing and every player
        // would paint a play button over a running video. Backed by a real flag, spec-initial `true`.
        var paused = true;
        Object.defineProperty(el, 'paused', {
          configurable: true,
          get: function(){ return paused; },
          set: function(v){ paused = !!v; }
        });
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
        // The host's report wins when it has one; otherwise NETWORK_LOADING while a MediaSource is
        // attached (the element genuinely is being fed) and NETWORK_NO_SOURCE when it is not.
        live('networkState', function(){ return netState !== null ? netState : (el.__ms ? 2 : 3); });
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
        // `textTracks` is the live list the player enumerates to find the language the user picked.
        live('textTracks', function () {
          var list = el.__textTracks || [];
          list.getTrackById = function (id) {
            for (var i = 0; i < this.length; i++) { if (this[i].id === id) { return this[i]; } }
            return null;
          };
          return list;
        });
        ro('videoWidth', 0);
        ro('videoHeight', 0);

        // Live accessors (tick 360): they stick AND they reach the host. `v.muted = true` /
        // `v.volume = 0.3` are what a player's mute button and volume slider execute — the
        // attribute path never sees them. Published via __mediaProp; getters return the stored
        // value so read-back semantics are unchanged. playbackRate applies host-side since tick
        // 361 (scaled wall clock; audio mutes honestly at rate != 1 — no time-stretch yet).
        (function () {
          var live = { volume: 1, muted: false, playbackRate: 1 };
          var defineLive = function (name, coerce) {
            Object.defineProperty(el, name, {
              configurable: true,
              get: function () { return live[name]; },
              set: function (v) {
                live[name] = coerce(v);
                if (typeof __mediaProp === 'function' && el.__nodeId != null) {
                  __mediaProp(String(el.__nodeId), name, Number(live[name]));
                }
              }
            });
          };
          defineLive('volume', function (v) {
            var n = Number(v); if (n !== n) { n = 1; }
            return n < 0 ? 0 : (n > 1 ? 1 : n);
          });
          defineLive('muted', function (v) { return !!v; });
          defineLive('playbackRate', function (v) {
            var n = Number(v); return n === n ? n : 1;
          });
        })();
        // `currentTime` is NOT a plain data property, because it is the CLOCK the caption timeline
        // runs on. Storing the number and telling nobody is what made `cuechange` unreachable: a
        // page can only learn that the caption changed by being told, and the only thing that knows
        // is the write that moved the clock past a cue boundary.
        el.__currentTime = 0;
        Object.defineProperty(el, 'currentTime', {
          configurable: true,
          get: function () { return el.__currentTime; },
          set: function (v) {
            var n = Number(v);
            el.__currentTime = isFinite(n) ? n : 0;
            if (el.__syncTextTracks) { el.__syncTextTracks(); }
          }
        });
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

        // ── `canPlayType` — and as of tick 263, `''` for everything became the LIE.
        //
        // This answered `''` unconditionally, which was exactly right when nothing could decode.
        // Tick 263 wired the shell's media drive, so a plain `<video src="movie.mp4">` carrying
        // Constrained-Baseline H.264 now fetches, decodes and PLAYS on screen. Keeping the blanket
        // `''` means every site that politely feature-detects is told no about something that
        // works, hides its `<video>`, and shows the "your browser cannot play this" fallback over a
        // player that would have run. **An honest no becomes a lie the moment the answer changes,
        // and this file is the only place that knows.**
        //
        // The three answers are the spec's own, and the distinction is real rather than decorative:
        // `'probably'` means the codecs were NAMED and we have them; `'maybe'` means the container
        // is one we read but the codec string was absent, so it cannot be promised; `''` is no.
        // Answering `'probably'` to a bare `video/mp4` would be the same class of lie in reverse —
        // that container carries HEVC and High-profile H.264 too, and neither decodes here.
        el.canPlayType = function(type) {
          if (typeof type !== 'string' || type === '') { return ''; }
          var t = type.toLowerCase().replace(/\s+/g, '');
          // Anything we can name and cannot decode is refused UP FRONT, before the container is
          // even considered — an mp4 carrying HEVC is still an mp4.
          // av01 left OFF this refuse-list since tick 354, mp3 t363, flac/vorbis t364: they
          // genuinely play in the shell lane, and refusing them became the lie in reverse.
          // Opus stays: symphonia has no Opus decoder, so codecs="opus" is an honest no.
          if (/vp8|vp9|vp09|hev1|hvc1|theora|opus|ac-3/.test(t)) { return ''; }
          if (/webm|matroska|x-flv/.test(t)) { return ''; }
          // Raw audio streams (t363/364): mp3 and flac decode outright; an Ogg is 'probably'
          // only when it NAMES vorbis — a bare audio/ogg may be Opus, so the container being
          // readable earns exactly 'maybe' and no more.
          if (/^audio\/(mpeg|mp3|flac|x-flac|wav|x-wav|wave)($|;)/.test(t)) { return 'probably'; }
          if (/^audio\/ogg/.test(t)) {
            if (t.indexOf('codecs=') === -1) { return 'maybe'; }
            return /vorbis/.test(t) ? 'probably' : '';
          }
          if (/ogg/.test(t)) { return ''; }
          if (!/^(video|audio)\/mp4/.test(t)) { return ''; }
          if (t.indexOf('codecs=') === -1) { return 'maybe'; }
          // `avc1.42xxxx` is Constrained Baseline — the ONLY profile openh264 decodes here. The
          // profile lives in the two hex digits after the dot, and 42 is the one that plays;
          // `avc1.4d`/`avc1.64` (Main/High) are most of the real web and are refused.
          var ok = true;
          if (/avc1\./.test(t) && !/avc1\.42/.test(t)) { ok = false; }
          if (/mp4a\./.test(t) && !/mp4a\.40/.test(t)) { ok = false; }
          return ok ? 'probably' : '';
        };

        // `play()` returned a REJECTED promise, which was honest for the same window and is now
        // the same lie: the shell autoplays a decoded `<video>`, so a page whose play button
        // awaits this promise lands in its own catch branch while the video plays behind it.
        // Resolves, and flips `paused` — the property every player reads back to paint its button.
        el.play = function() {
          el.paused = false;
          return Promise.resolve();
        };
        el.pause = function() { el.paused = true; };
        el.load  = function() {};
        // ── TEXT TRACKS — a REAL TextTrack, because this is the API captions actually arrive
        //    through on streaming sites.
        //
        // hls.js and dash.js ship their OWN WebVTT parsers and call `addTextTrack` + `addCue`,
        // because a segmented stream carries captions inside the media segments rather than as a
        // separate file. So `<track src>` is not the path that matters for the sites this track is
        // aimed at — this is.
        //
        // The stub returned `{cues: [], activeCues: [], mode: 'disabled'}`: an object that accepts
        // every call, reports success, and holds nothing. A player added 900 cues to it and
        // rendered none.
        // `VTTCue` — what hls.js/dash.js construct before calling `addCue`. Absent, it was a
        // ReferenceError inside the player's own caption path, which is a throw rather than a
        // missing feature: the player stops, and whatever it had not yet done stays undone.
        if (typeof globalThis.VTTCue === 'undefined') {
          globalThis.VTTCue = function (startTime, endTime, text) {
            this.startTime = Number(startTime) || 0;
            this.endTime = Number(endTime) || 0;
            this.text = String(text === undefined ? '' : text);
            this.id = '';
            this.track = null;
            // Cue settings are accepted and inert — see the residue note in the journal. Accepting
            // them matters: a player that sets `cue.line = -3` must not throw.
            this.line = 'auto'; this.position = 'auto'; this.size = 100;
            this.align = 'center'; this.vertical = ''; this.snapToLines = true;
          };
          // Players feature-detect `TextTrackCue` before `VTTCue`.
          globalThis.TextTrackCue = globalThis.TextTrackCue || globalThis.VTTCue;
        }

        el.__textTracks = el.__textTracks || [];

        // ── THE CAPTION TIMELINE — `cuechange`, which is the ONLY way a caption ever reaches a
        //    screen.
        //
        // `activeCues` alone is a POLL-ONLY surface, and nothing polls it. Every caption renderer
        // there is — the players' own overlays, and the `<track>` UI — is
        // `track.addEventListener('cuechange', render)`. So a track that answers `activeCues`
        // correctly and never fires is a track whose captions are computed and never shown: the
        // same shape as the inert object tick 256 replaced, one layer further along.
        //
        // The event fires on CHANGE, not on every clock write. A player writes `currentTime` on
        // every frame; a listener that re-renders its caption node each time is a DOM write per
        // frame for a line of text that did not change.
        var sameCues = function (a, b) {
          if (a.length !== b.length) { return false; }
          // Identity, position by position — NOT length. Seeking from one single-cue line straight
          // to another is the common case (a click on the transcript), both sets are length 1, and
          // a length comparison reports "no change" while the viewer sits on the previous line.
          for (var i = 0; i < a.length; i++) { if (a[i] !== b[i]) { return false; } }
          return true;
        };
        el.__syncTextTracks = function () {
          var tracks = el.__textTracks || [];
          var changed = false;
          for (var i = 0; i < tracks.length; i++) {
            var tr = tracks[i];
            var now = tr.activeCues;
            if (sameCues(now, tr.__lastActive || [])) { continue; }
            tr.__lastActive = now;
            changed = true;
            tr.__fire('cuechange');
          }
          if (changed) { el.__publishCues(); }
        };

        // Hand the on-screen cue set to the UA's own caption overlay (the painter, in Rust).
        //
        // Only 'showing' tracks paint. `activeCues` deliberately answers for 'hidden' tracks too —
        // a hidden track is one whose cues are live and whose `cuechange` still fires, so a page's
        // own renderer keeps working — but 'hidden' means EXACTLY "do not display this", and it is
        // the mode a player sets when it draws captions itself. Painting them would double every
        // caption on every site that has a player.
        el.__publishCues = function () {
          if (el.__nodeId == null || !globalThis.__setActiveCues) { return; }
          var out = [];
          var tracks = el.__textTracks || [];
          for (var i = 0; i < tracks.length; i++) {
            var tr = tracks[i];
            if (tr.mode !== 'showing') { continue; }
            var act = tr.activeCues;
            for (var j = 0; j < act.length; j++) {
              var c = act[j];
              // `VTTCue.line` carries THREE distinct things in one property, and each has to come
              // apart correctly: 'auto', a bare number (a LINE COUNT, possibly negative), or a
              // '%'-suffixed string (a percentage of the box). `Number('10%')` is NaN and
              // `parseFloat` on a line count silently loses the distinction, so neither alone will
              // do. Getting this wrong is invisible for `line:0` — which reads the same either way
              // — and wrong for every other value.
              var ln = c.line, lnPct = false, lnVal = null;
              if (ln !== 'auto' && ln != null) {
                if (typeof ln === 'string' && ln.charAt(ln.length - 1) === '%') {
                  lnPct = true; lnVal = parseFloat(ln);
                } else {
                  lnVal = Number(ln);
                }
                if (!isFinite(lnVal)) { lnVal = null; lnPct = false; }
              }
              out.push({
                text: String(c.text == null ? '' : c.text),
                // 'auto' is the string the VTTCue API uses for what the parser calls null, and it
                // must reach the renderer as null — `line: 0` is the top of the frame, `auto` is
                // the bottom, so collapsing one into the other inverts the picture.
                line: lnVal,
                linePct: lnPct,
                position: c.position === 'auto' || c.position == null ? null : Number(c.position),
                size: c.size == null ? 100 : Number(c.size),
                align: String(c.align || 'center'),
                vertical: String(c.vertical || '')
              });
            }
          }
          try { globalThis.__setActiveCues(String(el.__nodeId), JSON.stringify(out)); } catch (e) {}
        };

        el.addTextTrack = function (kind, label, language) {
          var track = {
            kind: kind || 'subtitles',
            label: label || '',
            language: language || '',
            id: '',
            // **`disabled` is the spec default, and it is how "captions off" is represented.**
            // Every player sets `mode = 'showing'` as a deliberate separate step for exactly this
            // reason; a track that served cues regardless of mode would render subtitles the user
            // turned off.
            __mode: 'disabled',
            cues: [],
            __lastActive: [],
            __listeners: [],
            oncuechange: null,
            addCue: function (cue) {
              if (!cue) { return; }
              cue.track = this;
              this.cues.push(cue);
              // Start order, so the caption a viewer sees first is first.
              this.cues.sort(function (a, b) { return a.startTime - b.startTime; });
              // A cue appended over the CURRENT time is on screen the moment it lands — this is the
              // normal case for a live/segmented stream, where cues arrive while the clock runs.
              el.__syncTextTracks();
            },
            removeCue: function (cue) {
              var i = this.cues.indexOf(cue);
              if (i >= 0) { this.cues.splice(i, 1); el.__syncTextTracks(); }
            },
            addEventListener: function (type, fn) {
              if (typeof fn === 'function') { this.__listeners.push({ type: String(type), fn: fn }); }
            },
            removeEventListener: function (type, fn) {
              for (var i = 0; i < this.__listeners.length; i++) {
                if (this.__listeners[i].type === String(type) && this.__listeners[i].fn === fn) {
                  this.__listeners.splice(i, 1); return;
                }
              }
            },
            __fire: function (type) {
              var ev = { type: type, target: this, currentTarget: this, bubbles: false, cancelable: false };
              var on = this['on' + type];
              if (typeof on === 'function') { on.call(this, ev); }
              // A copy, because a listener that removes itself while we iterate must not make the
              // loop skip its neighbour.
              var ls = this.__listeners.slice();
              for (var i = 0; i < ls.length; i++) {
                if (ls[i].type === type) { ls[i].fn.call(this, ev); }
              }
            }
          };
          // `mode` is an accessor for the same reason `currentTime` is: turning captions ON is a
          // state change the renderer must be told about. `mode = 'showing'` with a cue already
          // under the playhead means a caption is now on screen, and the listener that draws it has
          // no other moment to learn that.
          Object.defineProperty(track, 'mode', {
            configurable: true,
            get: function () { return this.__mode; },
            set: function (v) {
              var m = String(v);
              if (m !== 'disabled' && m !== 'hidden' && m !== 'showing') { return; }
              this.__mode = m;
              el.__syncTextTracks();
              // ...and publish REGARDLESS of whether the active set changed. Flipping 'showing' to
              // 'hidden' leaves `activeCues` identical — a hidden track's cues are still live — so
              // `__syncTextTracks` correctly sees no change and fires nothing, and the overlay
              // would keep painting a caption the user just turned off.
              if (el.__publishCues) { el.__publishCues(); }
            }
          });
          Object.defineProperty(track, 'activeCues', {
            configurable: true,
            get: function () {
              // A disabled track has no active cues — that IS "off".
              if (this.mode === 'disabled') { return []; }
              var t = el.currentTime || 0;
              // A LIST: cues overlap (two speakers at once, a label held across lines). Answering
              // this plural question in the singular drops the second speaker for the whole
              // overlap. Half-open [start, end) so back-to-back cues never both render.
              return this.cues.filter(function (c) {
                return t >= c.startTime && t < c.endTime;
              });
            }
          });
          el.__textTracks.push(track);
          return track;
        };
        // ── `<track src>` — the way captions arrive on a plain `<video>`, with no player library.
        //
        // Ticks 255-257 built the parser, the TextTrack and the cuechange timeline, and left them
        // with NO PATH BETWEEN THEM: the parser had no caller outside its own unit tests, and the
        // only cues a page could hold were ones its own JavaScript constructed. That covers the
        // adaptive players and covers nothing else — a news clip, a course video, a documentation
        // screencast all ship `<track src="subs.vtt">` and no JS at all, and got nothing.
        //
        // The fetch goes through the page's OWN `fetch()` rather than a new host path, so it
        // inherits the one set of rules that already exist here: base-URL resolution, the host
        // pump, and whatever the network layer decides.
        el.__loadTracks = function () {
          var kids = el.children || [];
          for (var i = 0; i < kids.length; i++) {
            (function (te) {
              if (!te || te.tagName !== 'TRACK' || te.__loading) { return; }
              var src = te.getAttribute && te.getAttribute('src');
              if (!src) { return; }
              te.__loading = true;
              // NONE(0) -> LOADING(1) -> LOADED(2) / ERROR(3), the values a page reads to decide
              // whether to show its caption button yet.
              te.readyState = 1;
              var track = el.addTextTrack(
                te.getAttribute('kind') || 'subtitles',
                te.getAttribute('label') || '',
                te.getAttribute('srclang') || ''
              );
              te.track = track;
              // `default` is the author saying "on unless the user says otherwise". It is the ONLY
              // way a plain `<video>` ever shows a caption, because there is no script to set
              // `mode` and our chrome has no captions button — without honouring it, every
              // `<track default>` on the web renders exactly nothing.
              if (te.hasAttribute && te.hasAttribute('default')) { track.mode = 'showing'; }

              var fail = function () {
                te.readyState = 3;
                if (typeof te.onerror === 'function') { te.onerror({ type: 'error', target: te }); }
              };
              try {
                globalThis.fetch(src).then(function (r) {
                  if (!r || !r.ok) { fail(); return; }
                  return r.text().then(function (body) {
                    // The REAL parser, not a second one written in JS. A file that is not WebVTT
                    // reports rather than throws — an .srt renamed, or an HTML error page served
                    // with a 200, is a track that fails to load, not a page that dies.
                    var res = JSON.parse(globalThis.__parseVtt(body));
                    if (!res.ok) { fail(); return; }
                    for (var c = 0; c < res.cues.length; c++) {
                      var cue = new globalThis.VTTCue(res.cues[c].start, res.cues[c].end, res.cues[c].text);
                      cue.id = res.cues[c].id || '';
                      // The PLACEMENT half. A player's overlay reads exactly these to position the
                      // caption; leaving them at their defaults puts every cue bottom-centre, which
                      // is the one place an author who set `line:0` was deliberately avoiding.
                      // `null` means `auto` and must stay the string 'auto', not become 0 — `line:0`
                      // is the TOP of the frame and `auto` is the bottom.
                      var sp = res.cues[c];
                      cue.vertical = sp.vertical || '';
                      cue.line = (sp.line === null) ? 'auto' : (sp.linePct ? sp.line + '%' : sp.line);
                      cue.position = (sp.position === null) ? 'auto' : sp.position;
                      cue.size = sp.size;
                      cue.align = sp.align;
                      track.addCue(cue);
                    }
                    te.readyState = 2;
                    if (typeof te.onload === 'function') { te.onload({ type: 'load', target: te }); }
                  });
                }).catch(fail);
              } catch (e) { fail(); }
            })(kids[i]);
          }
        };

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
          // Go through a controller so the timeout FIRES the abort event (not just flips `aborted`) —
          // a `fetch()` given a timeout signal is cancelled by the event, and `AbortSignal.any` below
          // only sees a timeout expire because this dispatches. The reason is a `TimeoutError`
          // DOMException, which is how callers tell a timeout apart from a user abort.
          var c = new globalThis.AbortController();
          setTimeout(function(){ c.abort(new DOMException('signal timed out', 'TimeoutError')); }, ms);
          return c.signal;
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
      // `AbortSignal.any(signals)` — the combinator that returns a signal aborting as soon as ANY of
      // its inputs aborts, forwarding that input's reason. The canonical use is one request that must
      // cancel on EITHER a user action OR a timeout: `fetch(url, { signal: AbortSignal.any([
      // userController.signal, AbortSignal.timeout(5000)]) })`. `timeout` was already here but `any`
      // was missing, so that pattern threw `AbortSignal.any is not a function`. Built on the real
      // AbortController, so the returned signal is a REAL AbortSignal — its `abort` event fires and
      // `aborted`/`reason` are live — not an inert look-alike.
      if (typeof globalThis.AbortSignal === 'function' && typeof globalThis.AbortSignal.any !== 'function') {
        globalThis.AbortSignal.any = function(signals) {
          var ctrl = new globalThis.AbortController();
          var arr = Array.from(signals || []);
          // An already-aborted input → the result is aborted right now, with that input's reason.
          for (var i = 0; i < arr.length; i++) {
            if (arr[i] && arr[i].aborted) { ctrl.abort(arr[i].reason); return ctrl.signal; }
          }
          var registered = [];
          var cleanup = function() {
            for (var j = 0; j < registered.length; j++) {
              try { registered[j].s.removeEventListener('abort', registered[j].f); } catch (e) {}
            }
            registered = [];
          };
          for (var k = 0; k < arr.length; k++) {
            (function(s) {
              if (!s || typeof s.addEventListener !== 'function') { return; }
              var f = function() { ctrl.abort(s.reason); cleanup(); };
              s.addEventListener('abort', f);
              registered.push({ s: s, f: f });
            })(arr[k]);
          }
          return ctrl.signal;
        };
      }

      // `DOMPoint` / `DOMPointReadOnly` — the geometry point that pairs with `DOMMatrix`: a canvas or
      // graphics library maps a coordinate through a transform with `point.matrixTransform(matrix)`, and
      // `matrix.transformPoint(point)` returns one. Absent, `new DOMPoint(...)` threw. `{x,y,z,w}` with
      // `w` defaulting to 1 (a position, not a direction); `matrixTransform` applies the 2D affine map,
      // `fromPoint` copies, `toJSON` serialises.
      if (typeof globalThis.DOMPoint === 'undefined') {
        globalThis.DOMPoint = function DOMPoint(x, y, z, w) {
          this.x = x || 0; this.y = y || 0; this.z = z || 0; this.w = (w === undefined) ? 1 : w;
        };
        globalThis.DOMPoint.prototype.matrixTransform = function (m) {
          // 2D affine: x' = a·x + c·y + e, y' = b·x + d·y + f (the w/z pass through for the 2D case).
          if (!m) { return new globalThis.DOMPoint(this.x, this.y, this.z, this.w); }
          return new globalThis.DOMPoint(
            m.a * this.x + m.c * this.y + m.e,
            m.b * this.x + m.d * this.y + m.f,
            this.z, this.w
          );
        };
        globalThis.DOMPoint.prototype.toJSON = function () {
          return { x: this.x, y: this.y, z: this.z, w: this.w };
        };
        globalThis.DOMPoint.fromPoint = function (p) {
          p = p || {};
          return new globalThis.DOMPoint(p.x, p.y, p.z, p.w);
        };
        globalThis.DOMPointReadOnly = globalThis.DOMPoint;
      }

      // `DOMMatrix` — the 2D affine transform math class. `canvas.getContext('2d').getTransform()`
      // returns one, charting/graphics libraries build transforms with it, and CSS Typed OM hands it
      // back. It was ABSENT, so `new DOMMatrix(...)` threw `DOMMatrix is not defined`. This is a real
      // (pure, honest) 2D implementation — identity, array/`matrix(...)`-string construction, the
      // a-f + m11..m42 accessors, multiply/translate/scale/rotate (non-mutating, returning a new
      // matrix, as the spec's `*Self`-less methods do), inverse, transformPoint and the serialisers.
      // 3D (`m13..m44`, `is2D:false`) is not modelled — the honest limit noted in the wiki.
      if (typeof globalThis.DOMMatrix === 'undefined') {
        var __mkMatrix = function (a, b, c, d, e, f) {
          var m = Object.create(globalThis.DOMMatrix.prototype);
          m.a = m.m11 = a; m.b = m.m12 = b; m.c = m.m21 = c; m.d = m.m22 = d; m.e = m.m41 = e; m.f = m.m42 = f;
          m.is2D = true;
          m.isIdentity = (a === 1 && b === 0 && c === 0 && d === 1 && e === 0 && f === 0);
          return m;
        };
        globalThis.DOMMatrix = function DOMMatrix(init) {
          var a = 1, b = 0, c = 0, d = 1, e = 0, f = 0;
          if (Array.isArray(init) || (init && typeof init.length === 'number' && typeof init !== 'string')) {
            if (init.length === 6) { a = init[0]; b = init[1]; c = init[2]; d = init[3]; e = init[4]; f = init[5]; }
            else if (init.length === 16) { a = init[0]; b = init[1]; c = init[4]; d = init[5]; e = init[12]; f = init[13]; }
          } else if (typeof init === 'string') {
            var s = init.trim();
            if (s === '' || s === 'none') { /* identity */ }
            else {
              var mm = s.match(/matrix\(\s*([^)]+)\)/);
              if (mm) {
                var n = mm[1].split(',').map(function (x) { return parseFloat(x); });
                if (n.length === 6) { a = n[0]; b = n[1]; c = n[2]; d = n[3]; e = n[4]; f = n[5]; }
              } else { throw new SyntaxError('Failed to parse DOMMatrix from string'); }
            }
          }
          this.a = this.m11 = a; this.b = this.m12 = b; this.c = this.m21 = c; this.d = this.m22 = d;
          this.e = this.m41 = e; this.f = this.m42 = f;
          this.is2D = true;
          this.isIdentity = (a === 1 && b === 0 && c === 0 && d === 1 && e === 0 && f === 0);
        };
        var P = globalThis.DOMMatrix.prototype;
        // this * other  (both 2D affine): compose `other` AFTER `this`, per the spec.
        P.multiply = function (o) {
          return __mkMatrix(
            this.a * o.a + this.c * o.b,
            this.b * o.a + this.d * o.b,
            this.a * o.c + this.c * o.d,
            this.b * o.c + this.d * o.d,
            this.a * o.e + this.c * o.f + this.e,
            this.b * o.e + this.d * o.f + this.f
          );
        };
        P.translate = function (tx, ty) {
          return this.multiply(__mkMatrix(1, 0, 0, 1, tx || 0, ty || 0));
        };
        P.scale = function (sx, sy) {
          if (sy === undefined) { sy = sx; }
          return this.multiply(__mkMatrix(sx, 0, 0, sy, 0, 0));
        };
        P.rotate = function (deg) {
          var rad = (deg || 0) * Math.PI / 180, cs = Math.cos(rad), sn = Math.sin(rad);
          return this.multiply(__mkMatrix(cs, sn, -sn, cs, 0, 0));
        };
        P.inverse = function () {
          var det = this.a * this.d - this.b * this.c;
          if (det === 0) { return __mkMatrix(NaN, NaN, NaN, NaN, NaN, NaN); }
          var ia = this.d / det, ib = -this.b / det, ic = -this.c / det, id = this.a / det;
          return __mkMatrix(ia, ib, ic, id,
            -(this.e * ia + this.f * ic), -(this.e * ib + this.f * id));
        };
        P.transformPoint = function (pt) {
          pt = pt || {};
          var x = pt.x || 0, y = pt.y || 0;
          // Return a real DOMPoint (added just above), matching the spec, so a caller can chain
          // `.matrixTransform(...)` or read `.w` — not a bare object literal.
          return new globalThis.DOMPoint(
            this.a * x + this.c * y + this.e, this.b * x + this.d * y + this.f,
            pt.z || 0, (pt.w === undefined) ? 1 : pt.w
          );
        };
        P.toFloat64Array = function () {
          return new Float64Array([this.a, this.b, 0, 0, this.c, this.d, 0, 0, 0, 0, 1, 0, this.e, this.f, 0, 1]);
        };
        P.toFloat32Array = function () { return new Float32Array(this.toFloat64Array()); };
        P.toString = function () {
          return 'matrix(' + this.a + ', ' + this.b + ', ' + this.c + ', ' + this.d + ', ' + this.e + ', ' + this.f + ')';
        };
        // `DOMMatrixReadOnly` shares the surface; `DOMMatrix.fromMatrix` is the spec's copy constructor.
        globalThis.DOMMatrixReadOnly = globalThis.DOMMatrix;
        globalThis.DOMMatrix.fromMatrix = function (o) {
          o = o || {};
          return __mkMatrix(o.a != null ? o.a : 1, o.b || 0, o.c || 0, o.d != null ? o.d : 1, o.e || 0, o.f || 0);
        };
      }

      // `DOMQuad` — four `DOMPoint`s, the shape `element.getBoxQuads()` and transform code produce when
      // a rectangle has been rotated/skewed into a general quadrilateral (its corners are no longer
      // axis-aligned). It completes the geometry family (DOMMatrix/DOMPoint/DOMRect already here). It
      // was absent, so `DOMQuad.fromRect(...)` / `new DOMQuad(...)` threw. `getBounds()` returns the
      // axis-aligned `DOMRect` bounding box — the useful reduction after a transform.
      if (typeof globalThis.DOMQuad === 'undefined') {
        var __toPoint = function (p) {
          return (p instanceof globalThis.DOMPoint) ? p : new globalThis.DOMPoint(p && p.x, p && p.y, p && p.z, p && p.w);
        };
        globalThis.DOMQuad = function DOMQuad(p1, p2, p3, p4) {
          this.p1 = __toPoint(p1 || {}); this.p2 = __toPoint(p2 || {});
          this.p3 = __toPoint(p3 || {}); this.p4 = __toPoint(p4 || {});
        };
        globalThis.DOMQuad.prototype.getBounds = function () {
          var xs = [this.p1.x, this.p2.x, this.p3.x, this.p4.x];
          var ys = [this.p1.y, this.p2.y, this.p3.y, this.p4.y];
          var minX = Math.min.apply(null, xs), maxX = Math.max.apply(null, xs);
          var minY = Math.min.apply(null, ys), maxY = Math.max.apply(null, ys);
          return new globalThis.DOMRect(minX, minY, maxX - minX, maxY - minY);
        };
        globalThis.DOMQuad.prototype.toJSON = function () {
          return { p1: this.p1.toJSON(), p2: this.p2.toJSON(), p3: this.p3.toJSON(), p4: this.p4.toJSON() };
        };
        // `fromRect({x,y,width,height})` — the four corners, clockwise from the top-left.
        globalThis.DOMQuad.fromRect = function (r) {
          r = r || {};
          var x = r.x || 0, y = r.y || 0, w = r.width || 0, h = r.height || 0;
          return new globalThis.DOMQuad(
            new globalThis.DOMPoint(x, y), new globalThis.DOMPoint(x + w, y),
            new globalThis.DOMPoint(x + w, y + h), new globalThis.DOMPoint(x, y + h)
          );
        };
        globalThis.DOMQuad.fromQuad = function (q) {
          q = q || {};
          return new globalThis.DOMQuad(q.p1, q.p2, q.p3, q.p4);
        };
      }

      // `Path2D` — a reusable, declared-once path object. `new Path2D()` builds one imperatively
      // (moveTo/lineTo/arc/bezierCurveTo/…), `new Path2D(other)` copies another, and — the form that
      // matters most — `new Path2D("M10 10 L20 20 …")` parses an SVG path-data string. Icon systems
      // (Lucide, Feather, Material), Chart.js/D3 shape generators and every "draw this glyph on a
      // canvas" helper hand a pre-built path to `ctx.fill(path)` / `ctx.stroke(path)` rather than
      // re-issuing segments each frame. It was ABSENT, so `new Path2D(...)` threw `Path2D is not
      // defined` and the whole draw routine died. The command stream produced here is the SAME flat
      // `[op, args…]` format the 2D context accumulates (0 moveTo · 1 lineTo · 2 quadTo · 3 cubicTo ·
      // 4 close · 5 rect), so `ctx.fill(path)` rasterizes it through the existing single native call.
      if (typeof globalThis.Path2D === 'undefined') {
        // Flatten a circular arc into lineTo segments — same granularity (π/8) the context's own
        // `ctx.arc` uses, so a Path2D arc and a context arc land on the same pixels.
        var __arcCmds = function (cmds, cx, cy, r, a0, a1, ccw) {
          cx = +cx || 0; cy = +cy || 0; r = +r || 0; a0 = +a0 || 0; a1 = +a1 || 0;
          var span = a1 - a0;
          if (ccw) { if (span > 0) { span -= 2 * Math.PI; } } else { if (span < 0) { span += 2 * Math.PI; } }
          var n = Math.max(2, Math.ceil(Math.abs(span) / (Math.PI / 8)));
          for (var i = 0; i <= n; i++) {
            var t = a0 + span * (i / n);
            cmds.push(i === 0 ? 0 : 1, cx + r * Math.cos(t), cy + r * Math.sin(t));
          }
        };
        // SVG elliptical-arc (`A`/`a`) → line segments, via the endpoint-to-center conversion in the
        // SVG spec's implementation notes (F.6.5). This is what makes real icon paths — which lean on
        // `A` for every rounded corner and circle — render instead of collapsing to a straight chord.
        var __svgArc = function (cmds, x1, y1, rx, ry, phiDeg, laf, sf, x2, y2) {
          rx = Math.abs(rx); ry = Math.abs(ry);
          if (rx === 0 || ry === 0 || (x1 === x2 && y1 === y2)) { cmds.push(1, x2, y2); return; }
          var phi = phiDeg * Math.PI / 180, cp = Math.cos(phi), sp = Math.sin(phi);
          var dx = (x1 - x2) / 2, dy = (y1 - y2) / 2;
          var x1p = cp * dx + sp * dy, y1p = -sp * dx + cp * dy;
          var lam = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
          if (lam > 1) { var sc = Math.sqrt(lam); rx *= sc; ry *= sc; }
          var sign = (laf !== sf) ? 1 : -1;
          var num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
          var den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
          var co = sign * Math.sqrt(Math.max(0, num / den));
          var cxp = co * rx * y1p / ry, cyp = -co * ry * x1p / rx;
          var cx = cp * cxp - sp * cyp + (x1 + x2) / 2, cy = sp * cxp + cp * cyp + (y1 + y2) / 2;
          var ang = function (ux, uy, vx, vy) {
            var dot = ux * vx + uy * vy, len = Math.sqrt((ux * ux + uy * uy) * (vx * vx + vy * vy));
            var a = Math.acos(Math.max(-1, Math.min(1, len === 0 ? 1 : dot / len)));
            return (ux * vy - uy * vx < 0) ? -a : a;
          };
          var ux = (x1p - cxp) / rx, uy = (y1p - cyp) / ry, vx = (-x1p - cxp) / rx, vy = (-y1p - cyp) / ry;
          var theta1 = ang(1, 0, ux, uy), dtheta = ang(ux, uy, vx, vy);
          if (!sf && dtheta > 0) { dtheta -= 2 * Math.PI; }
          else if (sf && dtheta < 0) { dtheta += 2 * Math.PI; }
          var n = Math.max(2, Math.ceil(Math.abs(dtheta) / (Math.PI / 8)));
          for (var k = 1; k <= n; k++) {
            var th = theta1 + dtheta * (k / n);
            cmds.push(1,
              cx + rx * Math.cos(th) * cp - ry * Math.sin(th) * sp,
              cy + rx * Math.cos(th) * sp + ry * Math.sin(th) * cp);
          }
        };
        // Tokenize + walk an SVG path-data string. Handles M/L/H/V/C/S/Q/T/A/Z in both cases (absolute
        // and relative), implicit command repetition, and S/T control-point reflection.
        var __parseSvgPath = function (cmds, d) {
          var re = /([MmLlHhVvCcSsQqTtAaZz])|(-?(?:\d*\.\d+|\d+\.?)(?:[eE][-+]?\d+)?)/g;
          var toks = [], mm;
          while ((mm = re.exec(String(d)))) { toks.push(mm[1] != null ? mm[1] : parseFloat(mm[2])); }
          var i = 0, cmd = '', px = 0, py = 0, sx = 0, sy = 0, rcx = 0, rcy = 0, prev = '';
          var num = function () { return +toks[i++] || 0; };
          while (i < toks.length) {
            if (typeof toks[i] === 'string') { cmd = toks[i]; i++; }
            else if (cmd === 'M') { cmd = 'L'; } else if (cmd === 'm') { cmd = 'l'; }
            var rel = (cmd >= 'a'), C = cmd.toUpperCase();
            if (C === 'Z') { cmds.push(4); px = sx; py = sy; prev = C; continue; }
            if (i >= toks.length || typeof toks[i] === 'string') { if (!cmd) { break; } continue; }
            if (C === 'M') {
              var x = num() + (rel ? px : 0), y = num() + (rel ? py : 0);
              cmds.push(0, x, y); px = x; py = y; sx = x; sy = y;
            } else if (C === 'L') {
              var lx = num() + (rel ? px : 0), ly = num() + (rel ? py : 0);
              cmds.push(1, lx, ly); px = lx; py = ly;
            } else if (C === 'H') {
              var hx = num() + (rel ? px : 0); cmds.push(1, hx, py); px = hx;
            } else if (C === 'V') {
              var vy2 = num() + (rel ? py : 0); cmds.push(1, px, vy2); py = vy2;
            } else if (C === 'C') {
              var c1x = num() + (rel ? px : 0), c1y = num() + (rel ? py : 0);
              var c2x = num() + (rel ? px : 0), c2y = num() + (rel ? py : 0);
              var ex = num() + (rel ? px : 0), ey = num() + (rel ? py : 0);
              cmds.push(3, c1x, c1y, c2x, c2y, ex, ey); rcx = c2x; rcy = c2y; px = ex; py = ey;
            } else if (C === 'S') {
              var r1x = (prev === 'C' || prev === 'S') ? 2 * px - rcx : px;
              var r1y = (prev === 'C' || prev === 'S') ? 2 * py - rcy : py;
              var s2x = num() + (rel ? px : 0), s2y = num() + (rel ? py : 0);
              var sex = num() + (rel ? px : 0), sey = num() + (rel ? py : 0);
              cmds.push(3, r1x, r1y, s2x, s2y, sex, sey); rcx = s2x; rcy = s2y; px = sex; py = sey;
            } else if (C === 'Q') {
              var qcx = num() + (rel ? px : 0), qcy = num() + (rel ? py : 0);
              var qex = num() + (rel ? px : 0), qey = num() + (rel ? py : 0);
              cmds.push(2, qcx, qcy, qex, qey); rcx = qcx; rcy = qcy; px = qex; py = qey;
            } else if (C === 'T') {
              var tcx = (prev === 'Q' || prev === 'T') ? 2 * px - rcx : px;
              var tcy = (prev === 'Q' || prev === 'T') ? 2 * py - rcy : py;
              var tex = num() + (rel ? px : 0), tey = num() + (rel ? py : 0);
              cmds.push(2, tcx, tcy, tex, tey); rcx = tcx; rcy = tcy; px = tex; py = tey;
            } else if (C === 'A') {
              var arx = num(), ary = num(), rot = num(), laf = num(), sf = num();
              var aex = num() + (rel ? px : 0), aey = num() + (rel ? py : 0);
              __svgArc(cmds, px, py, arx, ary, rot, laf, sf, aex, aey); px = aex; py = aey;
            } else { break; }
            prev = C;
          }
        };
        globalThis.Path2D = function Path2D(init) {
          this.__cmds = [];
          if (init && init.__cmds) { this.__cmds = init.__cmds.slice(); }
          else if (typeof init === 'string') { __parseSvgPath(this.__cmds, init); }
        };
        var PP = globalThis.Path2D.prototype;
        PP.moveTo = function (x, y) { this.__cmds.push(0, +x || 0, +y || 0); };
        PP.lineTo = function (x, y) { this.__cmds.push(1, +x || 0, +y || 0); };
        PP.quadraticCurveTo = function (cx, cy, x, y) { this.__cmds.push(2, +cx || 0, +cy || 0, +x || 0, +y || 0); };
        PP.bezierCurveTo = function (a, b, c, d, e, f) { this.__cmds.push(3, +a || 0, +b || 0, +c || 0, +d || 0, +e || 0, +f || 0); };
        PP.rect = function (x, y, w, h) { this.__cmds.push(5, +x || 0, +y || 0, +w || 0, +h || 0); };
        PP.roundRect = function (x, y, w, h) { this.__cmds.push(5, +x || 0, +y || 0, +w || 0, +h || 0); };
        PP.closePath = function () { this.__cmds.push(4); };
        PP.arc = function (cx, cy, r, a0, a1, ccw) { __arcCmds(this.__cmds, cx, cy, r, a0, a1, ccw); };
        PP.ellipse = function (cx, cy, rx, ry, rot, a0, a1, ccw) {
          __arcCmds(this.__cmds, cx, cy, Math.max(+rx || 0, +ry || 0), a0, a1, ccw);
        };
        PP.arcTo = function (x1, y1, x2, y2) { this.lineTo(x1, y1); this.lineTo(x2, y2); };
        // `addPath(path, transform?)` — append another path, optionally through a DOMMatrix. Under a
        // transform each coordinate pair is mapped; a `rect` op becomes a closed 4-line polygon because
        // a rotated/skewed rectangle is no longer axis-aligned.
        PP.addPath = function (path, tf) {
          if (!path || !path.__cmds) { return; }
          var src = path.__cmds;
          if (!tf) { for (var j = 0; j < src.length; j++) { this.__cmds.push(src[j]); } return; }
          var a = tf.a, b = tf.b, c = tf.c, d = tf.d, e = tf.e, f = tf.f;
          var self = this;
          var tx = function (x, y) { return [a * x + c * y + e, b * x + d * y + f]; };
          var k = 0;
          while (k < src.length) {
            var op = src[k++];
            if (op === 0 || op === 1) { var p = tx(src[k], src[k + 1]); self.__cmds.push(op, p[0], p[1]); k += 2; }
            else if (op === 2) { var p1 = tx(src[k], src[k + 1]), p2 = tx(src[k + 2], src[k + 3]); self.__cmds.push(2, p1[0], p1[1], p2[0], p2[1]); k += 4; }
            else if (op === 3) { var q1 = tx(src[k], src[k + 1]), q2 = tx(src[k + 2], src[k + 3]), q3 = tx(src[k + 4], src[k + 5]); self.__cmds.push(3, q1[0], q1[1], q2[0], q2[1], q3[0], q3[1]); k += 6; }
            else if (op === 4) { self.__cmds.push(4); }
            else if (op === 5) {
              var rx = src[k], ry = src[k + 1], rw = src[k + 2], rh = src[k + 3]; k += 4;
              var c0 = tx(rx, ry), c1 = tx(rx + rw, ry), c2 = tx(rx + rw, ry + rh), c3 = tx(rx, ry + rh);
              self.__cmds.push(0, c0[0], c0[1], 1, c1[0], c1[1], 1, c2[0], c2[1], 1, c3[0], c3[1], 4);
            }
          }
        };
      }

      // `createImageBitmap` — decode/snapshot a drawable into an `ImageBitmap` you hand back to
      // `ctx.drawImage(bmp, …)`. Games/texture uploaders (Pixi/Three), image editors and tile
      // renderers call `createImageBitmap(imgOrCanvas).then(b => ctx.drawImage(b, …))` to get a
      // ready-to-blit source without an intermediate element. It was ABSENT, so the call threw
      // `createImageBitmap is not a function`. Our image-source registry is keyed by NODE, and both
      // `<img>` (decoded bytes) and `<canvas>` (live bitmap) already publish pixels under their node
      // id — so a bitmap of one of those sources is just that node id plus an optional crop rect, with
      // ZERO new decode path. `ctx.drawImage` already accepts anything carrying `__nodeId`.
      if (typeof globalThis.ImageBitmap === 'undefined') {
        globalThis.ImageBitmap = function ImageBitmap() { throw new TypeError('Illegal constructor'); };
      }
      if (typeof globalThis.createImageBitmap === 'undefined') {
        globalThis.createImageBitmap = function (source, sx, sy, sw, sh) {
          var args = arguments;
          return new Promise(function (resolve, reject) {
            if (!source) { reject(new TypeError('createImageBitmap: the source is null')); return; }
            var id = source.__nodeId;
            // Blob / ImageData / SVG-image sources need a real decode-to-pixels path we do not have
            // yet — reject LOUDLY (an unhandled rejection or a caught error) rather than hand back a
            // silently-blank bitmap, which is the worse shape of failure. The honest follow-on.
            if (id == null) {
              var msg = 'createImageBitmap: Blob/ImageData sources are not decodable yet';
              reject(typeof DOMException !== 'undefined' ? new DOMException(msg, 'InvalidStateError') : new Error(msg));
              return;
            }
            // The source's own base rect: a source that is ITSELF a cropped ImageBitmap composes.
            var baseX = 0, baseY = 0, fullW = 0, fullH = 0;
            if (source.__crop) { baseX = source.__crop[0]; baseY = source.__crop[1]; fullW = source.__crop[2]; fullH = source.__crop[3]; }
            else if (typeof source.__cvSourceSize === 'function') {
              var sz = source.__cvSourceSize(id);
              if (sz) { fullW = sz[0]; fullH = sz[1]; }
            }
            if (!fullW) { fullW = source.naturalWidth || source.width || 0; fullH = source.naturalHeight || source.height || 0; }
            var bmp = Object.create(globalThis.ImageBitmap.prototype);
            bmp.__nodeId = id;
            // The crop overload: `createImageBitmap(source, sx, sy, sw, sh)`. The rect is relative to
            // the source's own base, so offset by (baseX, baseY) into the underlying node's pixels.
            if (args.length >= 5 && typeof args[1] === 'number') {
              var cx = baseX + (+sx || 0), cy = baseY + (+sy || 0), cw = +sw || 0, ch = +sh || 0;
              bmp.__crop = [cx, cy, cw, ch];
              bmp.width = Math.abs(cw); bmp.height = Math.abs(ch);
            } else {
              if (source.__crop) { bmp.__crop = source.__crop.slice(); }
              bmp.width = fullW; bmp.height = fullH;
            }
            // `close()` releases the handle — after it, the bitmap draws nothing (spec: detached).
            bmp.close = function () { this.__nodeId = null; this.width = 0; this.height = 0; };
            resolve(bmp);
          });
        };
      }

      // `URLPattern` — the URL matcher SPA routers and Service Worker routing use to dispatch a request
      // by shape: `new URLPattern({ pathname: '/users/:id' }).exec(url).pathname.groups.id`. It was
      // absent, so `new URLPattern(...)` threw. This is a real (honest) matcher for the PATHNAME
      // component — the one routers actually key on — compiling `:name` to a named capture and `*` to a
      // greedy wildcard, with `.test()` and `.exec()`. Other components (protocol/hostname/search) are
      // matched permissively; full multi-component matching is the honest follow-on noted in the wiki.
      if (typeof globalThis.URLPattern === 'undefined') {
        var __compilePattern = function (pattern) {
          var names = [], rx = '', wild = 0, i = 0;
          while (i < pattern.length) {
            var ch = pattern.charAt(i);
            if (ch === ':') {
              var j = i + 1, name = '';
              while (j < pattern.length && /[A-Za-z0-9_]/.test(pattern.charAt(j))) { name += pattern.charAt(j); j++; }
              names.push(name); rx += '([^/]+)'; i = j;
            } else if (ch === '*') {
              names.push(String(wild++)); rx += '(.*)'; i++;
            } else {
              rx += ch.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'); i++;
            }
          }
          return { regex: new RegExp('^' + rx + '$'), names: names };
        };
        var __pathnameOf = function (input) {
          if (input == null) { return '/'; }
          if (typeof input === 'string') {
            try { return new globalThis.URL(input, 'https://x.invalid/').pathname; }
            catch (e) { return input; }   // a bare pathname like '/a/b'
          }
          return (input.pathname != null) ? input.pathname : '/';
        };
        globalThis.URLPattern = function URLPattern(init) {
          if (typeof init === 'string') { init = { pathname: init }; }
          init = init || {};
          this.pathname = (init.pathname != null) ? init.pathname : '*';
          this.__c = __compilePattern(this.pathname);
        };
        globalThis.URLPattern.prototype.test = function (input) {
          return this.__c.regex.test(__pathnameOf(input));
        };
        globalThis.URLPattern.prototype.exec = function (input) {
          var path = __pathnameOf(input);
          var m = this.__c.regex.exec(path);
          if (!m) { return null; }
          var groups = {};
          for (var k = 0; k < this.__c.names.length; k++) { groups[this.__c.names[k]] = m[k + 1]; }
          return { pathname: { input: path, groups: groups } };
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
        // The `label` used to be IGNORED — every TextDecoder decoded UTF-8, so `new
        // TextDecoder('latin1')` or `'utf-16le'` returned mojibake on the legacy/Windows and UTF-16
        // content that is still all over the web (Windows-authored CSV/HTML, `.decode()` of a
        // non-UTF-8 response, binary protocols that frame text as UTF-16). Now the label is normalised
        // to one of the encodings we implement; an unknown label falls back to UTF-8 (lenient rather
        // than the spec's RangeError, so nothing that limped before newly throws).
        globalThis.TextDecoder = function TextDecoder(label, options){
          var norm = String(label == null ? 'utf-8' : label).trim().toLowerCase();
          var enc;
          if (norm === 'utf-8' || norm === 'utf8' || norm === 'unicode-1-1-utf-8' || norm === '') { enc = 'utf-8'; }
          else if (norm === 'utf-16' || norm === 'utf-16le' || norm === 'ucs-2' || norm === 'unicode' || norm === 'csunicode') { enc = 'utf-16le'; }
          else if (norm === 'utf-16be' || norm === 'unicodefffe') { enc = 'utf-16be'; }
          else if (norm === 'latin1' || norm === 'iso-8859-1' || norm === 'iso8859-1' || norm === 'iso88591'
                   || norm === 'l1' || norm === 'latin-1' || norm === 'cp819' || norm === 'ibm819'
                   || norm === 'iso-ir-100' || norm === 'windows-1252' || norm === 'cp1252' || norm === 'x-cp1252'
                   || norm === 'ansi_x3.4-1968' || norm === 'ascii' || norm === 'us-ascii') { enc = 'windows-1252'; }
          else { enc = 'utf-8'; } // unsupported label → decode as UTF-8 (strictly more capable than the old ignore)
          // The `encoding` attribute reports the canonical name (utf-16le collapses the utf-16/ucs-2 aliases).
          this.encoding = (enc === 'windows-1252') ? 'windows-1252' : enc;
          this.__enc = enc;
          this.__tail = null;
          this.fatal = !!(options && options.fatal);
          this.ignoreBOM = !!(options && options.ignoreBOM);
        };
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
          var enc = this.__enc || 'utf-8';

          // ── windows-1252 (the latin1/iso-8859-1 family): one byte per character, no partial
          // sequences, so streaming needs nothing held back. 0x00-0x7F and 0xA0-0xFF map straight to
          // the code point equal to the byte; 0x80-0x9F are the CP1252 punctuation/symbol block (€ ' " —
          // … ™ etc.), the difference that makes it windows-1252 and not raw latin1.
          if (enc === 'windows-1252') {
            var hi = [0x20AC,0x81,0x201A,0x192,0x201E,0x2026,0x2020,0x2021,0x2C6,0x2030,0x160,0x2039,0x152,0x8D,0x17D,0x8F,
                      0x90,0x2018,0x2019,0x201C,0x201D,0x2022,0x2013,0x2014,0x2DC,0x2122,0x161,0x203A,0x153,0x9D,0x17E,0x178];
            var s1 = '';
            for (var j = 0; j < end; j++) {
              var by = b[j];
              s1 += String.fromCharCode((by >= 0x80 && by <= 0x9F) ? hi[by - 0x80] : by);
            }
            return s1;
          }

          // ── UTF-16 (LE or BE): two bytes per code unit. Surrogate PAIRS are already the JS string's
          // native form, so each 16-bit unit is emitted as-is. A trailing ODD byte under {stream:true}
          // is an incomplete unit — hold it for the next call.
          if (enc === 'utf-16le' || enc === 'utf-16be') {
            var le = (enc === 'utf-16le');
            if (streaming && (end & 1)) { this.__tail = new Uint8Array(b.subarray(end - 1, end)); end -= 1; }
            var s2 = '';
            for (var k = 0; k + 1 < end; k += 2) {
              var unit = le ? (b[k] | (b[k + 1] << 8)) : ((b[k] << 8) | b[k + 1]);
              s2 += String.fromCharCode(unit);
            }
            return s2;
          }

          // ── UTF-8 (default). Hold an incomplete trailing multi-byte sequence under {stream:true}.
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
        // Hash `bytes` with the host's pure-Rust digest, returning a Uint8Array. The building block for
        // HMAC below (which is just SHA over key-padded blocks — a standard composition of the existing
        // correct hash, NOT a hand-rolled primitive).
        var __digestBytes = function(algo, bytes) {
          var ih = '';
          for (var i = 0; i < bytes.length; i++) { ih += ('0' + bytes[i].toString(16)).slice(-2); }
          var oh = __subtleDigestHex(algo, ih);
          var out = new Uint8Array(oh.length / 2);
          for (var j = 0; j < out.length; j++) { out[j] = parseInt(oh.slice(j * 2, j * 2 + 2), 16); }
          return out;
        };
        // HMAC (RFC 2104): H((k⊕opad) || H((k⊕ipad) || msg)), key hashed/zero-padded to the block size.
        var __hmac = function(algo, keyBytes, msgBytes) {
          var B = (algo === 'SHA-384' || algo === 'SHA-512') ? 128 : 64;
          var key = keyBytes;
          if (key.length > B) { key = __digestBytes(algo, key); }
          var k = new Uint8Array(B);
          k.set(key);
          var inner = new Uint8Array(B + msgBytes.length);
          for (var i = 0; i < B; i++) { inner[i] = k[i] ^ 0x36; }
          inner.set(msgBytes, B);
          var innerHash = __digestBytes(algo, inner);
          var outer = new Uint8Array(B + innerHash.length);
          for (var j = 0; j < B; j++) { outer[j] = k[j] ^ 0x5c; }
          outer.set(innerHash, B);
          return __digestBytes(algo, outer);
        };
        var __asBytes = function(v) {
          if (v instanceof ArrayBuffer) { return new Uint8Array(v); }
          if (v && v.buffer instanceof ArrayBuffer) { return new Uint8Array(v.buffer, v.byteOffset, v.byteLength); }
          throw new TypeError('not a BufferSource');
        };
        var __hashName = function(algorithm) {
          var h = algorithm && (algorithm.hash || algorithm);
          return String((h && h.name) || h || 'SHA-256').toUpperCase();
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
            },
            // `importKey('raw', keyBytes, {name:'HMAC', hash:'SHA-256'}, ...)` — the key half of the
            // HMAC path every webhook-signature check and HS256 JWT verifier uses. Returns a CryptoKey
            // holding the raw secret. Only raw HMAC keys are supported (asymmetric/derive stay absent).
            importKey: function(format, keyData, algorithm, extractable, keyUsages) {
              var name = String((algorithm && algorithm.name) || algorithm || '').toUpperCase();
              if (name !== 'HMAC' && name !== 'HKDF') { return Promise.reject(__mkCryptoErr('NotSupportedError', 'Only HMAC/HKDF importKey is supported: ' + name)); }
              if (String(format) !== 'raw') { return Promise.reject(__mkCryptoErr('NotSupportedError', "Only 'raw' key format is supported")); }
              var kb;
              try { kb = __asBytes(keyData); } catch (e) { return Promise.reject(new TypeError('keyData is not a BufferSource')); }
              var alg = (name === 'HKDF') ? { name: 'HKDF' } : { name: 'HMAC', hash: { name: __hashName(algorithm) } };
              return Promise.resolve({
                type: 'secret', extractable: !!extractable, algorithm: alg,
                usages: (keyUsages || []).slice(), __raw: kb
              });
            },
            // `deriveBits({name:'HKDF', hash, salt, info}, key, lengthBits)` — HKDF (RFC 5869), the key
            // derivation modern protocols and token schemes use to expand one secret into keying
            // material. It is Extract-then-Expand, both built on the HMAC above — so, like HMAC, a pure
            // composition of the existing hash, gate-verified against the RFC 5869 known-answer vectors.
            deriveBits: function(algorithm, key, length) {
              var name = String((algorithm && algorithm.name) || '').toUpperCase();
              if (name !== 'HKDF') { return Promise.reject(__mkCryptoErr('NotSupportedError', 'Only HKDF deriveBits is supported: ' + name)); }
              try {
                var hash = __hashName(algorithm);
                var hlen = ({ 'SHA-1': 20, 'SHA-256': 32, 'SHA-384': 48, 'SHA-512': 64 })[hash] || 32;
                var salt = algorithm.salt ? __asBytes(algorithm.salt) : new Uint8Array(hlen);
                var info = algorithm.info ? __asBytes(algorithm.info) : new Uint8Array(0);
                var L = (length || 0) / 8;
                var prk = __hmac(hash, salt, key.__raw);        // Extract
                var okm = new Uint8Array(L), t = new Uint8Array(0), pos = 0, i = 1;
                while (pos < L) {                               // Expand
                  var input = new Uint8Array(t.length + info.length + 1);
                  input.set(t); input.set(info, t.length); input[t.length + info.length] = i;
                  t = __hmac(hash, prk, input);
                  var take = Math.min(t.length, L - pos);
                  okm.set(t.subarray(0, take), pos);
                  pos += take; i++;
                }
                var out = new ArrayBuffer(L);
                new Uint8Array(out).set(okm);
                return Promise.resolve(out);
              } catch (e) { return Promise.reject(e); }
            },
            // `sign('HMAC', key, data)` → the MAC as an ArrayBuffer. Composes the host SHA (RustCrypto)
            // into HMAC — provably correct against the RFC 4231 test vectors the gate checks.
            sign: function(algorithm, key, data) {
              var name = String((algorithm && algorithm.name) || algorithm || '').toUpperCase();
              if (name !== 'HMAC') { return Promise.reject(__mkCryptoErr('NotSupportedError', 'Only HMAC sign is supported: ' + name)); }
              try {
                var mac = __hmac(__hashName(key.algorithm), key.__raw, __asBytes(data));
                var out = new ArrayBuffer(mac.length);
                new Uint8Array(out).set(mac);
                return Promise.resolve(out);
              } catch (e) { return Promise.reject(e); }
            },
            // `verify('HMAC', key, signature, data)` → boolean. Recomputes the MAC and compares in
            // constant time (a timing-variable compare is a classic signature-check flaw).
            verify: function(algorithm, key, signature, data) {
              return this.sign(algorithm, key, data).then(function(expected) {
                var a = new Uint8Array(expected), b;
                try { b = __asBytes(signature); } catch (e) { return false; }
                if (a.length !== b.length) { return false; }
                var diff = 0;
                for (var i = 0; i < a.length; i++) { diff |= a[i] ^ b[i]; }
                return diff === 0;
              });
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

      // `contenteditable` — the editability QUERY surface (Tier-1 rich-editing subsystem, brick 1).
      //
      // Reflection + computed inheritance ONLY: `el.contentEditable` (the enumerated IDL attribute),
      // `el.isContentEditable` (computed up the ancestor chain, falling back to `document.designMode`),
      // and `document.designMode`. Every rich-text editor DETECTS an editable host before it initialises —
      // ProseMirror, Slate, Draft, TinyMCE, CKEditor all branch on `el.isContentEditable` — and it was
      // `undefined` (falsy) on a `<div contenteditable>`, so editor mounts and `contenteditable`-detection
      // libraries read the element as plain. This makes DETECTION honest. It does NOT claim the keystroke/
      // `execCommand` editing path, which is separately and honestly still absent — a later brick.
      if (__HP && typeof __HP.isContentEditable === 'undefined') {
        // The element's OWN contenteditable state, ignoring inheritance:
        // 'true' | 'false' | 'plaintext-only' | 'inherit' (attribute absent or an unknown value).
        var __ceState = function (el) {
          if (!el || typeof el.getAttribute !== 'function' ||
              typeof el.hasAttribute !== 'function' || !el.hasAttribute('contenteditable')) {
            return 'inherit';
          }
          var v = el.getAttribute('contenteditable');
          if (v == null) { return 'inherit'; }
          v = String(v).toLowerCase();
          if (v === '' || v === 'true') { return 'true'; }
          if (v === 'false') { return 'false'; }
          if (v === 'plaintext-only') { return 'plaintext-only'; }
          return 'inherit';
        };
        Object.defineProperty(__HP, 'contentEditable', {
          configurable: true,
          get: function () { return __ceState(this); },
          set: function (val) {
            var v = (val == null ? '' : String(val)).toLowerCase();
            if (v === 'inherit') { this.removeAttribute('contenteditable'); return; }
            if (v === 'true') { this.setAttribute('contenteditable', 'true'); return; }
            if (v === 'false') { this.setAttribute('contenteditable', 'false'); return; }
            if (v === 'plaintext-only') { this.setAttribute('contenteditable', 'plaintext-only'); return; }
            throw new DOMException(
              "Failed to set the 'contentEditable' property: The value provided ('" + String(val) +
              "') is not one of 'true', 'false', 'plaintext-only' or 'inherit'.", 'SyntaxError');
          }
        });
        Object.defineProperty(__HP, 'isContentEditable', {
          configurable: true,
          get: function () {
            // The nearest ancestor (self included) with an EXPLICIT true/false/plaintext-only state wins;
            // 'inherit' keeps walking up. With no explicit state anywhere, the document decides.
            for (var n = this; n && typeof n === 'object'; n = n.parentElement) {
              var s = __ceState(n);
              if (s === 'true' || s === 'plaintext-only') { return true; }
              if (s === 'false') { return false; }
            }
            try { return !!(this.ownerDocument && this.ownerDocument.designMode === 'on'); }
            catch (e) { return false; }
          }
        });
      }
      // `document.designMode` — 'on' makes the WHOLE document editable (body.isContentEditable ⇒ true);
      // default 'off'. ASCII-case-insensitive; any non-'on' value is 'off' (the spec's parse).
      try {
        if (typeof document.designMode === 'undefined') {
          document.__designMode = 'off';
          Object.defineProperty(document, 'designMode', {
            configurable: true,
            get: function () { return this.__designMode || 'off'; },
            set: function (v) { this.__designMode = (String(v).toLowerCase() === 'on') ? 'on' : 'off'; }
          });
        }
      } catch (e) {}

      // `CSS.escape` / `CSS.supports` — feature detection, and the correct way to build a selector.
      //
      // `supports` used to be `return true`, which is the worst available answer. Progressive
      // enhancement is built on this call: a page asks whether a property works, and on "yes" it
      // hides its fallback and commits to the modern path. Answering yes to everything meant the
      // fallback was thrown away for properties this engine ignores — `container-type`,
      // `view-transition-name`, `animation-timeline` — so the page rendered its enhanced layout
      // with none of the enhancement. A "no" would have kept the layout the author already shipped
      // and tested.
      //
      // It now asks the CSS engine, which is the same Stylo evaluation the cascade already runs for
      // `@supports`. The two were disagreeing about the identical declaration; now there is one
      // answer, reached through one evaluator.
      if (typeof globalThis.CSS === 'undefined') {
        globalThis.CSS = {
          escape: function(v) { return String(v).replace(/([^\w-])/g, '\\$1'); },
          // Both spec forms: `supports(conditionText)` and `supports(property, value)`.
          supports: function (a, b) {
            if (typeof __cssSupports !== 'function') { return false; }
            if (arguments.length >= 2) {
              var prop = String(a), val = String(b);
              // The 2-argument form takes a property and a value, NOT a condition — so a value
              // carrying its own parens/braces must not be able to close the probe and turn a
              // question into an injection.
              if (prop.indexOf('(') >= 0 || prop.indexOf(':') >= 0 || val === '') { return false; }
              return __cssSupports('(' + prop + ':' + val + ')');
            }
            return __cssSupports(String(a));
          }
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
    deliver_bytes(rt, global, id, status, body, None, headers)
}

/// As [`deliver`], but also carrying the response's **raw bytes**.
///
/// `raw` is the body as a binary string — one code unit per byte — and it is what
/// `arrayBuffer()`/`bytes()`/`response.body` and an `arraybuffer` XHR read. `body` stays the host's
/// charset-decoded text for `text()`/`json()`.
///
/// Both, because a `Response` has two genuine readings and neither derives from the other without
/// loss. Encoding the text back to UTF-8 (what happened before tick 228) inflates every byte above
/// `0x7F` into two, so a 260-byte media segment arrived as 407 and no demuxer could parse it;
/// decoding the bytes as UTF-8 in JS would instead discard the charset sniffing that makes a
/// legacy-encoded page readable.
pub fn deliver_bytes(
    rt: &mut Runtime,
    global: mozjs::rust::HandleObject,
    id: u32,
    status: u16,
    body: &str,
    raw: Option<&[u8]>,
    headers: &[(String, String)],
) -> Result<(), String> {
    // `js_bytes_literal` — the SAME one-char-per-byte convention the streaming chunk path and the
    // binary-upload path already use. Reused rather than re-derived: the streaming path was always
    // byte-safe, and this buffered one being the odd path out is precisely how the corruption
    // survived unnoticed.
    let raw_lit = raw.map(js_bytes_literal);
    let script = format!(
        "__deliver({}, {}, {}, {}, {})",
        id,
        status,
        js_string_literal(body),
        js_headers_literal(headers),
        raw_lit.as_deref().unwrap_or("undefined")
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
