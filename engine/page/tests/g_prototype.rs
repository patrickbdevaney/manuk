//! **G_PROTOTYPE — DOM methods live on prototypes, and patching a prototype actually takes effect.**
//!
//! For sixty ticks every DOM method was defined as an **own-property of every element**. All 116 of
//! them, on every node, one `JS_DefineProperty` at a time. That is wrong in three separate ways, and
//! this gate holds the line on all three.
//!
//! **1. `Element.prototype.setAttribute` was `undefined`.** Not "subtly different" — *absent*. So was
//! `Node.prototype.appendChild`, and `EventTarget` did not exist at all (`ReferenceError`). A large
//! amount of ordinary web code reaches straight for these: polyfills, feature detection
//! (`'matches' in Element.prototype`), anything doing `.call()` on a borrowed DOM method.
//!
//! **2. Patching a prototype SILENTLY DID NOTHING — and that is the one that matters.**
//!
//! ```js
//! const real = Element.prototype.setAttribute;
//! Element.prototype.setAttribute = function (n, v) { track(n, v); return real.call(this, n, v); };
//! ```
//!
//! This is *the* way the web instruments the DOM: Sentry and every other error tracker, ad-blockers,
//! polyfills, framework internals, React DevTools. The assignment succeeded, nothing threw — and the
//! element's **own** property shadowed the patched prototype, so the wrapper was never called. **The
//! library believes it is installed and it is not.** A loud failure gets fixed; a silent one ships.
//!
//! **3. It was slow, per element.** 116 property definitions and two full JS *compiles* (the identity
//! cache was read and written by `eval`ing a formatted source string) for every single node. Creating
//! 5,000 divs took **124ms**. It takes **2ms** now. Every React, Vue and Angular page pays this on
//! every render.
//!
//! The chain is now real:
//!
//! ```text
//! element → HTMLElement.prototype → Element.prototype → Node.prototype → EventTarget.prototype
//! ```
//!
//! **Stated limit, so nobody has to discover it:** the members are own-properties of `Node.prototype`
//! rather than distributed across the `Node`/`Element`/`HTMLElement` tiers, because this engine's member
//! list does not yet distinguish them — so `Element.prototype.hasOwnProperty('setAttribute')` is `false`
//! where the spec says `true`. Everything that *resolves* through the chain is correct; the ownership
//! tiering is a later tick. Saying so beats pretending.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div>
<script>
  var R = [];

  // ── 1. The interfaces exist, and the methods are ON them.
  R.push('EventTarget:' + typeof EventTarget);
  R.push('etAEL:' + typeof EventTarget.prototype.addEventListener);
  R.push('elSet:' + typeof Element.prototype.setAttribute);
  R.push('elQS:' + typeof Element.prototype.querySelector);
  R.push('nodeAppend:' + typeof Node.prototype.appendChild);

  // ── 2. The chain is the spec's chain.
  var d = document.createElement('div');
  R.push('isEl:' + (d instanceof Element));
  R.push('isNode:' + (d instanceof Node));
  R.push('isET:' + (d instanceof EventTarget));

  // A borrowed method must work on a foreign `this` — `.call()` is how half of jQuery is written.
  Element.prototype.setAttribute.call(d, 'data-borrowed', 'yes');
  R.push('borrowed:' + d.getAttribute('data-borrowed'));

  // ── 3. THE ONE THAT MATTERS. Patch the prototype; the patch must actually run.
  var real = Element.prototype.setAttribute;
  var seen = [];
  Element.prototype.setAttribute = function (n, v) {
    seen.push(n);
    return real.call(this, n, v === 'raw' ? 'INTERCEPTED' : v);
  };
  var e = document.createElement('span');
  e.setAttribute('title', 'raw');            // goes through the patch, or it does not
  Element.prototype.setAttribute = real;     // put it back, like a well-behaved library
  R.push('patched:' + e.getAttribute('title'));
  R.push('sawCall:' + (seen.indexOf('title') !== -1));

  // ── 4. The element carries (almost) no own properties. This is the memory + speed half.
  R.push('ownProps:' + Object.getOwnPropertyNames(document.createElement('div')).length);

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"#;

#[test]
fn dom_methods_live_on_prototypes_and_patching_one_actually_takes_effect() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://proto.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("EventTarget:function", "EventTarget did not exist at all — a bare ReferenceError"),
        ("etAEL:function", "addEventListener must live on EventTarget.prototype, per spec"),
        ("elSet:function", "Element.prototype.setAttribute was `undefined` for sixty ticks"),
        ("elQS:function", "…and so was querySelector"),
        ("nodeAppend:function", "…and appendChild, on Node.prototype"),
        ("isEl:true", "the prototype chain must satisfy instanceof"),
        ("isNode:true", "…at every tier"),
        ("isET:true", "…including EventTarget, the root of the DOM chain"),
        ("borrowed:yes", "a borrowed method must work with .call() on a foreign `this`"),
        (
            "patched:INTERCEPTED",
            "PATCHING THE PROTOTYPE MUST TAKE EFFECT. This is how Sentry, ad-blockers, polyfills, \
             framework internals and React DevTools all hook the DOM. It used to return the RAW value: \
             the assignment succeeded, nothing threw, and the element's own property shadowed the \
             patch, so the wrapper was never called. The library believes it is installed and it is not",
        ),
        ("sawCall:true", "…and the wrapper must actually be entered, not merely defined"),
        (
            "ownProps:1",
            "an element must not carry the whole DOM API as own-properties. 116 of them, per node, \
             was 124ms to create 5,000 divs — the cost every React render pays",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_PROTOTYPE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
