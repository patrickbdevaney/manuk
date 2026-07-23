//! **G_COMPUTED_CUSTOM_PROPERTIES — `getComputedStyle(el).getPropertyValue('--x')` returns the
//! cascaded custom-property value.**
//!
//! CSS custom properties (`--brand`, `--gap`, design tokens) are how the modern web themes itself, and
//! reading their COMPUTED value back is the core of every runtime that does it: a chart library that
//! pulls `--color-primary` off `:root`, a component that reads `--gap` to size a canvas, every CSS-in-JS
//! and design-system runtime. `getComputedStyle(el).getPropertyValue('--x')` returned `''` for all of
//! them — the computed-style object exposed only the fixed longhand map, and custom properties (which
//! Stylo resolves and inherits) were dropped on the floor, so the theme read came back empty and the
//! component fell back to a hardcoded default or drew nothing.
//!
//! The claims check the returned string, each a way the old "custom props absent" behaviour goes RED:
//!
//!   * A property **declared on the element** reads back its value.
//!   * A property declared on **`:root`** is readable on a deep descendant (custom properties INHERIT).
//!   * A **missing** `--x` returns `''` (the total-function contract, not `undefined`).
//!   * A **normal** longhand (`color`) still resolves — the custom-property short-circuit did not break
//!     the existing path.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head><style>
    :root { --brand: #ff0000; --gap: 8px; }
    #a { --local: 42px; color: var(--brand); }
  </style></head><body><div id="a"><span id="child">x</span></div><div id="out">-</div><script>
    var r = [];
    var a = document.getElementById('a'), c = document.getElementById('child');
    r.push('local:' + getComputedStyle(a).getPropertyValue('--local'));
    r.push('root:' + getComputedStyle(a).getPropertyValue('--brand'));
    r.push('inherit:' + getComputedStyle(c).getPropertyValue('--brand'));
    r.push('gap:' + getComputedStyle(a).getPropertyValue('--gap'));
    r.push('missing:[' + getComputedStyle(a).getPropertyValue('--nope') + ']');
    r.push('color:' + (getComputedStyle(a).getPropertyValue('color').indexOf('rgb') >= 0 ? 'rgb' : 'no'));
    document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn getcomputedstyle_returns_custom_property_values() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://computed-custom-props.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "local:42px",      // a property declared on the element
        "root:#ff0000",    // a :root property, read on an element that inherits it
        "inherit:#ff0000", // custom properties INHERIT — readable on a deep descendant
        "gap:8px",
        "missing:[]", // a missing --x is '' (total function), not undefined
        "color:rgb",  // a normal longhand still resolves (no regression to the fixed map)
    ] {
        assert!(
            got.contains(claim),
            "G_COMPUTED_CUSTOM_PROPERTIES: expected {claim} in {got:?}\n  \
             getComputedStyle(el).getPropertyValue('--x') must return the cascaded custom-property \
             value — returning '' drops every design token, and theming/chart/CSS-in-JS runtimes read \
             their tokens exactly this way."
        );
    }
}
