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
//!   parent, and `contentWindow` is not a real `Window` — it carries `document`, `frameElement`,
//!   `location` and the listener/`postMessage` no-ops, which is what the code in the wild actually
//!   touches. Cross-origin restrictions are **not** enforced.
//! * ⚠⚠ **`getComputedStyle` is deliberately ABSENT from that window, and the reason is a defect
//!   one layer down, not an oversight.** `STYLES_PTR` is a single thread-local holding ONE page's
//!   style map, and `window_get_computed_style` keeps only the `NodeId` from `node_and_dom`,
//!   discarding the arena — so a child node is looked up in the PARENT's map. Measured on a frame
//!   whose own stylesheet sets `visibility:hidden`: `getComputedStyle(childEl).visibility` is
//!   `visible` (the child's stylesheet never reached it) and a *plain* child element reports
//!   `undefined`. That is the wrong-answer-of-the-right-type shape `el_content_document`'s doc
//!   comment describes, one document boundary further out. Exposing `getComputedStyle` here would
//!   turn *absent* into *silently wrong*; it stays off until the style lookup is arena-aware.
//!   Worth ~480 subtests in `css/selectors/attribute-selectors/attribute-case`, whose helper
//!   iterates `[window, quirks, xml]` — two of them frame windows — and dies on
//!   `global.getComputedStyle is not a function`.
//! * The frame does not re-render when its document is mutated from the parent. The DOM is live and
//!   readable; the *pixels* are a snapshot. Painting a mutated frame is its own tick.
//!
//! Both are written down here rather than papered over, because a `contentWindow` that pretended to be a
//! Window would be feature-detected, registered against, and would silently never work — which is the
//! failure this project has now made five separate times.

/// Installed once per global, after the DOM bindings.
pub const IFRAME_JS: &str = r#"
(function () {
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
      var win = {
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
})();
"#;
