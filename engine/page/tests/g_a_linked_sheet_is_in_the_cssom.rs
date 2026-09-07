//! **G_A_LINKED_SHEET_IS_IN_THE_CSSOM — the CSS loaded, applied, and the object model could not see
//! it.**
//!
//! Chrome-measured on a sheet that demonstrably changes a colour:
//!
//! ```text
//!                                    Chrome                  before          after
//!   document.styleSheets.length      2                       1               2
//!   …their ownerNodes                [STYLE, LINK]           [STYLE]         [STYLE, LINK]
//!   the linked rule's effect         color: rgb(1,2,3)       rgb(1,2,3)      rgb(1,2,3)   ✓
//!   link.sheet                       object                  undefined       object
//! ```
//!
//! ⭐⭐ **THE MIDDLE ROW IS WHY THIS WAS INVISIBLE.** The rules were in the cascade the whole time —
//! the page rendered correctly — and absent only from the *view* of it. Every theme switcher,
//! CSS-in-JS runtime and `sheet.disabled` toggler iterates `document.styleSheets`, and on any page
//! whose CSS is external they saw an empty-ish list and took their no-stylesheets branch.
//!
//! t665 built `<style>.sheet` and recorded this as deliberate scope — *"`<link>.sheet` stays
//! `undefined`… for an applied linked sheet `null` would be a lie."* The text was never missing:
//! `Page::external_css` has held it all along, keyed by resolved URL, and `link.href` is already
//! absolute. Only a way for the prelude to reach it was missing, which `document.__manukSheetText`
//! now is.
//!
//! ## Three things this gate pins that are easy to get subtly wrong
//!
//! ⚠ **ORDER IS DOCUMENT ORDER.** Chrome reports `[STYLE, LINK]` for a `<style>` written above a
//! `<link>`. The getter uses one `querySelectorAll('style, link[rel]')` rather than concatenating two
//! `getElementsByTagName` results, which would group by tag and silently reorder every page.
//!
//! ⚠ **`ownerNode` IS THE `<link>`, NOT THE SHIM.** The sheet is built from a detached `<style>`
//! carrying the fetched text so it goes through the *same* `__makeSheet` as the inline path — two
//! parsers for one grammar is how the two disagree later — and `ownerNode` is then pointed back at
//! the link, which is what a consumer walks back to.
//!
//! ⚠ **THE `rules0` FIELD READS THE INLINE SHEET, NOT THE LINKED ONE, AND THAT IS DELIBERATE.**
//! Chrome throws a `SecurityError` on `cssRules` for a `file://` linked sheet, so a gate that pinned
//! the linked sheet's rule count would be asserting a number **no oracle can confirm**. The linked
//! sheet's presence is proven by `kinds`, and that its rules reached the cascade by `color`.
//!
//! ⚠ **A LINK WITHOUT A SHEET IS `null`, NOT `undefined`.** `HTMLLinkElement.sheet` is
//! `CSSStyleSheet?`, and the standard guard `if (el.sheet === null)` is FALSE against `undefined` —
//! the false-presence trap t663 measured on `<style>`, which would otherwise have been reintroduced
//! one tag over. The `rel=preload` row below is that assertion.
//!
//! Mutations that must turn this red:
//!   1. the getter returns `undefined` for LINK        → `sheets=1`, and the preload row reads undef
//!   2. `getElementsByTagName('style')` instead of the → `sheets=1`
//!      combined `querySelectorAll`
//!   3. `ownerNode` left as the shim                   → kinds reads `[STYLE,STYLE]`
//!   4. the `rel~=stylesheet` filter dropped           → the preload link joins the list

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8>
<style>#t { font-weight: 700 }</style>
<link rel="stylesheet" href="ext.css?a=1&amp;b=2">
<link rel="preload" as="style" href="ext.css?a=1&amp;b=2">
</head><body>
<div id="t">t</div><div id="out">-</div>
<script>
var s = document.styleSheets, kinds = [];
for (var i = 0; i < s.length; i++) { kinds.push(s[i].ownerNode ? s[i].ownerNode.nodeName : '?'); }
var pre = document.querySelectorAll('link')[1];
document.getElementById('out').textContent =
  'sheets=' + s.length + ' kinds=[' + kinds.join(',') + ']' +
  ' rules0=' + (s.length ? s[0].cssRules.length : '-') +
  ' color=' + getComputedStyle(document.getElementById('t')).color +
  ' preload=' + (pre.sheet === null ? 'null' : typeof pre.sheet);
</script></body></html>"##;

#[test]
fn a_linked_stylesheet_is_visible_to_the_cssom() {
    let dir = std::path::Path::new("/tmp/g1479");
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("ext.css"),
        "#t { color: rgb(1,2,3) }\n#t { outline-width: 3px }\n",
    )
    .unwrap();
    let path = dir.join("page.html");
    std::fs::write(&path, HTML).unwrap();

    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(
            HTML,
            &format!("file://{}", path.display()),
            &fonts,
            800.0,
        )
        .await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CSSOM: {got}");

    // ── VACUITY. The linked rule must actually APPLY, or this gate is measuring a view of nothing
    //    and would pass just as well on a page whose stylesheet 404'd.
    assert!(
        got.contains("color=rgb(1, 2, 3)"),
        "VACUOUS: the linked stylesheet did not apply, so its presence in the CSSOM proves nothing \
         — got {got:?}"
    );

    // Chrome headless, every field.
    let want = "sheets=2 kinds=[STYLE,LINK] rules0=1 color=rgb(1, 2, 3) preload=null";
    assert_eq!(
        got, want,
        "\n  a linked stylesheet must be in document.styleSheets, own its <link>, and expose its \
         rules\n  want: {want}\n  got:  {got}"
    );
}
