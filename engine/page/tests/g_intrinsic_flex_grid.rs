//! # G_INTRINSIC_FLEX_GRID — the intrinsic sizing keywords SURVIVE the crossing into taffy
//!
//! ```css
//! .toolbar > .label { max-width: min-content }  /* keep this cell as narrow as its longest word */
//! .nav    > .item   { min-width: max-content }  /* never let these buttons wrap */
//! .card   > .title  { width: min-content }
//! ```
//!
//! t930 taught `ComputedStyle` to *hold* an intrinsic keyword on all four min/max properties and
//! taught the block path to honour it. **The flex and grid path never saw any of it — not even the
//! `width_keyword` sidecar that has existed since t153.** `to_taffy_style` maps `cs.width` through
//! `dimension()`, and a `min-content` width is stored as `Dim::Auto` *plus a sidecar*; the sidecar
//! did not cross, so every intrinsic keyword on a flex or grid item silently became
//! `Dimension::Auto` — *"size me from my flex basis"*. A different, valid answer: the wrong answer
//! of the right type, one formatting context over.
//!
//! t930's own note recorded this as "a FLEX ITEM's intrinsic **min/max** is still dropped". Measured
//! against Chrome, that undersold it by half: plain `width: min-content` is wrong on a flex item and
//! on a **grid** item too.
//!
//! ## Chrome-measured on THIS fixture
//!
//! `google-chrome --headless --hide-scrollbars --window-size=1200,800`, `16px/normal serif`,
//! `"hello there world"` (min-content 37.33, max-content 109.30), 400px container unless stated:
//!
//! ```text
//!                                                     Chrome   before   after
//!   flex  width:100px                    (CONTROL)    100.00     100     100  ✓ must not move
//!   flex  width:fit-content              (CONTROL)    109.30     109     109  ✓ must not move
//!   flex  min-width:fit-content (20px CB)(CONTROL)     37.33      37      37  ✓ must not move
//!   flex  flex:1                         (CONTROL)    400.00     400     400  ✓ must not move
//!   grid  width:100px                    (CONTROL)    100.00     100     100  ✓ must not move
//!   grid  min-width:max-content          (CONTROL)    400.00     400     400  ✓ non-binding min
//!   flex  width:min-content                            37.33     109      37  ✗→✓
//!   flex  max-width:min-content                        37.33     109      37  ✗→✓
//!   flex  min-width:max-content (20px CB)             109.30      37     109  ✗→✓
//!   flex  flex:1; max-width:min-content                37.33     400      37  ✗→✓
//!   flex  flex:1; max-width:max-content               109.30     400     109  ✗→✓
//!   grid  width:min-content                            37.33     400      37  ✗→✓
//!   grid  max-width:min-content                        37.33     400      37  ✗→✓
//!   flex  padding:0 10px; max-width:min-content        57.33     129      57  ✗→✓  content-box
//!   flex  border-box; padding:0 10px; …:min-content    57.33     129      57  ✗→✓  border-box
//! ```
//!
//! ⚠ **The last two rows are one claim: `box-sizing` has NO effect on an intrinsic keyword.** The
//! grammar invites the opposite assumption, and only a measurement settles it — Chrome gives the
//! **same 57.33 border box** under `content-box` and under `border-box`. Taffy subtracts the frame
//! from `size` under border-box, so the frame is added back there specifically to land on the same
//! number either way. A fix that skipped this is wrong in exactly one of the two rows.
//!
//! ⚠⚠ **`fit-content` is left as `Dimension::Auto` on purpose, and the CONTROL rows are why.**
//! `fit-content` is `min(max-content, max(min-content, stretch-fit))` and the stretch-fit inside a
//! flex line is not known when the style is built. Taffy's `auto` + `flex-basis: auto` +
//! `flex-shrink` **is** that clamp, and it measures Chrome-exact in a wide container (109.30) and a
//! narrow one (37.33). Resolving it here would replace a correct answer with a guess — so the
//! `fit-content` rows are CONTROLS that a too-eager fix fails.
//!
//! ⚠ **The widths are asserted against a CONTROL BOX, never against 37.33.** That number is this
//! face's min-content advance; hard-coding it would make the gate a font assertion that breaks on
//! any machine with different fonts installed. The claim with teeth is
//! `flex max-width:min-content` == `block width:min-content` — same measurement, two formatting
//! contexts — plus "and it is not the container width", which is exactly the before-state.
//!
//! ## How this goes RED
//!
//! - **Delete the `resolve_intrinsic_inline` call in `TaffyDom::add`** → every `✗→✓` row snaps back
//!   to 109 / 37 / 400 / 129 while all six CONTROL rows stay green. Verified.
//! - **Drop the `frame` term** (always add 0) → the two `box-sizing` rows split: `content-box` stays
//!   at 57, `border-box` falls to 37. Verified.
//! - **Resolve `fit-content` to max-content as well** (the obvious over-generalisation) → the
//!   `min-width:fit-content` CONTROL in a 20px container goes 37 → 109. Verified.
//!
//! ## NOT covered, named with its number rather than left looking handled
//!
//! - **The BLOCK axis on a flex item.** `height:200px; max-height:min-content` measures **200**
//!   against Chrome's 18. A block-axis intrinsic size is the content height *at the item's resolved
//!   width*, and that width does not exist when the style is built — a different mechanism from the
//!   inline axis, where min-content and max-content are answerable with no context at all.
//! - **An item that is ITSELF a flex/grid container.** `display:flex; width:min-content` nested in a
//!   flex row measures **109.30** against Chrome's 37.33. Resolving it would re-enter: the measure
//!   callback answers a container's intrinsic width by building a *second* `TaffyDom` for that node,
//!   whose `add` reaches the resolver again on the same node and recurses without bound — **a Bar-0
//!   crash, not a wrong number.** The `container` guard at the call site is what keeps the recursion
//!   profile identical to before this tick; lifting it needs a root-suppression flag on the nested
//!   build.

use manuk_text::FontContext;

/// Every width assertion is made against a control box laid out in the BLOCK path, so no font
/// advance width is ever written down here.
const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0;font:16px/normal serif}
.cb{width:400px}
.narrow{width:20px}
.f{display:flex}
.g{display:grid}
</style></head><body>
<div class="cb"><div id="ref_min" style="width:min-content">hello there world</div></div>
<div class="cb"><div id="ref_max" style="width:max-content">hello there world</div></div>

<div class="cb f"><div id="fw_min" style="width:min-content">hello there world</div></div>
<div class="cb f"><div id="fx_min" style="max-width:min-content">hello there world</div></div>
<div class="narrow f"><div id="fn_max" style="min-width:max-content">hello there world</div></div>
<div class="cb f"><div id="ff_min" style="flex:1;max-width:min-content">hello there world</div></div>
<div class="cb f"><div id="ff_max" style="flex:1;max-width:max-content">hello there world</div></div>

<div class="cb g"><div id="gw_min" style="width:min-content">hello there world</div></div>
<div class="cb g"><div id="gx_min" style="max-width:min-content">hello there world</div></div>

<div class="cb f"><div id="pc_min" style="padding:0 10px;max-width:min-content">hello there world</div></div>
<div class="cb f"><div id="pb_min" style="box-sizing:border-box;padding:0 10px;max-width:min-content">hello there world</div></div>

<div class="cb f"><div id="ctl_len" style="width:100px">hello there world</div></div>
<div class="cb f"><div id="ctl_fit" style="width:fit-content">hello there world</div></div>
<div class="narrow f"><div id="ctl_nfit" style="min-width:fit-content">hello there world</div></div>
<div class="cb f"><div id="ctl_flex" style="flex:1">hello there world</div></div>
<div class="cb g"><div id="ctl_glen" style="width:100px">hello there world</div></div>
<div class="cb g"><div id="ctl_gmin" style="min-width:max-content">hello there world</div></div>
</body></html>"##;

fn width_of(page: &manuk_page::Page, sel: &str) -> f32 {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    page.root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel}"))
        .width
}

#[test]
fn g_intrinsic_flex_grid() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ifg.test/", &fonts, 1200.0);

    // ── The BLOCK-path reference every flex/grid row below is measured against. t930 made these
    //    correct; if they move, nothing below is a test of anything and it says so rather than
    //    passing quietly.
    let minc = width_of(&page, "#ref_min");
    let maxc = width_of(&page, "#ref_max");
    assert!(
        minc > 1.0 && maxc > minc + 1.0 && maxc < 400.0,
        "G_INTRINSIC_FLEX_GRID: the block-path REFERENCE is wrong, so nothing below means anything \
         — `width:min-content`={minc}, `width:max-content`={maxc}. Expected 0 < min < max < 400 \
         (Chrome: 37.33 and 109.30 for this text)."
    );

    let check = |sel: &str, want: f32, why: &str| {
        let got = width_of(&page, sel);
        assert!(
            (got - want).abs() < 1.01,
            "G_INTRINSIC_FLEX_GRID: `{sel}` expected width={want}, got {got}.\n  {why}"
        );
    };

    // ── A FLEX ITEM. Before the fix the keyword was invisible to taffy and every one of these took
    //    the flex-basis answer instead: 109 (max-content) in a wide container, 37 in a narrow one,
    //    400 when the item also grew.
    check(
        "#fw_min",
        minc,
        "`width:min-content` on a flex item is the SAME measurement the block path makes. It was \
         taking the flex-basis-auto answer (max-content) — plain `width:` was dropped too, which is \
         the half t930's note did not record.",
    );
    check(
        "#fx_min",
        minc,
        "`max-width:min-content` is a real cap on a flex item. It was rendering at max-content.",
    );
    check(
        "#fn_max",
        maxc,
        "`min-width:max-content` in a 20px container is the do-not-wrap idiom — a nav row that must \
         keep its items on one line. It was crushed to the min-content width.",
    );
    check(
        "#ff_min",
        minc,
        "a GROWING item (`flex:1`) still obeys `max-width:min-content` — this is the exact case \
         t930 named as open, and it was rendering the full 400px container.",
    );
    check(
        "#ff_max",
        maxc,
        "a growing item obeys `max-width:max-content`; it was rendering 400px.",
    );

    // ── A GRID ITEM. Same sidecar, same crossing, and the before-state is the stretched track.
    check(
        "#gw_min",
        minc,
        "`width:min-content` on a grid item. It was stretching to fill its 400px track.",
    );
    check(
        "#gx_min",
        minc,
        "`max-width:min-content` on a grid item. It was stretching to fill its 400px track.",
    );

    // ── `box-sizing` has NO effect on an intrinsic keyword — ONE claim, asserted in both spellings
    //    because the fix has to add the frame back under border-box and not under content-box.
    let pc = width_of(&page, "#pc_min");
    let pb = width_of(&page, "#pb_min");
    assert!(
        (pc - pb).abs() < 1.01 && (pc - (minc + 20.0)).abs() < 1.01,
        "G_INTRINSIC_FLEX_GRID: `box-sizing` must NOT change an intrinsic keyword. With \
         `padding:0 10px`, Chrome gives the same 57.33 border box either way; got content-box={pc}, \
         border-box={pb}, expected both = min-content({minc}) + 20."
    );

    // ── WHAT MUST NOT MOVE. Six rows, and the three `fit-content`/`flex:1` ones are the reason the
    //    fix resolves only `min-content` and `max-content`: taffy's `auto` already IS the
    //    stretch-fit clamp, and a fix that "helpfully" resolved fit-content too breaks them while
    //    satisfying every row above.
    check(
        "#ctl_len",
        100.0,
        "a plain `width:100px` flex item is untouched — the row a too-broad fix breaks.",
    );
    check(
        "#ctl_fit",
        maxc,
        "`width:fit-content` in a WIDE container resolves to max-content, and taffy's `auto` \
         already gets this right.",
    );
    check(
        "#ctl_nfit",
        minc,
        "`min-width:fit-content` in a 20px container clamps to MIN-content, not max-content. This \
         is the row that fails if `fit-content` is resolved eagerly to max-content.",
    );
    check(
        "#ctl_flex",
        400.0,
        "`flex:1` with no size constraint still fills the line.",
    );
    check(
        "#ctl_glen",
        100.0,
        "a plain `width:100px` grid item is untouched.",
    );
    check(
        "#ctl_gmin",
        400.0,
        "a NON-BINDING `min-width:max-content` (max-content < the 400px track) must leave the item \
         stretched — a min that is resolved but then applied as if it were a width fails here.",
    );
}
