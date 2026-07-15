//! **Inline event-handler content attributes** — `onload`, `onclick`, `onsubmit`, and the rest.
//!
//! `<body onload="init()">`, `<button onclick="save()">`, `<form onsubmit="return validate()">`,
//! `<img onerror="this.src=fallback">`, `<a onclick="track()">`. Every one of these was **dead**: the
//! attribute was parsed and stored, and nothing ever compiled it into a handler. The button did nothing.
//! The form submitted raw. The page's `onload` bootstrap never ran.
//!
//! This is not a corner of the platform. It is one of the two oldest ways there are to attach behaviour
//! to markup, it predates `addEventListener` by a decade, and an enormous amount of the web — every
//! server-rendered form, every legacy admin panel, every email-client sandbox, every tutorial's first
//! example — still uses it as the *primary* mechanism.
//!
//! It surfaced from the encoding suite (767,003 subtests at 0%): WPT's decoder tests bootstrap with
//! `<body onload="showNodes(decoder)">`. `showNodes` was defined, the frame was loaded, the data was
//! readable — and `onload` never fired the call, so every file created zero subtests and reported as a
//! *timeout*. One dead attribute, the largest single area in WPT.
//!
//! ## The one rule that is not obvious
//!
//! **`on*` on `<body>` and `<frameset>` sets the *Window*'s handler, not the element's.**
//! `<body onload>` is `window.onload`; so are `onresize`, `onscroll`, `onblur`, `onfocus`, `onerror`,
//! `onhashchange`, `onpopstate`, `onstorage`, `onmessage`, `onbeforeunload`, `onunload`, `onpageshow`,
//! `onpagehide` and their kin. This is a real HTML parser rule, not an implementation shortcut, and it
//! is exactly the one the encoding suite depends on — `load` fires at the *window*, and a handler put on
//! the body element would never see it.
//!
//! Every other `on*`, and every window-mapped name on a non-body element, binds to the element via
//! `addEventListener`, which is where the dispatch machinery already looks.
//!
//! ## Compilation and scope
//!
//! The handler body is compiled once, with `event` as its parameter — `new Function('event', body)` —
//! and the compiled function is cached on the element so re-wiring is idempotent. The full HTML scope
//! chain (the element, its form, the document on the `with` stack) is a deliberate *non-goal* here: it
//! is rarely used, it is a correctness hazard, and the common shape — a call to a global, a reference to
//! `event`, `this` being the element — is handled exactly. A handler that throws is reported and
//! swallowed, never allowed to abort the wiring of the handlers after it.

/// Installed once per global. Exposes `__wireInlineHandlers()`, which the lifecycle calls at
/// DOMContentLoaded and again at load (idempotent).
pub const INLINE_HANDLERS_JS: &str = r#"
(function () {
  var g = globalThis;
  if (g.__wireInlineHandlers) return;

  // `on*` attributes that, on <body>/<frameset>, belong to the Window. This is the HTML spec's
  // "body element event handlers" set, plus the handful of UI events that are also window-scoped there.
  var WINDOW_EVENTS = {
    load: 1, unload: 1, beforeunload: 1, resize: 1, scroll: 1, blur: 1, focus: 1, error: 1,
    afterprint: 1, beforeprint: 1, hashchange: 1, languagechange: 1, message: 1, messageerror: 1,
    offline: 1, online: 1, pagehide: 1, pageshow: 1, popstate: 1, rejectionhandled: 1,
    storage: 1, unhandledrejection: 1
  };

  function compile(el, attr) {
    var code = el.getAttribute(attr);
    if (code == null) return null;
    try {
      // `event` is the sole formal parameter, matching the DOM. `this` is bound at call time.
      return new Function('event', code);
    } catch (e) {
      if (g.__reportError) g.__reportError(e);
      // A handler that will not even COMPILE must not be retried on every wiring pass, and must not
      // take out the elements after it. A no-op is the honest degraded form.
      return function () {};
    }
  }

  g.__wireInlineHandlers = function () {
    // **Only the elements that actually carry an `on*` attribute**, found by a single arena walk in Rust
    // (`__inlineHandlerNodes`). This deliberately avoids `document.querySelectorAll('*')`: iterating the
    // whole tree from JS and touching each element forces a reflector for every node, and with the
    // reflection layer installed that mass access trips a pathological re-entrancy that overflows the
    // stack — a Bar 0 crash. The native returns the handful of real matches and nothing else.
    var els;
    try { els = g.__inlineHandlerNodes ? g.__inlineHandlerNodes() : []; } catch (e) { return; }
    for (var i = 0; i < els.length; i++) {
      var el = els[i];
      var names;
      try { names = el.getAttributeNames ? el.getAttributeNames() : []; } catch (e) { continue; }
      var tag = (el.tagName || '').toUpperCase();
      var isBody = (tag === 'BODY' || tag === 'FRAMESET');
      for (var j = 0; j < names.length; j++) {
        var name = names[j];
        if (name.length < 3 || name.slice(0, 2) !== 'on') continue;
        var type = name.slice(2);
        // Compile-once, cached on the element. The reflector identity cache guarantees one object per
        // node, so an expando here is a correct per-node "already wired" mark — no global set needed.
        var mark = '__ih_' + name;
        if (el[mark]) continue;

        if (isBody && WINDOW_EVENTS[type]) {
          // <body onload> IS window.onload. Bind `this` to the window, as the spec does for a handler
          // that migrated to the Window object.
          var fn = compile(el, name);
          if (fn) {
            el[mark] = fn;
            (function (f) { g['on' + type] = function (ev) { return f.call(g, ev); }; })(fn);
          }
        } else {
          var hf = compile(el, name);
          if (hf) {
            el[mark] = hf;
            // Bind `this` to the element and forward `event`, then register where dispatch looks.
            (function (element, handler) {
              var listener = function (ev) { return handler.call(element, ev); };
              try { element.addEventListener(type, listener); } catch (e) {}
            })(el, hf);
          }
        }
      }
    }
  };
})();
"#;
