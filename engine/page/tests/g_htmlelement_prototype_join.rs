//! **G_HTMLELEMENT_PROTOTYPE_JOIN — `HTMLElement.prototype` was not the prototype elements use, so
//! `super.getAttribute(...)` did not exist.**
//!
//! Found by the t1406 corpus sweep's throw histogram — `TypeError: super.getAttribute is not a
//! function`, 5 hits — and reproduced in four lines. The custom-elements shim gives the `HTMLElement`
//! CONSTRUCTOR a fresh prototype on purpose (an upgrade grafts members onto the host object, because
//! a reflector's prototype cannot be swapped), so there are **two objects both entitled to the name**:
//!
//! ```text
//!   a <div>'s chain:  instance -> __protoHTMLElement -> Element.prototype -> Node.prototype -> EventTarget
//!                     ...and globalThis.HTMLElement.prototype IS NOT IN IT
//! ```
//!
//! Everything the engine has spent ticks adding — `currentSrc`, `complete`, `naturalWidth`,
//! `checkValidity`, `showModal`, the popover four — went on the first object, while
//! `class X extends HTMLElement` and every `'feature' in HTMLElement.prototype` detection read the
//! second. Headless Chrome 145.0.7632.116 vs us, one fixture:
//!
//! ```text
//!                                          chrome      before
//!   super.getAttribute('data-x')           'hello'     THREW: not a function
//!   super.setAttribute(...)                works       THREW
//!   super.closest('body')                  true        THREW
//!   super.addEventListener + dispatchEvent 1           THREW
//!   typeof HTMLElement.prototype.getAttribute  function undefined
//!   'popover' in HTMLElement.prototype     true        (t1395 hand-mirrored 8 names to get this)
//! ```
//!
//! `super.<method>()` is the ORDINARY way a custom element extends a DOM method — every Lit, Stencil
//! and vanilla component that overrides `getAttribute`/`setAttribute`/`addEventListener` and calls
//! through to the base does exactly this. It throws inside `connectedCallback`, so **the element never
//! upgrades and its whole subtree stays inert.**
//!
//! ⭐⭐⭐ **AND THE FIX IS A JOIN, NOT A MIRROR.** The first attempt copied names across and was wrong
//! twice: a hand-kept list is the t1351 shape (a plural asserting a sample — which is exactly what
//! t1395's eight names were, and they are deleted by this tick), and even a DERIVED copy of
//! `__protoHTMLElement`'s OWN names misses `getAttribute`, which lives two links further up on
//! `Element.prototype`. The constructor's prototype was not missing a LIST; it was missing a CHAIN.
//! One `Object.setPrototypeOf` gives it every member of `__HP`, `Element.prototype`, `Node.prototype`
//! and `EventTarget.prototype` at once — including everything added after that line ever runs.
//!
//! ⚠ **Named non-claim, measured:** `HTMLElement.prototype.getAttribute.call({}, 'x')` throws
//! `TypeError: Illegal invocation` in Chrome and returns quietly here — the native binding does not
//! brand-check its `this`. Pre-existing, orthogonal to the chain, and asserting OUR answer would pin
//! the engine to a bug (t1004), so the arm was written, measured, and removed rather than shipped.
//!
//! ⚠ **Named non-claim:** `'checkValidity' in HTMLElement.prototype` now reads `true` and Chrome says
//! `false` (it is a form-control member). That is the pre-existing consequence of this engine putting
//! form-control members on the shared `__protoHTMLElement` — `'checkValidity' in div` was already
//! `true` before this tick — and the join only makes the constructor's prototype agree with the
//! engine's own instances. Not introduced here, and not fixed here.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body><div id="out">-</div>
<my-el id="m" data-x="hello">child</my-el>
<script>
var r = [];
class MyEl extends HTMLElement {
  connectedCallback() {
    try { r.push('superGet:' + super.getAttribute('data-x')); } catch (e) { r.push('superGet:THREW ' + e.message); }
    try { super.setAttribute('data-y','set'); r.push('superSet:' + this.getAttribute('data-y')); }
    catch (e) { r.push('superSet:THREW ' + e.message); }
    try { r.push('superClosest:' + (super.closest('body') !== null)); } catch (e) { r.push('superClosest:THREW ' + e.message); }
    try { var n = 0; super.addEventListener('x', function () { n++; }); super.dispatchEvent(new Event('x'));
          r.push('superEvents:' + n); } catch (e) { r.push('superEvents:THREW ' + e.message); }
  }
}
customElements.define('my-el', MyEl);
r.push('protoGet:' + (typeof HTMLElement.prototype.getAttribute));
r.push('protoAddEL:' + (typeof HTMLElement.prototype.addEventListener));
r.push('inPopover:' + ('popover' in HTMLElement.prototype));
r.push('inNodeType:' + ('nodeType' in HTMLElement.prototype));
r.push('instOf:' + (document.getElementById('m') instanceof HTMLElement));
r.push('plainInst:' + (document.createElement('div') instanceof HTMLElement));
r.push('chainTerminates:' + (function(){ var o = HTMLElement.prototype, n = 0;
  while (o && n++ < 64) { o = Object.getPrototypeOf(o); } return n < 64; })());
setTimeout(function(){ document.getElementById('out').textContent = r.join(' | '); }, 200);
</script></body></html>"##;

#[test]
fn a_custom_element_can_call_super_dot_getattribute() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ce.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("HTMLELEMENT-PROTOTYPE-JOIN: {got}");

    for claim in [
        // ── the four `super.` calls, every one Chrome-measured on this exact fixture
        "superGet:hello",
        "superSet:set",
        "superClosest:true",
        "superEvents:1",
        // ── and the same objects read directly, which is what feature detection does
        "protoGet:function",
        "protoAddEL:function",
        "inPopover:true",
        // `nodeType` is on Node.prototype — TWO links above `__protoHTMLElement`, so it is the row
        // that proves this is a CHAIN and not a copy of one object's own names.
        "inNodeType:true",
        // the control: joining the chain must not break the brand the whole shim rests on
        "instOf:true",
        // ⭐ THE CYCLE CONTROL. The fix is `Object.setPrototypeOf(HTMLElement.prototype, __HP)`, and
        // if `__HP` ever came to chain THROUGH the constructor's prototype that makes an infinite
        // chain — every property lookup on every element hangs, a Bar 0 from one line. The guard
        // walks first and refuses; this row asserts the chain still ENDS.
        "chainTerminates:true",
    ] {
        assert!(
            got.contains(claim),
            "G_HTMLELEMENT_PROTOTYPE_JOIN: expected `{claim}`\n  got: {got}\n\n  \
             `globalThis.HTMLElement.prototype` must be IN the chain every element uses. It was a \
             disconnected object, so `super.getAttribute(...)` — the ordinary way a custom element \
             calls through to the base DOM method — threw inside `connectedCallback`, the element \
             never upgraded, and its whole subtree stayed inert. Every row is headless-Chrome-measured."
        );
    }
}
