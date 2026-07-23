//! **G_SCROLLBAR_THEME — `getComputedStyle(el).scrollbarWidth` / `.scrollbarColor` reflect the cascade.**
//!
//! `scrollbar-width` (`auto`/`thin`/`none`) and `scrollbar-color` (`auto` | `<thumb> <track>`) are the
//! CSS Scrollbars-1 theming properties (Baseline 2024). Dark-mode sites set them on their scroll
//! containers so a bright OS scrollbar does not sit on a dark UI (`scrollbar-color: #888 #222`), and
//! compact chrome uses `scrollbar-width: thin`. Custom-scrollbar libraries feature-detect the resolved
//! value through the CSSOM (`getComputedStyle(el).scrollbarColor` / `.scrollbarWidth`) to decide whether
//! to draw their own overlay.
//!
//! Before this the properties did not exist in the engine: both are `engine="gecko"` in the crates.io
//! `stylo 0.19` the browser compiles, so the servo build never generates them (no pref can surface them —
//! a vendored `stylo/` tree marking them `layout.unimplemented` is a decoy). Like `-webkit-line-clamp`,
//! they are recovered from `MinimalCascade`: the keyword / thumb+track colour pair is parsed there
//! (`engine/css/src/lib.rs`) and merged into the shipping style in the `stylo_engine` recovery loop, then
//! serialized (plus `getPropertyValue`) in `getComputedStyle`.
//!
//! Proven RED: delete the two `cs.scrollbar_* = m.scrollbar_*` merge lines in `stylo_engine` and every
//! element reads the initial `auto` — the themed values stop coming from the stylesheet.
//!
//! (Scope, stated honestly: this resolves the COMPUTED VALUE the CSSOM reports. Painting a themed
//! scrollbar is a paint concern the engine does not model — the same boundary `user-select` documents.)

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  .thin  { scrollbar-width: thin; }
  #hide  { scrollbar-width: none; }
  #theme { scrollbar-color: #888 #222; }
</style></head><body>
<div id="hide">hidden-bar</div>
<div id="none" style="scrollbar-width: thin; scrollbar-color: rgb(255,0,0) rgb(0,0,255)">themed</div>
<div id="theme">theme-class</div>
<div id="def">default</div>
<div id="out">-</div>
<script>
var r = [];
try {
  var g = function (id) { return getComputedStyle(document.getElementById(id)); };
  r.push('hide-w:' + g('hide').scrollbarWidth);
  r.push('thin-w:' + g('none').scrollbarWidth);
  r.push('def-w:' + g('def').scrollbarWidth);
  r.push('def-c:' + g('def').scrollbarColor);
  r.push('theme-c:' + g('theme').scrollbarColor);
  r.push('inline-c:' + g('none').scrollbarColor);
  // getPropertyValue routes the dash-case names back to the same resolved value.
  r.push('get-w:' + g('hide').getPropertyValue('scrollbar-width'));
  r.push('get-c:' + g('theme').getPropertyValue('scrollbar-color'));
} catch (e) {
  r.push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' | ');
</script></body></html>"##;

#[test]
fn scrollbar_theme_computed_values_reflect_the_cascade() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://scrollbar.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SCROLLBAR-THEME RESULT: {got}");

    for claim in [
        "hide-w:none",                                // `scrollbar-width: none` from #hide
        "thin-w:thin",                                // inline `scrollbar-width: thin`
        "def-w:auto",                                 // initial value, unset
        "def-c:auto",                                 // `scrollbar-color` initial is `auto`
        "theme-c:rgb(136, 136, 136) rgb(34, 34, 34)", // #888 thumb, #222 track (class rule)
        "inline-c:rgb(255, 0, 0) rgb(0, 0, 255)",     // inline thumb red, track blue
        "get-w:none",                                 // getPropertyValue('scrollbar-width')
        "get-c:rgb(136, 136, 136) rgb(34, 34, 34)",   // getPropertyValue('scrollbar-color')
    ] {
        assert!(
            got.contains(claim),
            "G_SCROLLBAR_THEME: expected `{claim}`\n  got: {got}\n\n  \
             `scrollbar-width`/`scrollbar-color` must cascade and reach `getComputedStyle`. If every \
             element reads `auto`, the `MinimalCascade` parse or the `stylo_engine` merge of \
             `scrollbar_*` was lost — the value is no longer coming from the stylesheet."
        );
    }
}
