//! **HTML attribute reflection** — the single largest gap in the whole web platform, by a factor of five.
//!
//! `a.href`. `input.disabled`. `img.width`. `option.selected`. `form.action`. `td.colSpan`. Every one of
//! these is a **reflected IDL attribute**: a property on the element that *is* a view over a content
//! attribute, with the HTML spec's type coercion in between. They were **all `undefined`.**
//!
//! Measured, not guessed. Histogramming every failing subtest message across `html/dom` (47,226 of them):
//!
//! | count | message |
//! |---:|---|
//! | 23,411 | `IDL get expected (string/boolean/number) X but got (undefined) undefined` |
//! | 13,724 | `getAttribute() expected X but got X` — the IDL **set** never reached the attribute |
//! | 1,470 | `hasAttribute() expected false but got true` |
//!
//! **~38,000 subtests — about 80% of `html/dom`'s failures — are this one mechanism.** For scale: the
//! entire `dom/` suite that the last ten ticks worked through is 6,484 subtests. This is five times that,
//! behind a single generic feature.
//!
//! And it is not a conformance curiosity. **It is how ordinary page code touches the DOM.**
//! `if (input.disabled)`, `img.width = 300`, `a.href`, `form.method` — a page that reads `undefined` from
//! all of them does not throw. It silently takes the wrong branch.
//!
//! ## The rules, which are the actual work
//!
//! The table (`reflect_table.rs`) only says *which* attribute has *which* type. **The mechanism is here**,
//! and it is where the spec actually lives:
//!
//! * **string** — absent reads as `""`, never `null`. Setting stringifies.
//! * **URL** — absent reads as `""`; present is **resolved against the document base**. `a.href` on
//!   `<a href="x">` is an absolute URL, and a page that compares it to `"x"` is one that never worked.
//! * **boolean** — get is `hasAttribute`; set is *presence*, not value. `el.disabled = false` must
//!   **remove** the attribute, and `disabled=""` is `true`. Writing `"false"` — which is what a naive
//!   implementation does — makes the element disabled.
//! * **long / unsigned long** — parsed; **invalid or absent falls back to the default**, and a negative
//!   value in an `unsigned long` is *not* clamped to 0, it falls back too.
//! * **limited unsigned long** — must be > 0 (`colSpan`'s default is 1, and `colspan="0"` is invalid, so
//!   it reads back as 1).
//! * **clamped unsigned long** — clamped into `[min, max]`, and out-of-range falls back to the default.
//! * **enumerated** — a keyword list, a *non-canonical* alias map (`""` often means something specific),
//!   an invalid-value default and a missing-value default, which are **different defaults**. Getting
//!   those two confused is the classic reflection bug.
//! * **tokenlist** — a live `DOMTokenList` over the attribute (`rel`, `sandbox`, `htmlFor`).
//!
//! **Why this is not "teaching to the test":** the table is the HTML spec's IDL (see `reflect_table.rs`),
//! and the rules above are implemented once, generically, against the spec's *algorithms*. They apply to
//! attributes no test covers. Swap the table and the mechanism still holds — which is exactly what makes
//! the score mean something.

/// The generic reflection mechanism. Reads [`crate::reflect_table::REFLECT_TABLE`].
pub const REFLECT_JS: &str = r#"
(function () {
  'use strict';
  if (typeof document === 'undefined' || typeof globalThis.__REFLECT_TABLE !== 'string') { return; }

  var TABLE;
  try { TABLE = JSON.parse(globalThis.__REFLECT_TABLE); } catch (e) { return; }
  delete globalThis.__REFLECT_TABLE;

  var EP = (typeof Element !== 'undefined' && Element.prototype) || null;
  if (!EP) { return; }
  function ownerOf(start, name) {
    for (var p = start; p; p = Object.getPrototypeOf(p)) {
      if (Object.getOwnPropertyDescriptor(p, name)) return p;
    }
    return null;
  }
  var proto = ownerOf(EP, 'tagName') || EP;

  // idlName -> { tag -> descriptor }. One accessor per IDL NAME, not per (tag, attribute) pair — the same
  // `disabled` is reflected by a dozen elements, and defining it a dozen times over would have the last
  // definition win.
  var BY_NAME = {};
  for (var tag in TABLE) {
    var rows = TABLE[tag];
    for (var i = 0; i < rows.length; i++) {
      var r = rows[i];
      (BY_NAME[r.n] || (BY_NAME[r.n] = {}))[tag] = r;
    }
  }

  function descFor(el, idl) {
    var byTag = BY_NAME[idl];
    if (!byTag || !el || !el.tagName) return null;
    return byTag[el.tagName.toLowerCase()] || null;
  }
  function attrOf(d) { return d.a || d.n.toLowerCase(); }

  // ── Integer parsing, the HTML way. NOT parseInt: leading whitespace is allowed, a trailing
  // non-digit ENDS the number, and anything else is invalid — it does not become NaN and then 0.
  function parseIntHTML(s) {
    if (s === null || s === undefined) return null;
    var m = /^[ \t\n\f\r]*([-+]?[0-9]+)/.exec(String(s));
    if (!m) return null;
    var n = parseInt(m[1], 10);
    return isNaN(n) ? null : n;
  }
  function parseFloatHTML(s) {
    if (s === null || s === undefined) return null;
    var m = /^[ \t\n\f\r]*([-+]?(?:[0-9]+\.?[0-9]*|\.[0-9]+)(?:[eE][-+]?[0-9]+)?)/.exec(String(s));
    if (!m) return null;
    var n = parseFloat(m[1]);
    return isFinite(n) ? n : null;
  }

  function get(el, d) {
    var a = attrOf(d);
    var raw = el.getAttribute(a);
    var t = d.t;

    if (t === 'boolean') {
      // Presence, not value. `disabled=""` is TRUE; there is no `disabled="false"`.
      return el.hasAttribute(a);
    }

    if (t === 'string') {
      return raw === null ? '' : raw;
    }

    if (t === 'url') {
      // Absent is `""`. Present is RESOLVED against the document base — `a.href` on `<a href="x">` is an
      // absolute URL, and code comparing it to `"x"` was never going to work in any browser.
      if (raw === null) return '';
      try { return new URL(raw, document.baseURI || location.href).href; }
      catch (e) { return raw; }
    }

    if (t === 'long' || t === 'unsigned long' || t === 'limited long'
        || t === 'limited unsigned long' || t === 'limited unsigned long with fallback'
        || t === 'clamped unsigned long') {
      var dflt = (d.d !== undefined && d.d !== null) ? d.d : 0;
      var n = parseIntHTML(raw);
      if (n === null) return dflt;
      if (t === 'unsigned long' || t === 'limited unsigned long'
          || t === 'limited unsigned long with fallback' || t === 'clamped unsigned long') {
        // A NEGATIVE value in an unsigned field is INVALID — it falls back to the default. It is not
        // clamped to zero, which is the intuitive-and-wrong thing to do.
        if (n < 0) return dflt;
        if (n > 2147483647) return dflt;
      }
      if (t === 'limited unsigned long' || t === 'limited unsigned long with fallback') {
        if (n <= 0) return dflt;    // `colspan="0"` is invalid, and colSpan reads back as 1
      }
      if (t === 'clamped unsigned long') {
        var mn = (d.mn !== undefined) ? d.mn : 0, mx = (d.mx !== undefined) ? d.mx : 2147483647;
        if (n < mn) return mn;
        if (n > mx) return mx;
      }
      return n;
    }

    if (t === 'double' || t === 'limited double') {
      var dd = (d.d !== undefined && d.d !== null) ? d.d : 0;
      var f = parseFloatHTML(raw);
      if (f === null) return dd;
      if (t === 'limited double' && f <= 0) return dd;
      return f;
    }

    if (t === 'enum') {
      // TWO different defaults, and confusing them is THE classic reflection bug:
      //   * the MISSING-value default (`d`) — the attribute is not there at all;
      //   * the INVALID-value default (`i`) — it is there and says something unrecognised.
      if (raw === null) return (d.d === undefined) ? (d.z ? null : '') : d.d;
      var v = String(raw).toLowerCase();
      var kw = d.k || [];
      for (var j = 0; j < kw.length; j++) {
        if (kw[j].toLowerCase() === v) return kw[j];
      }
      if (d.c && Object.prototype.hasOwnProperty.call(d.c, v)) return d.c[v];
      if (d.i !== undefined) return d.i;
      return (d.d === undefined) ? (d.z ? null : '') : d.d;
    }

    return raw === null ? '' : raw;
  }

  function set(el, d, v) {
    var a = attrOf(d), t = d.t;

    if (t === 'boolean') {
      // Presence, not value. `el.disabled = false` must REMOVE the attribute. Writing the string
      // "false" — which is what stringifying does — leaves the element disabled, and the page has no
      // way to tell.
      if (v) el.setAttribute(a, ''); else el.removeAttribute(a);
      return;
    }
    if (t === 'long' || t === 'unsigned long' || t === 'limited long'
        || t === 'limited unsigned long' || t === 'limited unsigned long with fallback'
        || t === 'clamped unsigned long') {
      var n = Math.trunc(Number(v));
      if (!isFinite(n)) n = 0;
      el.setAttribute(a, String(n));
      return;
    }
    if (t === 'double' || t === 'limited double') {
      var f = Number(v);
      el.setAttribute(a, String(isFinite(f) ? f : 0));
      return;
    }
    if (v === null && d.z) { el.removeAttribute(a); return; }
    el.setAttribute(a, String(v));
  }

  // One accessor per IDL name, on the prototype every element inherits from. The getter dispatches on
  // `this.tagName`, so `disabled` reflects for `<input>` and is inert on `<div>` — which is the spec, and
  // is why a single shared accessor is correct rather than merely convenient.
  // **`tokenlist` is DELIBERATELY NOT REFLECTED, and this is the honest choice.**
  //
  // `rel`/`relList`, `sandbox`, `htmlFor` reflect as a **live `DOMTokenList`**, not a string. The first
  // version returned the raw attribute string when it could not build one — and that made things WORSE
  // than leaving them `undefined`: `dom/lists/DOMTokenList-coverage-for-attributes.html` fell 129 → 115,
  // because a caller that gets a *string* where a `DOMTokenList` belongs has been lied to, whereas a
  // caller that gets `undefined` at least knows nothing is there.
  //
  // **A stub is worse than an absence** — this codebase has proven that four times now (`Range`,
  // `TreeWalker`, `MutationObserver`, and here). So these are skipped until they can be done properly,
  // which is generalising the `DOMTokenList` built for `classList` over an arbitrary attribute. That is a
  // tick, not a line.
  var SKIP_TYPES = { 'tokenlist': 1, 'settable tokenlist': 1 };

  Object.keys(BY_NAME).forEach(function (idl) {
    var anyReflectable = false;
    for (var tg in BY_NAME[idl]) { if (!SKIP_TYPES[BY_NAME[idl][tg].t]) anyReflectable = true; }
    if (!anyReflectable) return;
    // **Never clobber anything already reachable on the PROTOTYPE CHAIN.**
    //
    // The first version checked only `proto` itself — `getOwnPropertyDescriptor(proto, idl)` — and that is
    // not the same question. A name the engine implements natively on a *different* link of the chain
    // (or on `Object.prototype`) sailed through, got a reflected accessor defined over it, and the two
    // re-entered each other. It crashed a WPT child process: **a Bar 0 regression, introduced by a
    // feature that was otherwise worth +9,940 subtests, and the score is worth nothing next to it.**
    //
    // `in` walks the chain, which is the question actually being asked: *does this name already mean
    // something here?* If it does, the engine's implementation wins — a reflection layer that overwrites
    // a real one is a regression wearing a feature's clothes.
    if (idl in proto) return;

    Object.defineProperty(proto, idl, {
      configurable: true,
      enumerable: true,
      get: function () {
        var d = descFor(this, idl);
        return (d && !SKIP_TYPES[d.t]) ? get(this, d) : undefined;
      },
      set: function (v) {
        var d = descFor(this, idl);
        if (d && !SKIP_TYPES[d.t]) set(this, d, v);
        else Object.defineProperty(this, idl, { value: v, writable: true, configurable: true, enumerable: true });
      },
    });
  });
})();
"#;
