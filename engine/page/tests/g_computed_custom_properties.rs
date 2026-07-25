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
    /* MORE THAN EIGHT properties here on purpose: Stylo's `should_expand_chain` only switches the
       custom-property map from copy-to-child to a PARENT-CHAIN above a threshold of 8. Below it a
       redefining descendant just copies, the chain never forms, and the duplicate this gate exists
       to catch cannot occur. A fixture with two tokens tests the easy shape only. */
    :root { --brand: #ff0000; --gap: 8px; --t1: 1px; --t2: 2px; --t3: 3px; --t4: 4px;
            --t5: 5px; --t6: 6px; --t7: 7px; --t8: 8px; --t9: 9px; --t10: 10px; }
    #a { --local: 42px; color: var(--brand); }
    /* SHADOWING: a descendant redefines an inherited token. Stylo stores this copy-on-write, as a
       child map whose PARENT still holds the old entry — so the overridden name is reachable twice
       through the chain if the walk is not bounded by `len()`. */
    #shadow { --brand: #00ff00; }
  </style></head><body><div id="a"><span id="child">x</span></div>
  <div id="shadow"><span id="deep">y</span></div>
  <div id="out">-</div><script>
    var r = [];
    var a = document.getElementById('a'), c = document.getElementById('child');
    var sh = document.getElementById('shadow'), dp = document.getElementById('deep');
    r.push('shadow:' + getComputedStyle(sh).getPropertyValue('--brand'));
    r.push('deep:' + getComputedStyle(dp).getPropertyValue('--brand'));
    // Every custom property must be enumerated EXACTLY ONCE. `getPropertyValue` cannot see a
    // duplicate (the lookup object is keyed by name, so a repeat silently collapses); the
    // enumeration can, and it is the only place the copy-on-write parent chain leaking a shadowed
    // entry back into the list would ever show up.
    var cs = getComputedStyle(sh), seen = 0;
    for (var i = 0; i < cs.length; i++) { if (cs.item(i) === '--brand') seen++; }
    r.push('brandcount:' + seen);
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
        // The override wins on the element that declares it AND on its descendants...
        "shadow:#00ff00",
        "deep:#00ff00",
        // ...and the shadowed name is listed ONCE, not once per level of the property chain.
        "brandcount:1",
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
