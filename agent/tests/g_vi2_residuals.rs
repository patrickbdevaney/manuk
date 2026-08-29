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
//! ## ⚠ NAMED, MEASURED, NOT BUILT — the one row that is a real shipping divergence
//!
//! ```text
//!   A FLOAT THAT FOLLOWS INLINE TEXT DOES NOT RE-FLOW THE LINE IT JOINS
//!
//!     <div style="width:400px">xxxx xxxx xxxx<div style="float:left;width:80px;height:20px">
//!     </div>yyyy</div>
//!                                        Chrome   ours
//!       the float's own rect             [0 0 80x20]  [0 0 80x20]   ✓ (t1002 placed it correctly)
//!       the BLOCK's height                   24        48           ✗ we make a second line
//!
//!     the same with a 380px float that cannot fit beside the text:
//!       the float drops to y=24 in both  [0 24 380x20] same         ✓
//!       the BLOCK's height                   24        48           ✗
//!
//!     CONTROL — float FIRST, then the text: 24 in both               ✓
//! ```
//!
//! t1002 fixed *where the float goes* (§9.5 rule 6 — the top of the line, not below the run). The
//! remaining half is that **the inline content already flushed onto that line is not re-laid around
//! the float**, so the text keeps its original x positions, the float overlaps it, and the trailing
//! text is pushed to a second line. `layout_block`'s own comment names this exactly: *"`place()`
//! cannot see this for us: it scans bands of FLOATS, and what is in the way here is the line's own
//! already-placed inline content."* Doubling the height of any block whose paragraph contains a
//! mid-text float is a large `dy` and floats are on 60.4% of the declared corpus, so this is the
//! ranked next tick — and it is a re-flow, not a placement tweak, which is why it is pinned rather
//! than attempted at the end of a battery.
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
    // ⚠ **`c5` IS ABSENT FROM THIS LIST ON PURPOSE, AND THAT IS THE HONEST HANDLING OF A KNOWN
    // DIVERGENCE.** Chrome measures it 24; we produce 48 (the float-after-text re-flow named in this
    // module's header). Asserting Chrome's 24 would land a RED gate; asserting our 48 would PIN THE
    // BUG, which is the t1004 shape this project has caught before. So the number is recorded in the
    // header, the row is not asserted, and the day the re-flow lands, `(5, 24.0)` joins this list.
    let wrapper_h: [(usize, f32); 10] = [
        (1, 30.0),
        (2, 24.0),
        (3, 36.0),
        (4, 78.0),
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

    // ── PINNED NEGATIVE — the CONTROL half of the float divergence, which IS correct and must stay
    //    correct: a float written BEFORE the text it shares a line with produces one line box.
    //    The failing direction (a float AFTER text) is documented in the module header as NAMED,
    //    MEASURED, NOT BUILT and is deliberately not asserted here — a gate that asserts a value we
    //    do not produce is a red gate, not a record.
    // The float-after-text row's PLACEMENT half (t1002) is already asserted above as `e5`, and it
    // is the half that must not regress while the re-flow half is outstanding: the float's own rect
    // is Chrome-exact even though the block it lives in is twice too tall.
    let c5 = rect(&page, "c5").height;
    assert!(
        c5 > 0.0 && rect(&page, "e5").width > 0.0,
        "VACUOUS: the float-after-text row did not lay out at all, so `e5` above proves nothing \
         (c5 height {c5})"
    );
}
