//! **`Animatable` lives on `Element`, and ours was installed on a SUBCLASS of it.**
//!
//! `element.animate(...)` worked. Every element had it, calling it did the right thing, and the gate
//! that exercises it was green. What did **not** work is the only line the web actually writes to ask
//! whether the API is there:
//!
//! ```js
//!   'animate' in Element.prototype        // false
//! ```
//!
//! The prelude installs the Web Animations shim on `Object.getPrototypeOf(document.createElement('div'))`
//! — measured, that is **`HTMLElement.prototype`**, one link *below* `Element` on the chain
//! `HTMLElement → Element → Node → EventTarget`. Every element inherits from there, so nothing looked
//! broken; but `in` walks **up**, never down, so asking `Element.prototype` about a property defined on
//! its subclass answers `false`. That is a **FALSE ABSENCE**, which the reliability doctrine ranks
//! beside a false presence for exactly this reason: a detect that answers `false` about a feature you
//! have makes the caller take the fallback branch forever, silently, with nothing thrown or logged.
//!
//! ⚠⚠⚠ **It cost 909 failing subtests in `css/css-transforms` alone.** WPT's
//! `css/support/interpolation-testcommon.js` gates its whole Web Animations leg on
//! `isSupported: function() {return 'animate' in Element.prototype;}`, and that leg runs in every one of
//! the 194 `*-interpolation.html` files across twelve CSS areas.
//!
//! ⚠ **What this does NOT buy, measured rather than assumed.** The obvious second claim — *"and SVG was
//! broken, because `SVGElement` chains to `Element` without passing through `HTMLElement`"* — is **true
//! of the spec and false of this engine**, and it was written into an earlier draft of this file before
//! it was checked. Every node here shares ONE chain: a `<rect>`, and a **Text node**, both walk
//! `HTMLElement.prototype → Element.prototype → Node.prototype`, so `rect.animate` already worked and a
//! text node already answered `'animate' in node` — a pre-existing false presence this module does not
//! create and does not fix. Moving the property one link up changes neither. The whole benefit is the
//! detect, and saying so is the point.
//!
//! ⚠⚠ **Why this is a separate, LATE module and not a one-word change at the install site.** The obvious
//! fix — install on `g.Element.prototype` in the prelude — was tried, built and measured, and it is
//! **worse**: at prelude time the prototype chain is not yet assembled, and the object
//! `Element.prototype` names then is the shared tier that later becomes `Node.prototype`. `'animate' in
//! Element.prototype` still read `false` afterwards. The chain has to exist first, so the move happens
//! here, after `reflect.js`, once every link is real.
//!
//! The move is a **relocation, not a redefinition**: the descriptor is carried across verbatim and
//! deleted from the link it was on, so the implementation, its closed-over `WeakMap` of running
//! animations, and `Animation` identity are untouched. Nothing about the shim's honest limit changes
//! here — it still fast-forwards to the end state rather than tweening (see `dom_bindings.rs`), and this
//! module makes no claim about that.

/// Re-home `animate` / `getAnimations` from the subclass the prelude reached onto `Element.prototype`.
pub const ANIMATABLE_JS: &str = r#"
(function () {
  'use strict';
  if (typeof Element === 'undefined' || !Element.prototype || typeof document === 'undefined') { return; }
  var EP = Element.prototype;
  var NAMES = ['animate', 'getAnimations'];

  // Already home, or already on an ANCESTOR of Element: `in` can see it either way, so there is nothing
  // to fix and defining a second copy over a working one is how ordering bugs are made.
  for (var q = EP; q; q = Object.getPrototypeOf(q)) {
    if (Object.getOwnPropertyDescriptor(q, 'animate')) { return; }
  }

  // Otherwise it is on a SUBCLASS — which is invisible to `'animate' in Element.prototype` and is the
  // whole defect. Find it by walking a real element's chain DOWNWARD-to-upward and stopping at Element:
  // anything found before we reach `EP` is below it.
  var probe;
  try { probe = document.createElement('div'); } catch (e) { return; }
  for (var p = Object.getPrototypeOf(probe); p && p !== EP; p = Object.getPrototypeOf(p)) {
    if (!Object.getOwnPropertyDescriptor(p, 'animate')) { continue; }
    for (var i = 0; i < NAMES.length; i++) {
      var n = NAMES[i];
      var d = Object.getOwnPropertyDescriptor(p, n);
      if (!d) { continue; }
      try {
        Object.defineProperty(EP, n, d);
        // DELETE from the subclass. A relocation that only copies is a duplication, and two homes for
        // one method is the shape that lets a later patch land on the copy nobody calls.
        delete p[n];
      } catch (e) { /* non-configurable: leave the working inherited copy alone */ }
    }
    return;
  }
})();
"#;
