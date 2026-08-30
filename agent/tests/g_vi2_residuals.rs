//! **G_VI2_RESIDUALS — CONSTITUTION.MD VI.2's named residual layout gap, re-measured against Chrome
//! and banked.**
//!
//! `CONSTITUTION.MD` VI.2's H0.1 row is the loop's ranking instrument for *where the residual layout
//! gap lives*. It names the box types that opt **out** of ordinary block sizing: tables, inline
//! composition, floats and `clear`, out-of-flow boxes under a transformed containing block, and the
//! intrinsic measurement pass. Check #129's STEER #2 was *"re-measure VI.2's remaining named
//! residuals before ranking against them"*, after t1362 found three of its table entries had been
//! false for ~427 ticks.
//!
//! This is that battery. Eleven cases, one per named category, every number headless-Chrome-measured.
//! **Nine are Chrome-exact and are banked here; two diverge and are pinned below.**
//!
//! ```text
//!                                                            manuk        chrome
//!   1  anonymous table row (<table><td> with no <tr>)      [2 2 12x26]  [2 2 12x26]   ✓
//!   2  anonymous table (display:table-cell, no table)      [0 0 39x24]  [0 0 39x24]   ✓
//!   3  inline box holding no text of its own (t934/t935)   [0 -11 5x46] [0 -11 5x46]  ✓
//!   4  self-collapsing box between two margins (t1001)     [0 54 …]     [0 54 …]      ✓
//!   5  float placed at the TOP of its line (t1002, §9.5)   [0 0 80x20]  [0 0 80x20]   ✓
//!   6  abspos under a TRANSFORMED containing block (t1012) [-10 -5 60x20] same        ✓
//!   7  percentage height in an auto-height parent          [0 0 400x24] same          ✓
//!   8  shrink-to-fit width of a float                      [0 0 241x24] same          ✓
//!   9  `clear:left` past a float                           [0 40 400x24] same         ✓
//!  10  inline-block baseline with overflow:hidden          [10 0 30x40] same          ✓
//!  11  UA `table { border-spacing: 2px }`                  [2 2] h=30   [2 2] h=30    ✓ (fixed here)
//! ```
//!
//! ## ⚠⚠⚠ ROW 11 WAS A UA-SHEET TWIN DRIFT, AND IT MIS-MEASURED EVERY TABLE GATE ON THIS HARNESS
//!
//! `stylo_engine.rs` gained `table { display: table; border-spacing: 2px }` at t908 with a note
//! saying it had been missing. **Its hand-maintained twin in `MinimalCascade` never did**, and the
//! two sheets have disagreed ever since:
//!
//! ```text
//!                      cell offset in the table     table height
//!   Chrome                    [2, 2]                     30
//!   Stylo (shipping)          [2, 2]                     30
//!   MinimalCascade            [0, 0]                     26
//! ```
//!
//! That is not only a rendering bug in the JS-less build. `engine/layout`'s 191 unit tests and
//! everything under `agent/tests/` cascade through `MinimalCascade`, so **every table fixture on
//! those harnesses measured 4px short in both axes** unless it set `border-spacing` itself. The rule
//! from t923 stands: a UA declaration lives in BOTH sheets or in NEITHER.
//!
//! ⭐ **It was found by running one battery on BOTH paths and getting different answers.** The first
//! run of this battery was on `agent/tests` alone, and it reported the table rows as an engine
//! defect; re-running the identical fixture through `manuk-page --features stylo` returned Chrome's
//! numbers. That is t1361's lesson applied to the *measurement* rather than to the engine:
//! **measure on the shipping path, or say which cascade you measured.**
//!
//! ## ⭐ THE ONE ROW THAT WAS A REAL SHIPPING DIVERGENCE — BUILT, AND `c5` NOW ASSERTS 24
//!
//! ```text
//!   A FLOAT THAT FOLLOWS INLINE TEXT DOES NOT RE-FLOW THE LINE IT JOINS
//!
//!     <div style="width:400px">xxxx xxxx xxxx<div style="float:left;width:80px;height:20px">
//!     </div>yyyy</div>
//!                                        Chrome    before    after
//!       the float's own rect             [0 0 80x20]  same    same   (t1002 placed it correctly)
//!       the BLOCK's height                   24        48       24
//!
//!     the same with a 380px float that cannot fit beside the text:
//!       the float drops to y=24 in both  [0 24 380x20] same    same
//!       the BLOCK's height                   24        48       24
//!
//!     CONTROL — float FIRST, then the text: 24 in all three
//! ```
//!
//! t1002 fixed *where the float goes* (§9.5 rule 6 — the top of the line, not below the run). The
//! remaining half was that the inline content already flushed onto that line was not re-laid around
//! the float, so the text kept its original x positions and the trailing text was pushed to a second
//! line. **`layout_block` no longer commits that flush**: the pending run is laid out as a trial to
//! answer rule 6's question, the trial's boxes are discarded, and the one real flush happens at the
//! end with the float already in the context — so the line re-flows in both directions. The full
//! ten-shape battery, with the text positions this row cannot see, is `G_FLOAT_LINE_REFLOW`.
//!
//! ⚠ **`c5`'s height moved from unasserted to asserted here, and that is the whole point of how it
//! was carried.** Asserting Chrome's 24 while we produced 48 would have been a red gate; asserting
//! our 48 would have PINNED THE BUG (the t1004 shape). It was recorded in this header instead, with
//! `(5, 24.0)` named as the row that would join the list the day the re-flow landed. It has.
//!
//! ⚠ This gate lives in `agent/tests/` for the reason surface audit #78 measured: 502 of the 522
//! gate files the ratchet counts are executed by no automatic runner, because `manuk-page` is in
//! neither the wall's nor CI's crate list. `scripts/` is observer-owned.

use manuk_dom::NodeId;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font-family:monospace;font-size:16px;line-height:24px}
*{box-sizing:border-box}
.w{width:400px;margin:0 0 8px 0;position:relative}
</style></head><body>
<div class="w" id="c1"><table><td id="e1">a</td><td id="e1b">b</td></table></div>
<div class="w" id="c2"><div style="display:table-row"><div style="display:table-cell" id="e2">cell</div></div></div>
<div class="w" id="c3"><span style="font-size:40px" id="e3"><span style="font-size:8px">x</span></span></div>
<div class="w" id="c4"><div style="margin-bottom:20px">a</div><div id="e4"></div><div style="margin-top:30px" id="e4b">b</div></div>
<div class="w" id="c5">xxxx xxxx xxxx<div style="float:left;width:80px;height:20px;background:#333" id="e5"></div>yyyy</div>
<div class="w" id="c6"><div style="transform:scale(2);width:100px;height:50px;position:relative"><div style="position:absolute;left:20px;top:10px;width:30px;height:10px;background:#333" id="e6"></div></div></div>
<div class="w" id="c7"><div style="height:auto"><div style="height:50%;background:#333" id="e7">p</div></div></div>
<div class="w" id="c8"><div style="float:left;background:#333" id="e8">hello world wrapping text</div></div>
<div class="w" id="c9"><div style="float:left;width:50px;height:40px"></div><div style="clear:left" id="e9">after</div></div>
<div class="w" id="c10">A<span style="display:inline-block;overflow:hidden;width:30px;height:40px;background:#333" id="e10"></span></div>
<div class="w" id="c11"><table><tr><td id="e11">a</td></tr></table></div>
</body></html>"##;

fn by_id(page: &manuk_page::Page, id: &str) -> NodeId {
    let dom = page.dom();
    dom.descendants(dom.root())
        .find(|&n| {
            dom.element(n)
                .and_then(|e| e.attr("id"))
                .is_some_and(|v| v == id)
        })
        .unwrap_or_else(|| panic!("VACUOUS: no element with id={id:?}"))
}

fn rect(page: &manuk_page::Page, id: &str) -> manuk_layout::Rect {
    *page
        .root_box
        .node_rects(page.dom())
        .get(&by_id(page, id))
        .unwrap_or_else(|| panic!("VACUOUS: no box for id={id:?}"))
}

#[test]
fn vi2_named_residuals_match_chrome() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://vi2.test/", &fonts, 1200.0);
    let near = |g: f32, w: f32| (g - w).abs() < 1.6;
    let d = |id: &str, w: &str| {
        let (a, x) = (rect(&page, id), rect(&page, w));
        (a.x - x.x, a.y - x.y, a.width, a.height)
    };
    let same = |g: (f32, f32, f32, f32), w: (f32, f32, f32, f32)| {
        near(g.0, w.0) && near(g.1, w.1) && near(g.2, w.2) && near(g.3, w.3)
    };

    // ── VACUITY. Eleven wrappers, each with a laid-out box. Height is asserted per-wrapper against
    // Chrome rather than as `> 0`, because ⚠ **`c8` is legitimately ZERO** — a block whose only
    // child is a float, and which is not a BFC root, does not contain it. A blanket `height > 0`
    // vacuity check called that a missing box on the first run of this gate; the row is now a claim
    // instead of an exception.
    // ⭐ **`c5` WAS ABSENT FROM THIS LIST ON PURPOSE UNTIL THE RE-FLOW LANDED.** Chrome measures it
    // 24 and we produced 48 (the float-after-text re-flow named in this module's header). Asserting
    // Chrome's 24 then would have landed a RED gate; asserting our 48 would have PINNED THE BUG,
    // which is the t1004 shape this project has caught before. So the number lived in the header
    // with a standing promise that `(5, 24.0)` joins this list the day the re-flow lands. It did.
    let wrapper_h: [(usize, f32); 11] = [
        (1, 30.0),
        (2, 24.0),
        (3, 36.0),
        (4, 78.0),
        (5, 24.0),
        (6, 50.0),
        (7, 24.0),
        (8, 0.0),
        (9, 64.0),
        (10, 47.0),
        (11, 30.0),
    ];
    for (n, want) in wrapper_h {
        let r = rect(&page, &format!("c{n}"));
        assert!(
            r.width > 0.0,
            "VACUOUS: wrapper c{n} has no box at all ({r:?})"
        );
        assert!(
            near(r.height, want),
            "G_VI2_RESIDUALS wrapper c{n}: Chrome measures {want} tall, got {}. Every wrapper's \
             height is a Chrome-measured claim here, including c8's ZERO — a non-BFC block does \
             not contain its float.",
            r.height
        );
    }

    // (id, wrapper, chrome rect, what the row is about)
    let rows: &[(&str, &str, (f32, f32, f32, f32), &str)] = &[
        ("e1", "c1", (2.0, 2.0, 12.0, 26.0), "anonymous table row — the HTML parser inserts the implied <tbody>/<tr>, and the UA border-spacing applies to it"),
        ("e1b", "c1", (16.0, 2.0, 12.0, 26.0), "the second anonymous-row cell — 2px of border-spacing between the two"),
        ("e2", "c2", (0.0, 0.0, 39.0, 24.0), "an anonymous TABLE generated around a bare display:table-row"),
        ("e3", "c3", (0.0, -11.0, 5.0, 46.0), "t934/t935 — an inline box that holds no text of its own still contributes its own leading"),
        ("e4b", "c4", (0.0, 54.0, 400.0, 24.0), "t1001 — a self-collapsing box between two margins collapses THROUGH, so 20 and 30 do not add to 50"),
        ("e5", "c5", (0.0, 0.0, 80.0, 20.0), "t1002 / CSS 2.1 §9.5 rule 6 — a float following text sits at the TOP of that line, not below the run"),
        ("e6", "c6", (-10.0, -5.0, 60.0, 20.0), "t1012 — an abspos child under a scale(2) containing block: its offsets are untransformed pixels, its box is transformed"),
        ("e7", "c7", (0.0, 0.0, 400.0, 24.0), "a percentage height against an AUTO-height parent resolves to auto, not to zero"),
        ("e8", "c8", (0.0, 0.0, 241.0, 24.0), "a float's shrink-to-fit width is max-content capped by the available space"),
        ("e9", "c9", (0.0, 40.0, 400.0, 24.0), "clear:left drops past the float's bottom margin edge"),
        ("e10", "c10", (10.0, 0.0, 30.0, 40.0), "an inline-block with overflow:hidden takes its BOTTOM MARGIN EDGE as its baseline"),
        ("e11", "c11", (2.0, 2.0, 12.0, 26.0), "the UA default `table { border-spacing: 2px }` — present in the Stylo sheet since t908 and MISSING from its MinimalCascade twin until t1364"),
    ];
    for (id, w, want, why) in rows {
        let got = d(id, w);
        assert!(
            same(got, *want),
            "G_VI2_RESIDUALS `{id}`: Chrome measures {want:?}, got {got:?}.\n  {why}"
        );
    }

    // ── VACUITY for the row that was pinned and is now built: `c5` is asserted twice over — its
    //    height above (24, the re-flow) and its float's own rect in the table above (`e5`, the
    //    t1002 placement half). Both must hold at once, which is what makes the row a statement
    //    about one line box rather than two independent numbers that could each be right alone.
    let c5 = rect(&page, "c5").height;
    assert!(
        c5 > 0.0 && rect(&page, "e5").width > 0.0,
        "VACUOUS: the float-after-text row did not lay out at all, so `e5` above proves nothing \
         (c5 height {c5})"
    );
}
