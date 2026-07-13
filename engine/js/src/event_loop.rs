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
    globalThis.setTimeout = function(cb, ms) {
        if (typeof cb !== 'function') { return 0; }
        var id = ++__timerId;
        __tasks.push(function(){ if (!__cancelled[id]) { cb(); } });
        return id;
    };
    globalThis.clearTimeout = function(id) { if (id) { __cancelled[id] = true; } };
    globalThis.setInterval = function(cb, ms) {
        if (typeof cb !== 'function') { return 0; }
        var id = ++__timerId;
        var tick = function() {
            if (__cancelled[id]) { return; }
            cb();
            if (!__cancelled[id]) { __tasks.push(tick); }
        };
        __tasks.push(tick);
        return id;
    };
    globalThis.clearInterval = function(id) { if (id) { __cancelled[id] = true; } };
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

      iface('Node', isNode);
      iface('Element', isEl);
      iface('HTMLElement', isEl);
      iface('SVGElement', function(o){ return isEl(o) && o.tagName === 'SVG'; });
      iface('Text', function(o){ return !!o && o.nodeType === 3; });

      iface('Comment', function(o){ return !!o && o.nodeType === 8; });
      iface('DocumentFragment', function(o){ return !!o && o.nodeType === 11; });

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
        var warned = false;
        el.getContext = function(kind) {
          kind = String(kind || '2d').toLowerCase();
          if (kind !== '2d') {
            return null;   // the spec's "cannot": every library already handles a null WebGL context
          }
          if (!warned) {
            warned = true;
            console.warn('canvas 2D drawing is not rasterized yet — the canvas will be blank, but the page runs');
          }
          if (el.__ctx) { return el.__ctx; }
          var noop = function(){};
          var ctx = {
            canvas: el,
            // State
            save: noop, restore: noop, scale: noop, rotate: noop, translate: noop, transform: noop,
            setTransform: noop, resetTransform: noop,
            // Paths
            beginPath: noop, closePath: noop, moveTo: noop, lineTo: noop, bezierCurveTo: noop,
            quadraticCurveTo: noop, arc: noop, arcTo: noop, ellipse: noop, rect: noop, roundRect: noop,
            fill: noop, stroke: noop, clip: noop, isPointInPath: function(){ return false; },
            // Rects
            clearRect: noop, fillRect: noop, strokeRect: noop,
            // Text — `measureText` must return a real shape, because layout code multiplies by `.width`
            // and `undefined * n` is NaN, which propagates into every coordinate downstream.
            fillText: noop, strokeText: noop,
            measureText: function(t){
              var w = String(t == null ? '' : t).length * 7;
              return { width: w, actualBoundingBoxLeft: 0, actualBoundingBoxRight: w,
                       actualBoundingBoxAscent: 10, actualBoundingBoxDescent: 3,
                       fontBoundingBoxAscent: 10, fontBoundingBoxDescent: 3 };
            },
            // Images / pixels
            drawImage: noop,
            createImageData: function(w, h){
              w = w|0; h = h|0;
              return { width: w, height: h, data: new Uint8ClampedArray(Math.max(w*h*4, 0)) };
            },
            getImageData: function(x, y, w, h){ return ctx.createImageData(w, h); },
            putImageData: noop,
            // Gradients / patterns — objects, because scripts assign them to fillStyle and then use them
            createLinearGradient: function(){ return { addColorStop: noop }; },
            createRadialGradient: function(){ return { addColorStop: noop }; },
            createConicGradient: function(){ return { addColorStop: noop }; },
            createPattern: function(){ return null; },
            // Line/shadow/composite state — plain writable properties, which is what they are
            fillStyle: '#000', strokeStyle: '#000', lineWidth: 1, lineCap: 'butt', lineJoin: 'miter',
            miterLimit: 10, lineDashOffset: 0, setLineDash: noop, getLineDash: function(){ return []; },
            font: '10px sans-serif', textAlign: 'start', textBaseline: 'alphabetic', direction: 'inherit',
            globalAlpha: 1, globalCompositeOperation: 'source-over', imageSmoothingEnabled: true,
            shadowBlur: 0, shadowColor: 'rgba(0,0,0,0)', shadowOffsetX: 0, shadowOffsetY: 0,
            filter: 'none',
          };
          Object.defineProperty(el, '__ctx', { value: ctx, enumerable: false });
          return ctx;
        };
        el.toDataURL = function(){ return 'data:image/png;base64,'; };
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
        ro('readyState', 0);        // HAVE_NOTHING
        ro('networkState', 3);      // NETWORK_NO_SOURCE
        ro('paused', true);
        ro('ended', false);
        ro('seeking', false);
        ro('duration', NaN);
        ro('buffered',  { length: 0, start: function(){ return 0; }, end: function(){ return 0; } });
        ro('played',    { length: 0, start: function(){ return 0; }, end: function(){ return 0; } });
        ro('seekable',  { length: 0, start: function(){ return 0; }, end: function(){ return 0; } });
        ro('textTracks', []);
        ro('videoWidth', 0);
        ro('videoHeight', 0);

        // Writable, because scripts set them and expect them to stick. They just do not do anything.
        el.currentTime = 0; el.volume = 1; el.muted = false; el.playbackRate = 1;
        el.autoplay = el.autoplay || false; el.loop = el.loop || false;

        el.HAVE_NOTHING = 0; el.HAVE_METADATA = 1; el.HAVE_CURRENT_DATA = 2;
        el.HAVE_FUTURE_DATA = 3; el.HAVE_ENOUGH_DATA = 4;
        el.NETWORK_EMPTY = 0; el.NETWORK_IDLE = 1; el.NETWORK_LOADING = 2; el.NETWORK_NO_SOURCE = 3;

        // `''` is the spec's "no". `'probably'` / `'maybe'` are the only other answers, and both
        // would be lies.
        el.canPlayType = function() { return ''; };

        el.play = function() {
          var e = new Error('media playback is not supported by this browser');
          e.name = 'NotSupportedError';
          return Promise.reject(e);
        };
        el.pause = function() {};
        el.load  = function() {};
        el.addTextTrack = function() { return { cues: [], activeCues: [], mode: 'disabled' }; };
        el.requestPictureInPicture = function() {
          var e = new Error('picture-in-picture is not supported');
          e.name = 'NotSupportedError';
          return Promise.reject(e);
        };
        return el;
      };

      // Install the bridges once every interface exists (see `bridge` above).
      try {
        bridge(globalThis.Node.prototype, NODE_ACCESSORS);
        bridge(globalThis.Element.prototype, NODE_ACCESSORS.concat(EL_ACCESSORS));
        bridge(globalThis.HTMLElement.prototype, NODE_ACCESSORS.concat(EL_ACCESSORS));
        bridge(globalThis.Text.prototype, NODE_ACCESSORS.concat(CD_ACCESSORS));
        bridge(globalThis.Comment.prototype, NODE_ACCESSORS.concat(CD_ACCESSORS));
        bridge(globalThis.DocumentFragment.prototype, NODE_ACCESSORS);
      } catch (e) { /* a frozen builtin prototype: leave it alone */ }
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
          var s = new AbortSignal(); s.aborted = true; s.reason = reason; return s;
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
          s.reason = reason !== undefined ? reason : new Error('AbortError');
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
        globalThis.TextDecoder = function TextDecoder(){ this.encoding = 'utf-8'; };
        globalThis.TextDecoder.prototype.decode = function(buf) {
          if (!buf) { return ''; }
          var b = buf instanceof Uint8Array ? buf : new Uint8Array(buf);
          var s = '', i = 0;
          while (i < b.length) {
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

      // `crypto.randomUUID` is now everywhere (React keys, request ids, cache busting).
      if (typeof globalThis.crypto === 'undefined') {
        globalThis.crypto = {
          getRandomValues: function(a) {
            for (var i = 0; i < a.length; i++) { a[i] = (Math.random() * 256) | 0; }
            return a;
          },
          randomUUID: function() {
            var h = '0123456789abcdef', s = '';
            for (var i = 0; i < 36; i++) {
              s += (i === 8 || i === 13 || i === 18 || i === 23) ? '-'
                 : (i === 14) ? '4'
                 : h[(Math.random() * 16) | 0];
            }
            return s;
          }
        };
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
