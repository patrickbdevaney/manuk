//! # G_TEXT_ALIGN_JUSTIFY — the slack goes into the WORD GAPS, not into one offset
//!
//! `text-align: justify` was **parsed and then ignored**. `TextAlign::Justify` reached `close_line`,
//! fell through the `_ => 0.0` arm of the offset match, and rendered identically to `left` — for the
//! engine's whole life.
//!
//! Every other alignment is a single translation of the whole line, which is exactly why this one
//! fell through: it is the only value that is not an offset. The slack has to be distributed across
//! the line's word gaps.
//!
//! ```text
//!                                                      Chrome   before   after
//!   2nd word of a justified line                         49       45       49    ✗→✓
//!   6th word of the same line                           237      220      237    ✗→✓
//!   …the same words with NO justify (control)         45/220   45/220   45/220   ✓ must not move
//!   last line of a justified block                       43       43       43    ✓ must not move
//!   line ended by <br>, and the line after it          45/59    45/59    45/59   ✓ must not move
//!   one unbreakable word (no gaps to expand)              0        0        0    ✓ must not move
//!   text-align:center / :right in the same fixture   160/341  160/341  160/341   ✓ must not move
//!   an inline-block inside justified text                49       45       49    ✗→✓
//! ```
//!
//! It does not degrade gently. On a justified paragraph **every word after the first is misplaced**
//! and the error grows along the line, so one paragraph produces dozens of divergences — and
//! justified text is the default look of prose-heavy pages, newspapers, institutional and government
//! sites, and much of the non-English long tail this corpus is drawn from.
//!
//! ## The three call sites are the specification
//!
//! CSS Text §7.3: `justify` justifies every line EXCEPT the last, and except any line ended by a
//! **forced break**, which take `text-align-last` (`start` by default). `close_line` already had
//! exactly three callers — the `<br>` site, the wrap site and the final flush — so the eligibility
//! flag is not a heuristic, it is one boolean per caller. `#ps` (a short last line) and `#pb` (a
//! `<br>`) are in the fixture because justifying either is the most recognisable rendering bug the
//! property has: a three-word line stretched across the whole column.
//!
//! ## How this goes RED
//!
//! - **Restore `_ => 0.0` for `Justify`** → `#j1` reads 45 and `#j2` reads 220, while every
//!   must-not-move row passes. Those two numbers are the *unjustified* control's, which is the point:
//!   the defect was indistinguishable from `text-align: left`.
//! - **Pass `true` at the last-line call site** → `#s1` stretches off its own line. **This mutation
//!   is why the flag exists**, and it is the one a fix written from the property name alone gets
//!   wrong.
//! - ⚠ **Snapshot the gap positions BEFORE shifting, or the expansion stops accumulating.** Reading
//!   `line[i-1].x` inside the loop that has already moved it compares a moved fragment against an
//!   unmoved one, so every gap after the first measures as closed. I wrote it that way first: `#j1`
//!   landed exactly right and `#j2` was 10px short — a shift that stops accumulating looks, from the
//!   outside, exactly like a slightly-wrong per-gap constant. `#j2` is in the fixture because `#j1`
//!   alone cannot tell those apart.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.25 sans-serif}
.w{width:400px}
</style></head><body>
<div class="w" style="text-align:justify"><p id="pj">Alpha <span id="j1">bravo</span> charlie delta echo <span id="j2">foxtrot</span> golf hotel india juliet kilo lima mike november.</p></div>
<div class="w"><p id="pl">Alpha <span id="l1">bravo</span> charlie delta echo <span id="l2">foxtrot</span> golf hotel india juliet kilo lima mike november.</p></div>
<div class="w" style="text-align:justify"><p id="ps">Short <span id="s1">line</span> only.</p></div>
<div class="w" style="text-align:justify"><p id="pb">Alpha <span id="b1">bravo</span> charlie.<br>Second <span id="b2">line</span> here.</p></div>
<div class="w" style="text-align:justify"><p id="pw"><span id="w1">Supercalifragilisticexpialidociousandmoreletters</span></p></div>
<div class="w" style="text-align:center"><p id="pc"><span id="c1">centred still</span></p></div>
<div class="w" style="text-align:right"><p id="pr"><span id="r1">right still</span></p></div>
<div class="w" style="text-align:justify"><p id="pi">Alpha <span id="i1" style="display:inline-block;width:40px;height:10px"></span> charlie delta echo foxtrot golf hotel india juliet kilo lima.</p></div>
</body></html>"##;

fn x_of(page: &manuk_page::Page, sel: &str) -> f32 {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    page.root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel}"))
        .x
}

fn assert_x(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let got = x_of(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_TEXT_ALIGN_JUSTIFY: `{sel}` expected x {want} (MEASURED in headless Chrome on THIS \
         fixture), got {got}.\n  {why}"
    );
}

#[test]
fn g_text_align_justify() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://just.test/", &fonts, 1200.0);

    // ── THE BUG: the slack is distributed, not offset. TWO words, because one cannot distinguish a
    //    wrong per-gap amount from an expansion that stops accumulating.
    assert_x(
        &page,
        "#j1",
        49.0,
        "the 2nd word of a justified line — 45 unjustified, so one gap's expansion is 4px",
    );
    assert_x(
        &page,
        "#j2",
        237.0,
        "the 6th word of the SAME line: 220 unjustified plus five gaps. If the shift stops \
         accumulating this reads 227, which is why one word is not enough to gate this",
    );
    assert_x(
        &page,
        "#i1",
        49.0,
        "an inline-block flows as a word and moves with the expansion — it is positioned from the \
         same `LineFrag.x`, and a fix that only touched text fragments would leave it at 45",
    );

    // ── THE CONTROL: the same markup, unjustified. These are the numbers the DEFECT produced, so if
    //    they ever move, the fix has started applying where it must not.
    assert_x(
        &page,
        "#l1",
        45.0,
        "no justify: the 2nd word is where it always was",
    );
    assert_x(&page, "#l2", 220.0, "no justify: the 6th word likewise");

    // ── CSS Text §7.3 — the lines that are NOT justified. Each kills a different wrong version.
    assert_x(
        &page,
        "#s1",
        43.0,
        "the LAST line of a justified block takes `text-align-last` (start), not justify. A fix \
         written from the property name alone stretches this three-word line across 400px",
    );
    assert_x(
        &page,
        "#b1",
        45.0,
        "a line ended by a FORCED break <br> is also not justified — the same exception, and the \
         one that needs a separate call site to express",
    );
    assert_x(
        &page,
        "#b2",
        59.0,
        "…and the line AFTER the <br> is this block's last line, so it is not justified either",
    );
    assert_x(
        &page,
        "#w1",
        0.0,
        "one unbreakable word: no gaps to expand, so nothing moves. A division by the gap count \
         would panic or produce NaN here",
    );

    // ── THE OTHER ALIGNMENTS, in the same fixture, because they share the code path.
    assert_x(&page, "#c1", 160.0, "text-align:center is untouched");
    assert_x(&page, "#r1", 341.0, "text-align:right is untouched");
}
