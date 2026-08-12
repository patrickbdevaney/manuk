//! **G_COMPUTED_DASHED_ATTRIBUTE — a computed style answers to its own CSS property name.**
//!
//! CSSOM defines THREE IDL attributes per supported CSS property on a `CSSStyleDeclaration`: the
//! *camel-cased* attribute (`marginLeft`), the *webkit-cased* attribute (`webkitUserSelect`), and —
//! for every property whose name contains a dash — the **dashed attribute**, which is the CSS
//! property name itself (`style['margin-left']`). We shipped only the first: `getComputedStyle(el)`
//! is a snapshot object with camelCase slots, so `cs['margin-left']` was `undefined` and
//! `'margin-left' in cs` was **false** — for `margin-left`, a property this engine has laid out
//! correctly for a thousand ticks.
//!
//! That is not a nicety. It is the FIRST LINE of the CSS-WG's own test helper:
//!
//! ```js
//! // wpt/css/support/computed-testcommon.js
//! assert_true(property in getComputedStyle(target),
//!             property + " doesn't seem to be supported in the computed style");
//! ```
//!
//! `test_computed_value` is how the CSS test corpus asks *every* computed-value question, and it
//! passes the DASHED name. So a property we implement, cascade and paint correctly reported
//! **"doesn't seem to be supported"** and every subtest under it failed before reading a value.
//!
//! The negative rows are what make this a rule about the DASHED ATTRIBUTE rather than "put more
//! keys on the object":
//!
//!   * a property we genuinely do not have (`view-transition-name`) must still answer `false` —
//!     `in` is a question about THIS engine, and a blanket `true` would be the t1177 lie in a new
//!     place;
//!   * a **custom property** (`--brand`) is NOT a dashed attribute — Chrome answers `false` and
//!     routes it through `getPropertyValue` only;
//!   * `length` / `item(i)` enumerate the property list ONCE; an alias must not double-count, or
//!     every style-copy loop writes each declaration twice.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head><style>#d{--brand:7px}</style></head><body>
    <div id="d" style="margin-left:11px;letter-spacing:3px;position:relative;z-index:5;object-fit:cover;color:red">hi</div>
    <div id="out">-</div><script>
    var r = [];
    var d = document.getElementById('d');
    var cs = getComputedStyle(d);

    // ── NEGATIVE ROWS FIRST — what must stay false after the fix. ──────────────────────────
    r.push('neg-absent:' + ('view-transition-name' in cs));   // we do not implement it
    r.push('neg-bogus:' + ('not-a-property' in cs));          // not a CSS property at all
    r.push('neg-custom-in:' + ('--brand' in cs));             // custom props are NOT dashed attrs
    r.push('neg-custom-get:' + (cs['--brand'] === undefined ? 'undefined' : cs['--brand']));

    // ── CONTROL ROWS — the surface that already worked and must not move. ──────────────────
    r.push('ctl-camel:' + cs.marginLeft);
    r.push('ctl-gpv:' + cs.getPropertyValue('margin-left'));
    r.push('ctl-camel-in:' + ('marginLeft' in cs));
    r.push('ctl-item0:' + cs.item(0));
    r.push('ctl-nodash-in:' + ('color' in cs));               // a dashless property needs no alias

    // ── THE SUBJECT — `property in cs` and `cs[property]` for the DASHED name. ─────────────
    r.push('in-margin-left:' + ('margin-left' in cs));
    r.push('in-z-index:' + ('z-index' in cs));
    r.push('in-letter-spacing:' + ('letter-spacing' in cs));
    r.push('in-background-image:' + ('background-image' in cs));
    r.push('in-object-fit:' + ('object-fit' in cs));
    r.push('in-background-color:' + ('background-color' in cs));
    r.push('in-webkit-user-select:' + ('-webkit-user-select' in cs));
    r.push('get-margin-left:' + cs['margin-left']);
    r.push('get-letter-spacing:' + cs['letter-spacing']);
    r.push('get-z-index:' + cs['z-index']);
    r.push('agree:' + (cs['margin-left'] === cs.marginLeft && cs['object-fit'] === cs.objectFit));

    // ── ENUMERATION MUST NOT DOUBLE-COUNT — the alias is an attribute, not a new declaration.
    var seen = 0;
    for (var i = 0; i < cs.length; i++) { if (cs.item(i) === 'margin-left') seen++; }
    r.push('once:' + seen);
    r.push('len-sane:' + (cs.length > 40 && cs.length < 200));

    // ── THE HELPER'S OWN SHAPE — set through the dashed key, read back through it. ─────────
    d.style['margin-top'] = '13px';
    r.push('roundtrip:' + getComputedStyle(d)['margin-top']);

    document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn computed_style_answers_to_the_dashed_css_property_name() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://dashed.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        // NEGATIVE — an absent property, a non-property and a custom property all stay false.
        "neg-absent:false",
        "neg-bogus:false",
        "neg-custom-in:false",
        "neg-custom-get:undefined",
        // CONTROL — the camelCase surface is untouched.
        "ctl-camel:11px",
        "ctl-gpv:11px",
        "ctl-camel-in:true",
        "ctl-item0:color",
        "ctl-nodash-in:true",
        // SUBJECT — the dashed attribute exists and carries the same value.
        "in-margin-left:true",
        "in-z-index:true",
        "in-letter-spacing:true",
        "in-background-image:true",
        "in-object-fit:true",
        "in-background-color:true",
        "in-webkit-user-select:true",
        "get-margin-left:11px",
        "get-letter-spacing:3px",
        "get-z-index:5",
        "agree:true",
        // The alias is an ATTRIBUTE, not an extra declaration.
        "once:1",
        "len-sane:true",
        "roundtrip:13px",
    ] {
        assert!(
            got.contains(claim),
            "G_COMPUTED_DASHED_ATTRIBUTE: missing `{claim}` in:\n  {got}"
        );
    }
}
