//! **G_GET_PROPERTY_VALUE — `getComputedStyle(el).getPropertyValue(p)` returns a STRING. Always.**
//!
//! The tick-401 re-keyed oracle's second named console error, on okta.com:
//! `TypeError: can't access property "trim", getComputedStyle(...).getPropertyValue(...) is
//! undefined`. The CSSOM contract is total: getPropertyValue returns the serialized value for a
//! supported property and the EMPTY STRING for everything else — an unknown property, a property
//! the snapshot doesn't carry, a custom property that isn't set. It never returns `undefined`,
//! because half the web writes `getComputedStyle(el).getPropertyValue(x).trim()` in one
//! expression and `undefined.trim()` kills the caller's whole (usually async) frame.
//!
//! Two defects, one organ, both asserted here:
//!   1. the snapshot object's accessor did `return this[m[p]||p]` — `undefined` for anything
//!      outside its hand-written kebab→camel map or its property list;
//!   2. the no-style fallback was a bare `({})` — no accessor AT ALL, so the same page code
//!      throws `getPropertyValue is not a function` on any element getComputedStyle cannot style.
//!
//! Proven RED: with the coercion absent, `unknown-empty`/`trim-safe` read `undefined` and the
//! gate names the okta line.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="t" style="margin-top: 7px; color: rgb(1, 2, 3)">x</div>
<div id="out">-</div>
<script>
var r = [];
try {
  var cs = getComputedStyle(document.getElementById('t'));
  // Supported reads still work — via the map and via a bare name.
  r.push('known-mapped:' + cs.getPropertyValue('margin-top'));
  r.push('known-bare:' + cs.getPropertyValue('color'));
  // The contract: a string for EVERY input.
  r.push('unknown-type:' + typeof cs.getPropertyValue('transition-duration'));
  r.push('unknown-empty:' + (cs.getPropertyValue('not-a-real-property') === ''));
  r.push('custom-empty:' + (cs.getPropertyValue('--unset-custom') === ''));
  // The okta pattern, verbatim: .getPropertyValue(x).trim() in one expression.
  r.push('trim-safe:' + (cs.getPropertyValue('backdrop-filter').trim() === ''));
} catch (e) {
  r.push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn get_property_value_is_total_over_its_domain() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cssom.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("GET-PROPERTY-VALUE RESULT: {got}");

    for claim in [
        "known-mapped:7px",    // the map still serves supported kebab reads
        "known-bare:rgb",      // bare property names still resolve (rgb/rgba serialization)
        "unknown-type:string", // NEVER undefined —
        "unknown-empty:true",  // — the unknown answer is '' exactly,
        "custom-empty:true",   // custom properties included,
        "trim-safe:true",      // and .getPropertyValue(x).trim() cannot throw (the okta line)
    ] {
        assert!(
            got.contains(claim),
            "G_GET_PROPERTY_VALUE: expected `{claim}`\n  got: {got}\n\n  \
             CSSOM getPropertyValue is TOTAL: the serialized value for supported properties, the \
             empty string for everything else. `undefined` kills every `.getPropertyValue(x).trim()` \
             call site on the web — that is the exact okta.com failure this gate reproduces."
        );
    }
}
