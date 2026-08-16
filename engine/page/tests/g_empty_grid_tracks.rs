//! **G_EMPTY_GRID_TRACKS — a grid that declares tracks is not a zero box just because it has no
//! items.**
//!
//! `grid-template-rows: repeat(3, 100px)` on a childless grid is **300px tall** in every browser: the
//! template reserves the space, the items do not create it. `Ctx::layout_flex_or_grid` opened with
//! `if block_kids.is_empty() { return (Block(vec![]), 0.0) }`, so an empty grid **never reached
//! taffy** — and that one short-circuit produced three symptoms at once:
//!
//! 1. **height 0** where the template says 300;
//! 2. no used track sizes, because `set_detailed_grid_info` only fires during a real layout;
//! 3. and therefore `getComputedStyle().gridTemplateColumns` falling back to the *specified* list —
//!    `"1fr 1fr 1fr 1fr"` where Chrome answers `"200px 200px 200px 200px"`.
//!
//! ⭐ **The one that matters on a real page is the first.** The skeleton/placeholder grid a page
//! renders before its data arrives, the empty-state panel, and any grid whose rows are reserved by
//! the template rather than by content all collapsed to nothing — and the layout jumped when the
//! items landed. This was found while chasing symptom 3; **only the height probe separated a layout
//! bug from a serialization one.**
//!
//! ⚠ **The short-circuit is KEPT for everything else.** An empty *flex* container has no items to
//! size from, and an empty grid with **no explicit template** has no tracks to reserve; both are
//! genuinely zero, and running taffy for every childless container would pay for nothing on every
//! page. The `flexctl:`/`plainctl:` rows below assert that those stay zero — but see the second
//! mutation note at the end: they cannot, and do not, prove the exception is narrow.
//!
//! **To watch it go RED:** delete the `&& !empty_grid_with_tracks` from the short-circuit — `rows:`
//! and `gap:` return to `h=0` and `fr:` to the specified `1fr 1fr 1fr 1fr`.
//!
//! ⚠⚠⚠ **AND THE SECOND MUTATION STAYED GREEN, WHICH IS RECORDED HERE RATHER THAN QUIETLY DROPPED.**
//! Widening the exception to EVERY empty container (`if false && block_kids.is_empty() …`) does not
//! fail a single row: running taffy on an empty flex container, or on a grid with no template, still
//! produces height 0. So the `flexctl:`/`plainctl:` rows guard **correctness** — they would catch a
//! fix that made an empty container non-zero — but they do **not** prove the exception is narrow.
//! The narrowing is a COST decision (do not run taffy for every childless container on every page),
//! and cost is not what this gate measures. Claiming it did would be a green that cannot go red.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 .g { display: grid; width: 800px; }
 #rows { grid-template-rows: repeat(3, 100px); }
 #gap  { grid-template-rows: 100px 100px; row-gap: 20px; }
 #fr   { grid-template-columns: repeat(4, 1fr); }
 #flexctl  { display: flex; }
 #plainctl { }
</style></head><body style="margin:0">
<div class="g" id="rows"></div>
<div class="g" id="gap"></div>
<div class="g" id="fr"></div>
<div id="flexctl"></div>
<div class="g" id="plainctl"></div>
<div id="out">-</div>
<script>
  var R = [];
  ['rows', 'gap', 'fr', 'flexctl', 'plainctl'].forEach(function (k) {
    var e = document.getElementById(k);
    R.push(k + ':h=' + Math.round(e.getBoundingClientRect().height));
  });
  R.push('frcols:<' + getComputedStyle(document.getElementById('fr')).gridTemplateColumns + '>');
  document.getElementById('out').textContent = R.join('  ');
</script></body></html>"##;

// ⚠ ONE `#[test]` fn per JS gate binary — two live `PageContext`s on two threads fault SpiderMonkey.
#[test]
fn an_empty_grid_is_sized_by_its_template() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://emptygrid.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "rows:h=300",
            "THE GATE. Three 100px rows declared, no items: the template reserves 300px. This read 0 \
             — the skeleton-grid collapse, and the reason the page jumps when its data arrives",
        ),
        (
            "gap:h=220",
            "the gaps are part of what the template reserves: 100 + 20 + 100. A fix that summed only \
             the track sizes passes `rows:` and fails here",
        ),
        (
            "fr:h=0",
            "⚠ THE ROW THAT KEEPS THE FIX HONEST: reserving space is the ROW template's job. A grid \
             that declares only COLUMNS still has no rows and is still zero-height, so this must NOT \
             become 800 or any other number just because taffy now runs",
        ),
        (
            "frcols:<200px 200px 200px 200px>",
            "...and the same element's USED track sizes are now published, because taffy ran. It read \
             `1fr 1fr 1fr 1fr` — the specified list, a wrong answer of the right type that a grid \
             library parsing px out of gets NaN from",
        ),
        (
            "flexctl:h=0",
            "CONTROL — an empty FLEX container has no items to size from and is genuinely zero. The \
             short-circuit is kept for it; a fix that ran taffy for every childless container would \
             pay for nothing on every page",
        ),
        (
            "plainctl:h=0",
            "CONTROL — an empty grid with NO explicit template has no tracks to reserve and is also \
             genuinely zero. Together with `flexctl:` this pins the exception to exactly the case \
             where the template itself carries the size",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_EMPTY_GRID_TRACKS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
