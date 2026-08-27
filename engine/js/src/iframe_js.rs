//! **The nested browsing context** — `iframe.contentDocument` and `iframe.contentWindow`.
//!
//! This is the single largest gated capability this project has found, and it was gated by an
//! *architectural* fact rather than a missing feature.
//!
//! The child `Page` was **always built**: a real arena, real styles, real scripts, laid out at the frame's
//! own viewport width. Then it was painted to a bitmap and **dropped on the floor**. The pixels survived;
//! the document did not. So `iframe.contentDocument` was `undefined`, and every script that reaches into a
//! frame — which is what a frame is *for* — got nothing.
//!
//! ## What it gates
//!
//! | | |
//! |---:|---|
//! | **767,003** | `encoding` subtests, 91% of the measured WPT universe, scoring **0%**. They load a document in a frame and read its text back out: `iframeRef(f) { return f.contentWindow ? f.contentWindow.document : f.contentDocument }`. |
//! | ~1,100 | further `dom/` and `html/` subtests that navigate a frame |
//! | **#1** | platform-web capability on the constellation: embeds, OAuth frames, payment fields, ads, video players, comment widgets. The platform web *is* other people's documents inside yours. |
//!
//! And it was invisible for eighty-three ticks because `encoding` looked like a *character-decoding*
//! problem, and our character decoding was already correct — verified directly, sniffing Big5, zero
//! U+FFFD. **The name of the area named the wrong organ.** (PROCESS — again.)
//!
//! ## The one hard part
//!
//! A node id is only unique **within an arena**. Two documents means node #7 exists twice. Everything
//! downstream of that — `node_and_dom`, the identity cache — had one arena baked into it
//! (`CURRENT_DOM`), so a child reflector resolved against the *parent's* arena and returned a different
//! element, from a different document, with complete confidence. That is not a bug you find by testing
//! `contentDocument`; it is the reason `contentDocument` could not be written. Fixing it took three
//! changes in `dom_bindings`: reflectors honour their own `SLOT_DOM`; a registry of live arenas makes
//! that safe (a dropped `Page`'s arena is a use-after-free, not a document); and the identity cache is
//! **per-arena**, or `===` starts lying across documents.
//!
//! ## Stated limits, because a stub is worse than an absence
//!
//! * The child shares the parent's JS global. So a script *inside* a frame is not isolated from its
//!   parent, and `contentWindow` is not a real `Window`: it carries `document`, `frameElement`,
//!   `location`, `window`/`self`/`parent`/`top` and the listener/`postMessage` no-ops as its OWN
//!   properties, and **inherits every platform interface object from the parent global** through a
//!   Proxy (t1201). That inheritance is the literal truth about this engine rather than a
//!   pretence — one realm means `frameWin.Node === Node` — but it is also exactly why a separate
//!   realm per frame remains owed. Cross-origin restrictions are **not** enforced.
//! * ✅ **`getComputedStyle` is PRESENT and arena-correct since t1202** (`FRAME_STYLES`). ⚠ This
//!   bullet used to say it was *deliberately absent* because `STYLES_PTR` held one page's map — that
//!   text outlived the fix by ~28 ticks and was still blaming the wrong organ when t1230 re-measured
//!   it. **A stale limitation reads exactly like a current one**, and this one would have sent the
//!   next tick at a seam that was already correct.
//! * ⚠⚠⚠ **THE REAL REMAINING DEFECT IS NARROWER AND WAS MIS-ATTRIBUTED TO THE ABOVE: a node
//!   inserted into a frame's document AFTER the frame's initial cascade has NO computed style at
//!   all — not a wrong one, not an initial one.** The frame document is cascaded once when it loads
//!   and never again, so its style map has no entry for a later node and `getComputedStyle(n).display`
//!   is `undefined` rather than `"block"`. Measured t1230, with the CONTROL that names the organ:
//!
//!   ```text
//!     gCS present on the frame window                              PASS
//!     ORIGINAL frame node, frame's OWN sheet   -> "hidden"         PASS  <- CONTROL: the arena
//!     ORIGINAL frame node, display             -> "block"          PASS  <-   lookup is CORRECT
//!     PARENT-CREATED node in the frame, display -> undefined       FAIL  <- the defect
//!     PARENT-CREATED node, + a new rule         -> undefined       FAIL
//!   ```
//!
//!   The two passing ORIGINAL rows are what rule out the arena lookup and the frame's own
//!   stylesheet; the variable is **WHEN the node was inserted**, exactly the t1186 shape (*"do not
//!   ask whether layout runs — ask which re-entry the read happened in"*). This is what
//!   `css/selectors/attribute-selectors/attribute-case` dies on: its helper iterates
//!   `[window, quirks, xml]`, and for the two frame globals it CREATES the element it then measures,
//!   inside the parent's `load` handler. **726 failing subtests in that one directory**, and the
//!   top failure message across it is `expected (string) "hidden" but got (undefined) undefined`.
//!   ⚠ `global.mode` on a frame window answers the PARENT's `mode` (one realm, per the note above),
//!   so all 726 are *named* `"in standards mode"` — the name does NOT say which arm failed, and
//!   bucketing by it will mislead.
//! * The frame does not re-render when its document is mutated from the parent. The DOM is live and
//!   readable; the *pixels* are a snapshot. Painting a mutated frame is its own tick — and it is the
//!   same missing re-cascade as the bullet above, seen from the paint side rather than the CSSOM side.
//!
//! Both are written down here rather than papered over, because a `contentWindow` that pretended to be a
//! Window would be feature-detected, registered against, and would silently never work — which is the
//! failure this project has now made five separate times.

/// Installed once per global, after the DOM bindings.
pub const IFRAME_JS: &str = r#"
(function () {
  var G = globalThis;
  var EP = (typeof Element !== 'undefined' && Element.prototype) || null;
  if (!EP || !EP.__iframeDoc) return;

  // Only these elements have a nested browsing context. `div.contentDocument` must stay `undefined` —
  // a property that exists and answers `null` is a feature-detection trap, and code that asks
  // `if ('contentDocument' in el)` would take the wrong branch on every element on the page.
  var FRAMES = { IFRAME: 1, FRAME: 1, OBJECT: 1, EMBED: 1 };
  function frameDoc(el) {
    var t = el && el.tagName;
    if (!t || !FRAMES[t]) return undefined;
    return el.__iframeDoc() || null;
  }

  Object.defineProperty(EP, 'contentDocument', {
    configurable: true,
    get: function () { return frameDoc(this); }
  });

  // `contentWindow` is the proxy the real world reaches through:
  //   f.contentWindow ? f.contentWindow.document : f.contentDocument
  // is the exact line in WPT's own `encoding/resources/decode-common.js`, and it is the same line in a
  // thousand embed scripts. It is NOT a Window — see the module docs. It carries what is actually
  // touched, and nothing it cannot honour.
  Object.defineProperty(EP, 'contentWindow', {
    configurable: true,
    get: function () {
      var d = frameDoc(this);
      if (d === undefined) return undefined;
      if (d === null) return null;
      var el = this;
      // **ONE window object per frame.** This built a fresh object literal on EVERY read, so
      // `f.contentWindow === f.contentWindow` was false and anything a script stashed on the
      // window — a ready flag, a message-port handle, a resize callback, the bookkeeping every
      // embed and OAuth frame keeps — was written to an object thrown away on the next line.
      // Same rule as `el.sheet` and `f.contentDocument`; it was missing in all three.
      if (el.__manukWin) { return el.__manukWin; }
      var own = {
        // A GETTER, not the `d` captured above: the window object now outlives a single read, and a
        // frame that NAVIGATES gets a new document. Caching the value here would buy identity by
        // making the window permanently stale — the same live-and-stable pair `sheet.cssRules`
        // needed.
        get document() { return frameDoc(el); },
        frameElement: el,
        // A frame's window is a global-ish object, and scripts poke at these before anything else.
        get location() { return { href: el.getAttribute('src') || 'about:blank' }; },
        addEventListener: function () {},
        removeEventListener: function () {},
        postMessage: function () {}
      };

      // ── **A FRAME'S WINDOW HAD TWO PROPERTIES, AND ONE OF THEM WAS `location`.**
      //
      // Measured inside the harness: `Node`, `Element`, `HTMLElement`, `Event`, `Document`,
      // `DOMException`, `window`, `self` — every platform global — was `undefined` on this object.
      // A script inside a frame, and every script that reaches INTO one, works through
      // `d.defaultView`, so the whole platform vanished at the frame boundary. WPT's
      // `assert_throws_dom(type, root.ownerDocument.defaultView.DOMException, fn)` could not even
      // EXPRESS its assertion: 204 `dom` and 76 `css/selectors` subtests died reading `.name` off
      // `undefined`, before any behaviour was tested.
      //
      // ⚠ **Inheriting the parent's globals here is not a pretence — it is the literal truth about
      // this engine.** The child SHARES the parent's JS global (stated in the module docs above), so
      // `frameWin.Node` and `Node` really are the same object, and `e instanceof frameWin.DOMException`
      // really is the right answer for an exception this realm threw. A separate realm per frame is a
      // different, larger piece of work; until it exists, ONE realm honestly reported beats a window
      // with two properties.
      //
      // A Proxy rather than a prototype chain, for two reasons a prototype cannot give:
      //   * `getComputedStyle` must stay **ABSENT**, not merely shadowed by `undefined` — a property
      //     that exists and answers `undefined` is the feature-detection trap this file already
      //     refuses elsewhere. `STYLES_PTR` is a single thread-local holding ONE page's style map, so
      //     a frame node looked up there gets the PARENT's style: exposing it turns a documented
      //     absence into a silently wrong answer. `has` reports false and `get` returns undefined.
      //   * a `set` must land on the frame's OWN object. Every embed stashes a ready flag or a
      //     message-port handle on its window; with a prototype chain those writes create own
      //     properties here anyway, but the `has`/`getOwnPropertyDescriptor` answers would still be
      //     the parent's, and `Object.keys(frameWin)` would enumerate the whole parent global.
      // ⚠ **THE DENY LIST IS NOW EMPTY, and that is a RETIREMENT, not a relaxation (t1202).**
      // `getComputedStyle` was withheld here because `STYLES_PTR` held ONE page's style map, so a
      // frame element resolved against the PARENT's styles — a wrong answer of the right type, which
      // is worse than an absence. The lookup is arena-aware as of t1202 (`FRAME_STYLES`), so the
      // reason is gone and the property comes back. The mechanism stays in place because the NEXT
      // wrong-answer-of-the-right-type on this seam should be excluded the same way rather than
      // shipped.
      var DENY = {};
      var win = new Proxy(own, {
        get: function (t, k, r) {
          if (Reflect.has(t, k) || DENY[k]) { return Reflect.get(t, k, r); }
          return globalThis[k];
        },
        has: function (t, k) {
          if (Reflect.has(t, k)) { return true; }
          if (DENY[k]) { return false; }
          return k in globalThis;
        },
        set: function (t, k, v) { return Reflect.set(t, k, v); },
        deleteProperty: function (t, k) { return Reflect.deleteProperty(t, k); },
        ownKeys: function (t) { return Reflect.ownKeys(t); }
      });

      // `window`/`self` are the frame's OWN window — the single most-read property on a window
      // object, and the one a prototype chain would have silently answered with the PARENT's.
      // `parent`/`top` point at the containing global, which for a one-level frame is the truth.
      own.window = win;
      own.self = win;
      own.parent = globalThis;
      own.top = globalThis;

      Object.defineProperty(el, '__manukWin', {
        value: win, configurable: true, enumerable: false, writable: false
      });
      return win;
    }
  });

  // The other direction: a document inside a frame points back at the element that frames it. Null for
  // the top-level document, which is the check a script uses to ask "am I in a frame?".
  if (typeof Document !== 'undefined' && Document.prototype &&
      !('defaultView' in Document.prototype)) {
    Object.defineProperty(Document.prototype, 'defaultView', {
      configurable: true,
      get: function () {
        if (this === document) return globalThis;
        // **A FRAMED document's view is its frame's window** — it was a flat `null` for every
        // document that was not the singleton, so `iframeDoc.defaultView.postMessage(…)` and
        // `d.defaultView.location` (the way a script inside-out addresses its own frame, and the
        // idiom every embed uses to talk back) died on `null`.
        //
        // ⚠ This is only implementable because `contentDocument` identity now holds: the owning
        // frame is found by COMPARING documents, and until this tick that comparison was false
        // against the very document it was handed.
        var els = document.getElementsByTagName ? document.getElementsByTagName('iframe') : [];
        for (var i = 0; i < els.length; i++) {
          if (els[i].contentDocument === this) { return els[i].contentWindow; }
        }
        // A document with no frame — `createHTMLDocument`, `parseFromString` — genuinely has no
        // view, and `null` is the spec's answer for it, not a fallback.
        return null;
      }
    });
  }

  // ── ⚠⚠⚠ **`window.frames[0]` AND `window.length` — THE INDEXED HALF OF THE FRAME TREE.**
  //
  // Named access already worked (`window['one']` for `<iframe name=one>`), and INDEXED access did
  // not exist at all: `window.frames.length` and `window[0]` were both `undefined`. That is the
  // half every real script uses, because a frame's name is the embedder's business and its index is
  // the embedder's own:
  //
  //   for (var i = 0; i < window.length; i++) { window.frames[i].postMessage(msg, '*'); }
  //   if (window.length) { /* I have child frames */ }
  //   top.frames[0].location = …
  //
  // Measured against headless Chrome on two named `<iframe srcdoc>`s:
  //
  // ```text
  //                              Chrome    before
  //   typeof window.frames       object    object   ✓
  //   window.frames.length            2  undefined  ✗
  //   typeof window.frames[0]    object  undefined  ✗
  //   typeof window[0]           object  undefined  ✗
  //   window.length                   2  undefined  ✗
  //   frames[0].parent === window  true  TypeError  ✗
  //   frames[0].document…text       'a'  TypeError  ✗
  //   typeof window['one']       object    object   ✓  named access, already there
  //   window.frames === window     true      true   ✓
  // ```
  //
  // ⚠ `window.frames === window` already held, so ONE mechanism serves both spellings: defining the
  // indices on the global is what makes `frames[0]` work.
  //
  // ⭐ **THE INDICES ARE ACCESSORS OVER A LIVE QUERY, AND `length` SYNCS THEM.** A frame added by
  // script after this file ran must still be reachable, and there is no way to intercept an
  // arbitrary index on the global without making the global a Proxy (which would tax every property
  // read on every page). So each index is an accessor that re-reads the DOM when called — always
  // live — and `length`'s getter EXTENDS the accessor range before answering. That makes the loop
  // above correct by construction: reading `length` is what creates the indices it is about to be
  // used to walk. The high-water mark only grows, so an index that once existed keeps answering
  // (`undefined` once the frame is gone), which is what a stale reference should read.
  //
  // ⚠ NON-ENUMERABLE, and it is a deliberate small divergence: Chrome's are enumerable own
  // properties of the WindowProxy. Making ours enumerable would put `"0"`, `"1"`, … into
  // `Object.keys(window)` and every `for…in` over the global — a much larger lie than the one it
  // fixes, and one that breaks library feature-sniffing rather than a frame walk.
  //
  // ⚠ Only frames with a live browsing context are counted: `contentWindow` is `undefined` for a
  // frame whose document is not installed, and the spec counts child browsing contexts, not
  // elements. A `<frame>` inside a `<frameset>` counts the same way, which is why both tags are
  // queried.
  function __frameWins() {
    try {
      var all = document.querySelectorAll('iframe, frame');
      var out = [];
      for (var i = 0; i < all.length; i++) {
        var w = all[i].contentWindow;
        if (w) { out.push(w); }
      }
      return out;
    } catch (e) { return []; }
  }
  var __hwm = 0;
  G.__syncFrames = function () {
    var n = __frameWins().length;
    while (__hwm < n) {
      (function (i) {
        Object.defineProperty(G, String(i), {
          configurable: true,
          enumerable: false,
          get: function () { return __frameWins()[i]; }
        });
      })(__hwm);
      __hwm++;
    }
    return n;
  };
  Object.defineProperty(G, 'length', {
    configurable: true,
    enumerable: false,
    get: function () { return G.__syncFrames(); }
  });
})();
"#;
