//! **G_CSSOM_ENUMERATION — `CSSStyleDeclaration` is array-like and carries `!important`.**
//!
//! A style declaration (inline `el.style` and computed `getComputedStyle(el)`) is spec'd as an
//! ARRAY-LIKE object: `.length`, `.item(i)` → the property NAME at that index, and the indexed
//! getter `style[i]`. It also separates a value from its PRIORITY: `getPropertyValue` returns the
//! value alone, `getPropertyPriority` returns `'important'`, and `setProperty(k, v, 'important')`
//! sets the flag. These were absent — `style.item` / `style.getPropertyPriority` were `undefined`,
//! computed style had no `.length` at all, and `setProperty`'s third argument was silently dropped —
//! so a library enumerating a declaration (`for (i=0;i<s.length;i++) s.item(i)`, the shape every
//! style-copy / CSS-in-JS serializer uses) threw `s.item is not a function`, and any code that set or
//! read an `!important` from JS got the wrong answer.
//!
//! Each claim is a way the missing surface goes RED:
//!
//!   * inline `style.item(0)` / `style[0]` → the first property NAME; `.length` counts them.
//!   * `setProperty(k, v, 'important')` round-trips: `getPropertyValue` = value, `getPropertyPriority`
//!     = 'important', a camelCase read strips the flag, and `cssText` keeps it.
//!   * computed style is array-like too (`.length` > 0, `.item(0)` a dash-case name) and its
//!     `getPropertyPriority` exists (always '' — a computed value never carries a priority).

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head><style>#d{--brand:7px}</style></head><body>
    <div id="d" style="color:red;font-size:12px;margin:5px 10px">hi</div>
    <div id="out">-</div><script>
    var r = [];
    var d = document.getElementById('d');
    var s = d.style;
    r.push('item0:' + s.item(0));                       // first inline property name
    r.push('idx1:' + s[1]);                             // indexed getter → property name
    r.push('len:' + s.length);                          // three declarations
    r.push('oob:' + (s.item(9) === '' ? 'empty' : s.item(9)));
    r.push('pri0:' + (s.getPropertyPriority('color') === '' ? 'none' : s.getPropertyPriority('color')));
    s.setProperty('color', 'blue', 'important');
    r.push('val:' + s.getPropertyValue('color'));       // value WITHOUT !important
    r.push('pri1:' + s.getPropertyPriority('color'));   // now 'important'
    r.push('camel:' + s.color);                         // camelCase read strips the flag
    r.push('css:' + (s.cssText.indexOf('color: blue !important') >= 0 ? 'kept' : s.cssText));
    var cs = getComputedStyle(d);
    r.push('clen:' + (cs.length > 40 ? 'many' : cs.length));
    r.push('citem0:' + cs.item(0));                     // computed first name (dash-case)
    r.push('cpri:' + (typeof cs.getPropertyPriority === 'function' ? cs.getPropertyPriority('color') === '' ? 'fn-empty' : 'fn' : 'MISSING'));
    var custom = false;
    for (var i = 0; i < cs.length; i++) { if (cs.item(i) === '--brand') custom = true; }
    r.push('custom:' + custom);                         // custom props enumerate too
    document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn cssom_declaration_is_array_like_and_carries_priority() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cssom-enum.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "item0:color",    // inline .item(0) → the first declared property name
        "idx1:font-size", // the indexed getter returns the property NAME
        "len:3",          // three declarations
        "oob:empty",      // out-of-range .item(i) → '' (CSSOM contract, not null)
        "pri0:none",      // no !important initially
        "val:blue",       // getPropertyValue returns the value alone
        "pri1:important", // setProperty's third arg set the priority
        "camel:blue",     // a camelCase read strips the priority
        "css:kept",       // cssText keeps the raw `!important`
        "clen:many",      // computed style is array-like: .length > 40
        "citem0:color",   // computed .item(0) → a dash-case name
        "cpri:fn-empty",  // getPropertyPriority exists on computed (always '')
        "custom:true",    // custom properties enumerate through .item(i)
    ] {
        assert!(
            got.contains(claim),
            "G_CSSOM_ENUMERATION: expected {claim} in {got:?}\n  \
             CSSStyleDeclaration must be array-like (.length/.item/indexed) and separate value from \
             !important priority — their absence throws `item is not a function` and mis-reads priority."
        );
    }
}
