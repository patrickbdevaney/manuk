//! **G_GRID_COMPUTED_STYLE — the page laid out as a grid and `getComputedStyle(el).gridAutoFlow` was
//! `undefined`.**
//!
//! Not `""`, not the initial value — **absent**. `'gridTemplateAreas' in getComputedStyle(el)` was
//! `false`, and `getPropertyValue('grid-auto-rows')` returned the empty string, for an element the
//! very same `ComputedStyle` had just laid out as a grid container. The cascade has parsed
//! `grid_auto_flow`, `grid_auto_rows`, `grid_auto_columns`, `grid_template_areas`, `grid_column` and
//! `grid_row` into typed fields since grid landed, and `engine/layout/src/taffy_tree.rs` consumes
//! every one of them. Only the CSSOM object declined to publish them.
//!
//! ⚠⚠⚠ **That is invariant I3 failing on the property family a grid layout is MADE OF.** *"The
//! semantic model is never allowed to rot or lag the renderer"* — and an agent, a layout debugger or
//! a CSS-in-JS runtime asking a grid container how it flows got nothing back. It is the same shape as
//! the t901 sweep batch and as `getComputedStyle(el).transform`, which was applied for sixty ticks
//! before the number reached JS: **not a wrong value, an absent one**, which is the harder half to
//! notice because the page looks right.
//!
//! Measured on `css/css-grid`: **7484 → 7687 subtests on comparable solo runs (+203)**, and the
//! failing assertions it clears say so by name — `assert_true: grid-auto-rows doesn't seem to be
//! supported in the computed style` and `assert_in_array: gridTemplateAreas value undefined not in
//! array ["none"]`.
//!
//! ⚠⚠ **WHAT IS DELIBERATELY STILL ABSENT, AND WHY THAT IS THE POINT OF THIS GATE'S LAST
//! ASSERTION.** `grid-template-columns` / `grid-template-rows` are **not** published, and assertion
//! (5) pins that. Their resolved value is not the computed value: CSSOM §5.1 makes them one of the
//! few properties resolving to the **USED** value, so Chrome answers a grid container with its
//! laid-out track sizes in px — `repeat(3, 1fr)` on a 900px grid reads back
//! `"300px 300px 300px"`. This engine holds the SPECIFIED tracks and nothing else, so publishing
//! them from the cascade would answer `"repeat(3, 1fr)"`: a **wrong answer of the right type**, which
//! every caller then does arithmetic on. `undefined` at least tells the truth, and t608's rule
//! stands — *a name is defined IFF the thing it names exists.*
//!
//! ⚠ The used sizes are recoverable and the path is short, which is why that is a named next tick
//! rather than a limitation: taffy computes them and offers them through
//! `LayoutGridContainer::set_detailed_grid_info`, a trait method whose **default body is the no-op we
//! currently inherit**, under a `detailed_layout_info` feature taffy 0.12 already enables by default.
//! The hazard to design around first is t1120's rather than taffy's — `solve_subtree` also runs
//! during intrinsic measurement, so a side-table keyed by node id gets written by a probe whose
//! outputs are contractually discarded, which is exactly how `pre_transform_rect` was poisoned.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 #g { display: grid;
      grid-template-areas: "head head" "nav  main" "foot foot";
      grid-template-columns: 100px 200px;
      grid-auto-rows: 30px;
      grid-auto-columns: minmax(100px, auto);
      grid-auto-flow: column dense; }
 #i { grid-column: 2 / 4; grid-row: span 2; }
 #j { grid-column: 1; }
</style></head><body>
 <div id="g"><div id="i"></div><div id="j"></div></div>
 <div id="p"></div>
 <div id="out">-</div>
 <script>
   window.addEventListener('load', function () {
     var $ = function (x) { return getComputedStyle(document.getElementById(x)); };
     var g = $('g'), p = $('p'), i = $('i'), j = $('j');
     var parts = [];

     // (1) THE CONTAINER PROPERTIES, as declared. Every one of these is a value the cascade already
     // held and layout already consumed.
     parts.push('flow=' + g.gridAutoFlow);
     parts.push('auto=' + g.gridAutoRows + '|' + g.gridAutoColumns);
     parts.push('areas=' + g.gridTemplateAreas);

     // (2) THE INITIAL VALUES, on an element that is not a grid at all. A property that only answers
     // when it was declared is a property that still cannot be READ — the caller cannot tell "not
     // set" from "not supported", which is the whole failure being fixed.
     parts.push('init=' + p.gridAutoFlow + '|' + p.gridAutoRows + '|' + p.gridAutoColumns +
                '|' + p.gridTemplateAreas);

     // (3) THE ITEM HALF, and the shorthand's OMISSION rule: CSSOM drops a trailing component equal
     // to its initial value, so `grid-column: 1` is `"1"` and not `"1 / auto"`.
     parts.push('item=' + i.gridColumn + '|' + i.gridRow + '|' + j.gridColumn + '|' + j.gridRow);

     // (4) BOTH SPELLINGS. The camelCase attribute and `getPropertyValue` are two different code
     // paths in this object, and shipping one without the other is half a property.
     parts.push('kebab=' + g.getPropertyValue('grid-auto-flow') + '|' +
                g.getPropertyValue('grid-auto-rows') + '|' +
                g.getPropertyValue('grid-template-areas'));

     // (5) …AND THE TWO THAT MUST STAY ABSENT until they can be answered with USED track sizes.
     parts.push('withheld=' + (g.gridTemplateColumns === undefined) + '/' +
                              (g.gridTemplateRows === undefined));

     document.getElementById('out').textContent = parts.join(' ');
   });
 </script>
</body></html>"##;

fn out(page: &manuk_page::Page) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn the_grid_properties_the_cascade_holds_are_published_by_get_computed_style() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://grid.test/",
        &fonts,
        800.0,
    ));
    let got = out(&page);
    println!("GRID-COMPUTED-STYLE {got}");
    let has = |s: &str| got.contains(s);

    // (1) RED: drop the three container entries from the object literal → `flow=undefined`, which is
    // what shipped while the same style laid the page out as a grid.
    assert!(
        has("flow=column dense") && has("auto=30px|minmax(100px, auto)"),
        "the grid container properties the cascade already holds must be published — got {got:?}"
    );
    assert!(
        has(r#"areas="head head" "nav main" "foot foot""#),
        "`grid-template-areas` is stored as resolved line RECTS and must be reconstructed into the \
         spec's normalised quoted-row form (one space between cells, `.` for a null cell) — got {got:?}"
    );

    // (2) RED: publish only when declared → `init=undefined|…`. A property that answers only when it
    // was set leaves the caller unable to distinguish "not set" from "not supported".
    assert!(
        has("init=row|auto|auto|none"),
        "the INITIAL values must be published on an element that is not a grid — `row`, `auto`, \
         `auto`, `none` — got {got:?}"
    );

    // (3) RED: always emit `start / end` → `j.gridColumn` becomes `"1 / auto"`. CSSOM omits a
    // trailing component equal to its initial value; Chrome answers `"1"`.
    assert!(
        has("item=2 / 4|span 2|1|auto"),
        "`grid-column`/`grid-row` serialise as the shorthand, dropping a trailing `auto` — got {got:?}"
    );

    // (4) RED: add the names to the object literal but not to `COMPUTED_STD_NAMES` (or the reverse)
    // → one spelling works and the other returns `""`.
    assert!(
        has("kebab=column dense|30px|"),
        "the camelCase attribute and `getPropertyValue` are two paths in this object and both must \
         answer — got {got:?}"
    );

    // (5) **THE WITHHOLDING IS AN ASSERTION, NOT AN OMISSION.** RED: publish
    // `grid-template-columns` from the cascade → `withheld=false/false`, and every caller that asks
    // a 900px `repeat(3, 1fr)` grid for its tracks gets the string `"repeat(3, 1fr)"` where Chrome
    // gives `"300px 300px 300px"`. Delete this assertion only in the same commit that lands the USED
    // track sizes out of taffy's `set_detailed_grid_info`.
    assert!(
        has("withheld=true/true"),
        "`grid-template-columns`/`-rows` resolve to the USED track sizes (CSSOM §5.1). Until layout \
         hands those over, publishing the specified tracks would be a wrong answer of the right \
         type — got {got:?}"
    );
}
