//! **G_OVERFLOW_CSSOM — `overflowX`/`overflowY` are readable, and the scroll parent is findable.**
//!
//! `getComputedStyle(el).overflowY` returned **`undefined`**: only the single combined `overflow`
//! value layout uses for its clip rect was ever serialized. That one value cannot answer the
//! question anyway — `overflow-x: hidden; overflow-y: scroll` collapses to one keyword, and which
//! axis actually scrolls is unrecoverable from it.
//!
//! **Why this is a rendering bug and not a trivia bug.** Finding the scroll container is done by
//! walking up from an element asking each ancestor whether it scrolls. It is how a dropdown decides
//! what to position against, how a modal decides what to lock, how a virtualised list decides what
//! to listen to, and how "scroll this into view" picks its container. With `overflowY` undefined the
//! walk matches nothing, falls through to the document every time, and the popup anchors to the
//! wrong box. The last assertion here is that walk, run for real.
//!
//! The computed values themselves were already right (stylo applies CSS Overflow §3 — a `visible`
//! paired with a non-`visible` computes to `auto`). Only the CSSOM exposure was missing, so this
//! gate is about what a script can *read*.

use manuk_text::FontContext;

fn read(decl: &str) -> String {
    let html = format!(
        r#"<!doctype html><body><div id="t" style="{decl}"></div><div id="out">-</div><script>
var cs = getComputedStyle(document.getElementById('t'));
document.getElementById('out').textContent = [
  cs.overflowX, cs.overflowY, cs.overflow,
  cs.getPropertyValue('overflow-x'), cs.getPropertyValue('overflow-y')
].join('|');
</script>"#
    );
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(&html, "https://gov.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    page.dom().text_content(out)
}

#[test]
fn the_overflow_axes_are_readable_from_script() {
    // Each expectation is `overflowX|overflowY|overflow|getPropertyValue(-x)|getPropertyValue(-y)`,
    // so the camelCase properties and the kebab-case accessor must agree — a script may use either.
    for (decl, want, why) in [
        (
            "",
            "visible|visible|visible|visible|visible",
            "THE BUG: the per-axis values were never serialized, so BOTH read `undefined`. An empty \
             or undefined `overflowY` is what makes every scroll-parent walk on the page fail",
        ),
        (
            "overflow:hidden",
            "hidden|hidden|hidden|hidden|hidden",
            "both axes take the shorthand, and it still serializes as ONE value when they agree",
        ),
        (
            "overflow:auto",
            "auto|auto|auto|auto|auto",
            "the ordinary scroll-container declaration",
        ),
        (
            "overflow:clip",
            "clip|clip|clip|clip|clip",
            "`clip` is a distinct value and must not be flattened into `hidden`",
        ),
        (
            "overflow:hidden scroll",
            "hidden|scroll|hidden scroll|hidden|scroll",
            "THE SHORTHAND SERIALIZES TWO VALUES when the axes differ. Collapsing this to `hidden` \
             loses the scrolling axis in the property most code reads first — and the single \
             combined value layout keeps for its clip rect can never recover it",
        ),
        (
            "overflow-x:hidden",
            "hidden|auto|hidden auto|hidden|auto",
            "CSS Overflow 3: a `visible` paired with a non-`visible` computes to `auto`, so setting \
             ONE axis silently changes the other. A script reading back the axis it never set must \
             see `auto`, not `visible`",
        ),
        (
            "overflow-y:scroll",
            "auto|scroll|auto scroll|auto|scroll",
            "the same rule with the axes swapped",
        ),
    ] {
        let got = read(decl);
        assert_eq!(
            got, want,
            "G_OVERFLOW_CSSOM: `{decl}` should read `{want}` — {why}.\n  got: {got}"
        );
    }

    // ---- and the reason the property matters, asserted as the thing it is used for. ----
    //
    // (One `#[test]` per gate binary, as every sibling gate here does: each `Page::load` stands up a
    // JS context, and a second one in the same process is not survivable.)
    const HTML: &str = r##"<!doctype html><body style="margin:0">
<div id="outer" style="overflow:auto; height:100px">
  <div id="plain">
    <div id="mid" style="overflow-y:scroll; height:50px">
      <div id="leaf">x</div>
    </div>
  </div>
</div>
<div id="out">-</div>
<script>
  // The canonical scroll-parent walk, as every dropdown/modal/virtual-list library writes it.
  function scrollParent(el) {
    for (var p = el.parentElement; p; p = p.parentElement) {
      var o = getComputedStyle(p);
      if (/auto|scroll|overlay/.test(o.overflowY + ' ' + o.overflowX)) return p.id;
    }
    return 'DOCUMENT';
  }
  document.getElementById('out').textContent =
    scrollParent(document.getElementById('leaf')) + ',' +
    scrollParent(document.getElementById('mid'));
</script></body>"##;

    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://gov.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    assert_eq!(
        got, "mid,outer",
        "G_OVERFLOW_CSSOM: the scroll-parent walk must find the NEAREST scrolling ancestor — `mid` \
         for the leaf (skipping the non-scrolling `plain` in between) and `outer` for `mid`. \
         `DOCUMENT,DOCUMENT` is the pre-fix answer: with `overflowY` undefined the test matches \
         nothing, every popup anchors to the viewport instead of its container, and nothing about \
         the failure is visible in the DOM.\n  got: {got}"
    );
}
