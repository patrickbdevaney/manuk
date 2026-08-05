//! # G_INTRINSIC_MIN_MAX — the intrinsic sizing keywords on `min-width` / `max-width` /
//! `min-height` / `max-height`
//!
//! ```css
//! .cell   { max-width: min-content }   /* keep this column as narrow as its longest word */
//! .toolbar{ min-width: max-content }   /* never let these buttons wrap */
//! .panel  { max-height: fit-content }  /* stop growing once the content stops */
//! ```
//!
//! `min-content` / `max-content` / `fit-content` are legal on all four min/max sizing properties
//! (CSS Sizing L3 §5) exactly as they are on `width`/`height`. **Both cascades dropped them.**
//! `ComputedStyle` carried a keyword sidecar for `width` (`width_keyword`) and for `height`
//! (`height_intrinsic`) and *none* for the four min/max properties, so the keyword fell through to
//! `Dim::Auto` — which the clamp reads as **0 on a min** and as **no limit on a max**.
//!
//! That is a wrong answer of the *right type*, the shape this project rates most dangerous: the
//! declaration parsed to a different, valid value and then silently did nothing. `max-width:
//! min-content` left the box filling its container; `min-width: max-content` let it crush below its
//! own content.
//!
//! ## Chrome-measured on THIS fixture
//!
//! `google-chrome --headless=new --hide-scrollbars --window-size=1200,800`, `font:16px/1.5
//! monospace`, `"hello there world"` (min-content 48.17, max-content 163.77), 400px containing
//! block unless stated:
//!
//! ```text
//!                                                    Chrome   before   after
//!   width:min-content                (CONTROL)        48.17     48       48   ✓ always right
//!   width:max-content                (CONTROL)       163.77    164      164   ✓ always right
//!   max-width:min-content                             48.17    400       48   ✗→✓
//!   max-width:max-content                            163.77    400      164   ✗→✓
//!   max-width:fit-content                            163.77    400      164   ✗→✓
//!   min-width:min-content            (20px CB)        48.17     20       48   ✗→✓
//!   min-width:max-content            (20px CB)       163.77     20      164   ✗→✓
//!   min-width:fit-content            (20px CB)        48.17     20       48   ✗→✓
//!   width:400px; max-width:min-content                48.17    400       48   ✗→✓
//!   width:10px;  min-width:max-content               163.77     10      164   ✗→✓
//!   height:200px; max-height:min-content   (h)        48       200       48   ✗→✓
//!   height:1px;   min-height:min-content   (h)        48         1       48   ✗→✓
//!   max-width:200px                  (CONTROL)       200       200      200   ✓ must not move
//!   min-width:300px                  (CONTROL)       300       300      300   ✓ must not move
//! ```
//!
//! ⚠ **The two CONTROL rows at the bottom are the point of the fixture, not decoration.** The fix
//! adds a branch *in front of* every min/max clamp in three layout paths; a version that took the
//! new branch unconditionally would satisfy every `✗→✓` row above and destroy ordinary length
//! clamps, which are what the corpus is actually made of.
//!
//! ⚠⚠ **The keyword rows are asserted against their CONTROL, not against a hard-coded pixel
//! width.** `48.17` is *this* monospace face's min-content width and would make the gate a font
//! assertion. The claim that has teeth is `max-width:min-content` == `width:min-content` — same
//! measurement, two spellings — plus "and it is not the container width", which is exactly the
//! before-state. Heights use `line-height:1.5` at `16px`, so two lines are 48 on any face.
//!
//! ## How this goes RED
//!
//! - **Restore `let min_w = (s.min_width.resolve(cw, 0.0) - bs_extra_w).max(0.0)` / the plain
//!   `match s.max_width` in `layout_block`** (i.e. delete the `*_keyword` arms) → every `✗→✓` row
//!   above snaps back to 400 / 20 / 200 / 1 while both CONTROL rows and both `width:` controls stay
//!   green. Verified.
//! - **Delete the sidecar from `stylo_map.rs` alone**, leaving the minimal cascade's — the gate
//!   still goes RED, which is the property t925 had to learn the hard way: this fixture runs the
//!   SHIPPING cascade, so a proof aimed at `MinimalCascade` would have passed against a broken
//!   engine.
//!
//! ## NOT covered, named rather than left looking handled
//!
//! **A FLEX ITEM's intrinsic min/max is still dropped** — `display:flex` + `flex:1;
//! max-width:min-content` measures 400 against Chrome's 48.17. That path is taffy's: `to_taffy_style`
//! maps `min_size`/`max_size` to a `Dimension`, taffy 0.12 has no intrinsic-keyword variant, and the
//! keyword would have to be resolved to px *before* the style is built — which needs the measurer
//! plumbed into a function that today takes only a `ComputedStyle`. A different mechanism, with its
//! number recorded here.

use manuk_text::FontContext;

/// `monospace` and an explicit `line-height` keep the *heights* font-independent (two lines = 48 at
/// `16px/1.5`); the *widths* are asserted against a control box rather than a constant, so the face's
/// own advance width never enters an assertion.
const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0;font:16px/1.5 monospace}
.cb{width:400px}
.narrow{width:20px}
</style></head><body>
<div class="cb"><div id="w_min" style="width:min-content">hello there world</div></div>
<div class="cb"><div id="w_max" style="width:max-content">hello there world</div></div>

<div class="cb"><div id="mx_min" style="max-width:min-content">hello there world</div></div>
<div class="cb"><div id="mx_max" style="max-width:max-content">hello there world</div></div>
<div class="cb"><div id="mx_fit" style="max-width:fit-content">hello there world</div></div>

<div class="narrow"><div id="mn_min" style="min-width:min-content">hello there world</div></div>
<div class="narrow"><div id="mn_max" style="min-width:max-content">hello there world</div></div>
<div class="narrow"><div id="mn_fit" style="min-width:fit-content">hello there world</div></div>

<div class="cb"><div id="wx_min" style="width:400px;max-width:min-content">hello there world</div></div>
<div class="cb"><div id="wn_max" style="width:10px;min-width:max-content">hello there world</div></div>

<div class="cb"><div id="hx_min" style="height:200px;max-height:min-content">hello<br>there</div></div>
<div class="cb"><div id="hn_min" style="height:1px;min-height:min-content">hello<br>there</div></div>

<div class="cb"><div id="ctl_mx" style="max-width:200px">hello there world</div></div>
<div class="cb"><div id="ctl_mn" style="width:100px;min-width:300px">hello there world</div></div>
</body></html>"##;

fn rect_of(page: &manuk_page::Page, sel: &str) -> manuk_layout::Rect {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    *page
        .root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel}"))
}

#[test]
fn g_intrinsic_min_max() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://imm.test/", &fonts, 1200.0);

    // ── The two CONTROLS the keyword rows are measured against. These have always worked; if they
    //    move, every assertion below is measuring the wrong thing and says so rather than passing.
    let minc = rect_of(&page, "#w_min").width;
    let maxc = rect_of(&page, "#w_max").width;
    assert!(
        minc > 1.0 && maxc > minc + 1.0 && maxc < 400.0,
        "G_INTRINSIC_MIN_MAX: the CONTROLS are wrong, so nothing below is a test of anything — \
         `width:min-content`={minc}, `width:max-content`={maxc}. Expected 0 < min < max < 400 \
         (Chrome: 48.17 and 163.77 for this text)."
    );

    let check = |sel: &str, want: f32, why: &str| {
        let got = rect_of(&page, sel).width;
        assert!(
            (got - want).abs() < 1.01,
            "G_INTRINSIC_MIN_MAX: `{sel}` expected width={want}, got {got}.\n  {why}"
        );
    };

    // ── max-width: the keyword is a real cap. Before the fix every one of these was 400 — the
    //    containing block — because `Dim::Auto` in the max slot means "no limit".
    check(
        "#mx_min",
        minc,
        "`max-width:min-content` caps the box at its longest unbreakable run — the same measurement \
         `width:min-content` makes. It was rendering at the full 400px container width.",
    );
    check(
        "#mx_max",
        maxc,
        "`max-width:max-content` caps at the unwrapped content width (163.77 in Chrome).",
    );
    check(
        "#mx_fit",
        maxc,
        "`fit-content` = min(max-content, max(min-content, stretch-fit)); the container is wider \
         than max-content, so it resolves to max-content here.",
    );

    // ── min-width: the keyword is a real floor. Before the fix every one of these was 20 — the
    //    narrow container — because `Dim::Auto` in the min slot means 0.
    check(
        "#mn_min",
        minc,
        "`min-width:min-content` in a 20px container floors the box at its longest word. It was \
         being crushed to 20px, which is the overflow every `min-width` exists to prevent.",
    );
    check(
        "#mn_max",
        maxc,
        "`min-width:max-content` is the do-not-wrap idiom — a toolbar/nav row that must keep its \
         items on one line. It was crushed to 20px.",
    );
    check(
        "#mn_fit",
        minc,
        "`fit-content` against a 20px stretch-fit clamps to min-content, NOT max-content — the two \
         keywords must not be aliased, which is what makes this row distinct from `#mn_max`.",
    );

    // ── The keyword competing with an explicit length on the same box.
    check(
        "#wx_min",
        minc,
        "an explicit `width:400px` LOSES to a binding `max-width:min-content` (CSS2 §10.4).",
    );
    check(
        "#wn_max",
        maxc,
        "an explicit `width:10px` LOSES to a binding `min-width:max-content`.",
    );

    // ── The block axis. All three keywords name the content height for a block box, so these are
    //    the same claim twice, in the two directions.
    let hx = rect_of(&page, "#hx_min").height;
    assert!(
        (hx - 48.0).abs() < 1.01,
        "G_INTRINSIC_MIN_MAX: `height:200px; max-height:min-content` expected 48 (two 24px lines at \
         16px/1.5 — Chrome-measured), got {hx}. It was rendering the full 200px."
    );
    let hn = rect_of(&page, "#hn_min").height;
    assert!(
        (hn - 48.0).abs() < 1.01,
        "G_INTRINSIC_MIN_MAX: `height:1px; min-height:min-content` expected 48, got {hn}. It was \
         rendering 1px — the floor was read as 0."
    );

    // ── WHAT MUST NOT MOVE. A fix that took the keyword branch unconditionally passes everything
    //    above and destroys ordinary length clamps, which is what the corpus is made of.
    check(
        "#ctl_mx",
        200.0,
        "a plain `max-width:200px` is untouched — this is the row a too-broad fix breaks.",
    );
    check(
        "#ctl_mn",
        300.0,
        "a plain `min-width:300px` over `width:100px` is untouched.",
    );
}
