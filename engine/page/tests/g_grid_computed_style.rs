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
//! ⚠⚠⚠ **AND THE LAST TWO MEMBERS OF THE FAMILY RESOLVE TO THE *USED* VALUE, WHICH IS WHY THEY
//! NEEDED LAYOUT AND NOT THE CASCADE (t1270).** CSSOM §5.1 makes `grid-template-columns` /
//! `grid-template-rows` two of the handful of properties whose resolved value is the **used** value:
//! Chrome answers a grid container with its laid-out track sizes in px, so `repeat(3, 1fr)` on a
//! 900px grid reads back `"300px 300px 300px"`. t1269 held only the SPECIFIED tracks and therefore
//! **withheld both properties entirely** — `"repeat(3, 1fr)"` would have been a *wrong answer of the
//! right type*, which every caller then does arithmetic on.
//!
//! **The used sizes existed the whole time.** taffy resolves every track — that *is* grid layout —
//! and offers the result through `LayoutGridContainer::set_detailed_grid_info`, a trait method whose
//! **default body is a no-op**, under a `detailed_layout_info` feature taffy 0.12 enables by default.
//! Nothing had to be turned on; only the body had to exist. Assertions (5) and (6) below are what
//! that bought, and they replace the withholding assertion this gate shipped with.
//!
//! ⚠⚠ **THE WRITE-POLARITY HAZARD WAS t1120's, AND PRICING IT BEFORE THE CODE IS WHAT MADE THE
//! ANSWER CHEAP — but the falsification came back INERT, and that is recorded here rather than
//! quietly dropped.** `solve_subtree` also runs under INTRINSIC MEASUREMENT: a max-content probe
//! builds a *second* `TaffyDom` and lays the same subtree out at a huge available width, and a side
//! table written by a pass whose outputs are contractually discarded is exactly how
//! `pre_transform_rect` was poisoned permanently. Three things were established, in this order:
//!
//! 1. **Inside one tree, a sizing request never writes at all** — taffy's `compute_grid_layout`
//!    returns at `run_mode == RunMode::ComputeSize` (0.12.1 `compute/grid/mod.rs:543`) **160 lines
//!    before** it reaches `set_detailed_grid_info` at 703. That is a reading of the dependency, not
//!    an assumption about it.
//! 2. **The nested probe tree DOES write**, and is dropped at the recording site by
//!    `Ctx::intrinsic_probe` — the flag `record_transform` has gated on since t1120.
//! 3. ⚠ **AND REPLACING THAT GUARD WITH `if true` CHANGES NOTHING ON THIS FIXTURE.** Measured, not
//!    assumed: the `#fg` arm below is a grid inside a flex row, which *is* max-content probed, and
//!    `probed=` reads `300px 300px` guarded and unguarded alike. The reason is ordering — a flex
//!    container measures its items and *then* lays them out, so the real pass is the LAST write
//!    either way, and last-write-wins already carries it. **The guard is defence in depth against an
//!    ordering this fixture does not produce, not the load-bearing mechanism**, and saying so is
//!    cheaper than a future tick rediscovering that deleting it stays green.

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
 /* The USED-value case: three `1fr` columns in a definite 900px box must read back as px, and the
    numbers must SUM to the container's content width — the property of a used value that a
    specified-value answer can never accidentally satisfy. */
 #u { display: grid; width: 900px;
      grid-template-columns: repeat(3, 1fr);
      grid-template-rows: 40px 60px; }
 /* The NON-grid control. `grid-template-*` resolves to the COMPUTED value on anything that is not a
    grid container, which is why the used-value rule cannot simply be applied everywhere. */
 #nb { grid-template-columns: 100px 200px; }
 #f  { display: flex; width: 600px; }
 #fg { display: grid; grid-template-columns: 1fr 1fr; flex: 1; }
</style></head><body>
 <div id="f"><div id="fg"><div></div><div></div></div></div>
 <div id="g"><div id="i"></div><div id="j"></div></div>
 <div id="u"><div></div><div></div></div>
 <div id="nb"></div>
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

     // (5) THE USED TRACK SIZES — the CSSOM §5.1 rule, and the whole reason these two were
     // withheld until layout could answer them.
     var u = $('u');
     parts.push('used=' + u.gridTemplateColumns + '/' + u.gridTemplateRows);
     parts.push('decl=' + g.gridTemplateColumns);

     // (6) THE NON-GRID ARM. On anything that is not a grid container the resolved value is the
     // COMPUTED one — the author's list when declared, `none` when not.
     parts.push('nongrid=' + $('nb').gridTemplateColumns + '|' + p.gridTemplateColumns +
                '|' + p.gridTemplateRows);
     parts.push('probed=' + $('fg').gridTemplateColumns);

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

    // (5) **THE USED VALUE, AND THE NUMBER IS THE PROOF.** RED, proven by restoring taffy's default:
    // empty the `set_detailed_grid_info` body in `taffy_tree.rs` and this reads
    //
    // ```text
    //   used=1fr 1fr 1fr/40px 60px
    // ```
    //
    // ⚠⚠⚠ **`1fr 1fr 1fr` IS THE EXACT STRING t1269 REFUSED TO SHIP, AND THAT IS THE MOST USEFUL
    // THING THIS FALSIFICATION FOUND.** The prediction written beside it was `used=none/none`, on the
    // assumption that no tracks means no answer. Wrong: with the layout side-table empty, the
    // serialiser falls through to the CASCADE arm, and the caller gets a *wrong answer of the right
    // type* rather than an absence. **The fallback that makes the non-grid arm (6) correct is also
    // what makes a regression here SILENT in shape** — it can only ever be caught by the VALUE, which
    // is why this assertion pins the digits and not the presence.
    //
    // (That fallback is not itself a bug: an element with no layout is precisely the `display:none`
    // case, where Chrome also answers the computed track list rather than used px.)
    //
    // ⚠ `300px` three times is not merely "a px value": it SUMS to the container's 900px content
    // box, which a specified-value answer cannot accidentally satisfy.
    assert!(
        has("used=300px 300px 300px/40px 60px"),
        "`grid-template-columns`/`-rows` resolve to the USED track sizes (CSSOM §5.1) — three `1fr` \
         columns in a 900px grid are `300px` each, and they must SUM to the content box — got {got:?}"
    );
    // …and **THE IMPLICIT THIRD TRACK IS THE PROOF THAT THIS COMES FROM LAYOUT, because the cascade
    // cannot produce it.** `#g` declares exactly two columns; `#i` is placed `grid-column: 2 / 4`,
    // which reaches a line the explicit grid does not have, so the auto-placement algorithm creates
    // an implicit third column sized by `grid-auto-columns: minmax(100px, auto)` — and an `auto`
    // max sizing function STRETCHES to absorb the free space (CSS Grid §12.8, `justify-content`
    // being `normal`), which is where 484px comes from in a 784px content box.
    //
    // A serialiser reading `cs.grid_template_columns` would answer `"100px 200px"` and could not
    // ever answer anything else. So this assertion is not a duplicate of (5) with different numbers:
    // it is the one that cannot be satisfied by the implementation t1269 rejected.
    assert!(
        has("decl=100px 200px 484px"),
        "a grid container reports the tracks LAYOUT produced, including the IMPLICIT one an item \
         spanning past the explicit grid created — a cascade-derived answer has only two — got {got:?}"
    );

    // (6) **THE NON-GRID ARM, WHICH IS WHY THE USED RULE CANNOT JUST BE APPLIED EVERYWHERE.** RED:
    // return `"none"` whenever the layout side-table has no entry → `nongrid=none|none|none`, and a
    // stylesheet's own `grid-template-columns` becomes unreadable on any element that is not
    // currently a grid. Chrome resolves to the COMPUTED value there.
    //
    // ⚠ **The undeclared answer is `none`, not `auto`** — `grid-template-*` initialises to `none`
    // while its `grid-auto-*` siblings initialise to `auto`, and reusing the sibling serialiser
    // would have been silent: `auto` is a legal value, so `if (s.gridTemplateColumns !== 'none')`
    // — the standard "is this templated?" test — would pass on every element on the page.
    assert!(
        has("nongrid=100px 200px|none|none"),
        "on a NON-grid element `grid-template-*` resolves to the COMPUTED value — the declared list, \
         or `none` when undeclared (never `auto`, its siblings' initial) — got {got:?}"
    );

    // (7) **A GRID THAT IS MAX-CONTENT PROBED BEFORE IT IS LAID OUT REPORTS THE LAYOUT, NOT THE
    // PROBE.** `#fg` is a grid inside a 600px flex row, so the flex algorithm intrinsic-measures it
    // first — the pass whose outputs are contractually discarded, and the one that poisoned
    // `pre_transform_rect` at t1120 by being recorded first-wins.
    //
    // ⚠ **This assertion is the PROPERTY, deliberately not the MECHANISM**, because the two do not
    // coincide here: replacing the `intrinsic_probe` guard with `if true` leaves this string
    // unchanged (measured), since the real pass writes last regardless. Pinning the property keeps
    // the gate honest whichever of the two mechanisms a future edit removes.
    assert!(
        has("probed=300px 300px"),
        "a grid measured intrinsically before it is laid out must report the LAID-OUT tracks — \
         two `1fr` columns in a 600px flex item are 300px each — got {got:?}"
    );
}
