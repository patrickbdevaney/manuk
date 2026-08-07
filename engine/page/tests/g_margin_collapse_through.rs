//! **G_MARGIN_COLLAPSE_THROUGH — an EMPTY block's own top and bottom margins collapse with EACH
//! OTHER (CSS 2.1 §8.3.1), so it contributes ONE margin to the flow, not two.**
//!
//! The parent↔first-child and parent↔last-child collapses have been built here for a long time. The
//! box that collapses **through itself** was not, so every empty block pushed everything after it
//! down by an extra margin. `<div class="clearfix"></div>` and every spacer/wrapper div is exactly
//! this shape, and the error is a pure `dy` that cascades over **the whole rest of the page** rather
//! than misplacing one box — which is the top-ranked shape mechanism in the render burndown.
//!
//! Every number below is from `google-chrome-stable --headless=new --hide-scrollbars
//! --window-size=1200,800`, and the readout is always the position of the block AFTER the candidate.
//!
//! ```text
//!   <div>x</div>  <div style="margin:10px 0 30px"></div>  <div>R</div>
//!
//!                                    Chrome   before   after
//!     the empty box's own edge         30       30       30      <- never moved
//!     the block after it               50       60       50
//! ```
//!
//! ## The two rows that force `max`, and why one ratio cannot
//!
//! ⚠⚠⚠ With `10px` over `30px` the answer is **30**, and the rule *"an empty box contributes only
//! its BOTTOM margin"* gets that right. With `40px` over `5px` the answer is **40**, and that same
//! rule gives 5. The mirror rule — *"only its TOP margin"* — fails the first and passes the second.
//! **Only the pair forces a collapse**, and a fixture carrying one ratio cannot tell the three
//! rules apart. Both ratios are asserted below for exactly that reason.
//!
//! ⚠⚠ And it is `collapse_margins`, not `max`: two negative margins take the **`min`**. `-10px` over
//! `-30px` measures **-30** in Chrome, where `max` gives -10 and the sum gives -40.
//!
//! ## The condition, clause by clause — 15 rows, each varying ONE thing
//!
//! ```text
//!                                                  collapses through?   ours
//!     plain empty block                                  YES             yes
//!     border: 1px                                        no              no
//!     padding-top: 1px                                   no              no
//!     height: 0        (explicit)                        YES             yes
//!     min-height: 1px                                    no              no
//!     overflow: hidden (a BFC)                           no              no
//!     display: flow-root (a BFC)                         no              no
//!     height: 20px                                       no              no
//!     has text content                                   no              no
//!     only child is a FLOAT                              YES             yes
//!     only child is position:absolute                    YES             yes
//!     only child is another EMPTY block                  YES             yes
//!     two empty SIBLINGS                                 YES             yes
//!     contains only whitespace text                      YES             yes
//!     clear: both, no floats present                     YES             yes
//! ```
//!
//! ## ⚠⚠⚠ The recursive clause, and the two shortcuts that both fail
//!
//! The spec's last clause is *"and **all of its in-flow children's margins (if any) collapse**"*.
//!
//! - **"no in-flow children"** — what I implemented first. Fails an empty block wrapping an empty
//!   block (Chrome collapses; the shortcut does not), the same three deep, and `height: 0` wrapping
//!   an empty block.
//! - **"contains no line box"** — what I reached for next. Fails a `height: 0` block wrapping a
//!   block *with text*: it contains no line box **itself**, but its child's margins do not collapse,
//!   so Chrome does **not** collapse it (60, where the shortcut gives 30).
//!
//! **The two shortcuts fail in opposite directions**, which is what makes the pair of rows below a
//! proof rather than a pair of examples: any rule that passes both is doing the recursion.
//!
//! ## RED proof (run, not imagined)
//!
//! Forcing `self_collapsing = false` in `layout_block` gives `#m3 y=36` against Chrome's `28` and
//! `#m5 y=72` against `64` on the original reduction. Every "no" row above keeps passing under that
//! revert, which is what makes them controls.

use manuk_text::FontContext;

/// Each group is `x` (a line), the CANDIDATE, then the readout `R`. Only the candidate varies, and
/// `R`'s offset within its group is the whole measurement.
const HTML: &str = r##"<!doctype html><html><head><style>
 body{margin:0;font:16px/20px monospace}
 .g{background:#eee;width:300px}
 .t{background:#cfc}
 .e{margin-top:10px;margin-bottom:30px;width:300px;background:#fcc}
</style></head><body>
<div class="g" id="g1">x<div class="e" id="e1"></div><div class="t" id="t1">R</div></div>
<div class="g" id="g2">x<div class="e" id="e2" style="border:1px solid #000"></div><div class="t" id="t2">R</div></div>
<div class="g" id="g3">x<div class="e" id="e3" style="padding-top:1px"></div><div class="t" id="t3">R</div></div>
<div class="g" id="g4">x<div class="e" id="e4" style="height:0"></div><div class="t" id="t4">R</div></div>
<div class="g" id="g5">x<div class="e" id="e5" style="min-height:1px"></div><div class="t" id="t5">R</div></div>
<div class="g" id="g6">x<div class="e" id="e6" style="overflow:hidden"></div><div class="t" id="t6">R</div></div>
<div class="g" id="g7">x<div class="e" id="e7" style="height:20px"></div><div class="t" id="t7">R</div></div>
<div class="g" id="g8">x<div class="e" id="e8">c</div><div class="t" id="t8">R</div></div>
<div class="g" id="g9">x<div class="e" id="e9"><div style="float:left;width:5px;height:5px"></div></div><div class="t" id="t9">R</div></div>
<div class="g" id="g10">x<div class="e" id="e10"><div style="position:absolute;width:5px;height:5px"></div></div><div class="t" id="t10">R</div></div>
<div class="g" id="g11">x<div class="e" id="e11"><div class="e"></div></div><div class="t" id="t11">R</div></div>
<div class="g" id="g12">x<div class="e" id="e12"></div><div class="e" id="e12b"></div><div class="t" id="t12">R</div></div>
<div class="g" id="g13">x<div class="e" id="e13" style="margin-top:40px;margin-bottom:5px"></div><div class="t" id="t13">R</div></div>
<div class="g" id="g14">x<div class="e" id="e14" style="display:flow-root"></div><div class="t" id="t14">R</div></div>
<div class="g" id="g15">x<div class="e" id="e15" style="clear:both"></div><div class="t" id="t15">R</div></div>
<div class="g" id="h1">x<div class="e" id="d1" style="height:0"><div>text</div></div><div class="t" id="u1">R</div></div>
<div class="g" id="h3">x<div class="e" id="d3"><div class="e"><div class="e"></div></div></div><div class="t" id="u3">R</div></div>
<div class="g" id="h5">x<div class="e" id="d5" style="height:0"><div class="e"></div></div><div class="t" id="u5">R</div></div>
<div class="g" id="h6">x<div class="e" id="d6">   </div><div class="t" id="u6">R</div></div>
<div class="g" id="n1">x<div id="p1" style="margin-top:-10px;margin-bottom:-30px"></div><div class="t" id="v1">R</div></div>
</body></html>"##;

fn y_of(page: &manuk_page::Page, id: &str) -> f32 {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), &format!("#{id}"))
        .first()
        .copied()
        .unwrap_or_else(|| panic!("#{id} matched nothing"));
    page.root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("#{id} has no box — it was not laid out at all"))
        .y
}

/// The readout's offset inside its own group — the groups are stacked, so an absolute `y` would
/// encode the fixture's ordering rather than the claim.
fn readout(page: &manuk_page::Page, group: &str, id: &str) -> i64 {
    (y_of(page, id) - y_of(page, group)).round() as i64
}

fn check(page: &manuk_page::Page, group: &str, id: &str, want: i64, why: &str) {
    let got = readout(page, group, id);
    assert_eq!(
        got, want,
        "G_MARGIN_COLLAPSE_THROUGH: #{id} sits {got}px into #{group}, Chrome gives {want}.\n  {why}"
    );
}

#[test]
fn an_empty_blocks_own_margins_collapse_with_each_other() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://margin.test/", &fonts, 1200.0);

    // ── 1. THE DEFECT, and the box's own position as a control on the fix's blast radius.
    check(
        &page,
        "g1",
        "e1",
        30,
        "the empty box's OWN edge must not move — 20px of text plus its 10px top margin. The fix \
         changes what comes AFTER an empty box, and if this row moves it changed the box too",
    );
    check(
        &page,
        "g1",
        "t1",
        50,
        "`margin: 10px 0 30px` on an empty block contributes ONE margin of 30, not 10 + 30. This \
         is the whole defect: it was 60",
    );

    // ── 2. THE TWO RATIOS. Neither alone can distinguish `max` from "only the bottom margin"
    //    (which passes #t1) or "only the top margin" (which passes #t13).
    check(
        &page,
        "g13",
        "t13",
        60,
        "`margin: 40px 0 5px` gives 40. \"Only the bottom margin\" — which passes #t1 exactly — \
         gives 5 here, and \"only the top margin\" passes this and fails #t1. Only the PAIR forces \
         a collapse",
    );
    check(
        &page,
        "n1",
        "v1",
        -10,
        "`margin: -10px 0 -30px` gives -30, so it is `collapse_margins` and not `max`: two \
         negatives take the MIN. `max` gives -10 and the sum gives -40",
    );

    // ── 3. THE CLAUSES THAT MUST *STOP* IT. Every one is a control: they passed before the fix and
    //    must pass after, and they are what keep it from being "empty divs are special".
    check(&page, "g2", "t2", 62, "a border stops it (border: 1px)");
    check(&page, "g3", "t3", 61, "padding stops it (padding-top: 1px)");
    check(&page, "g5", "t5", 61, "`min-height: 1px` stops it");
    check(
        &page,
        "g6",
        "t6",
        60,
        "`overflow: hidden` stops it — the box is still zero-height, so a height check alone would \
         wrongly collapse it. This is why the BFC predicates are consulted separately",
    );
    check(
        &page,
        "g14",
        "t14",
        60,
        "`display: flow-root` stops it — the other way to make a BFC, and the row that stops the \
         `overflow` clause from standing in for the concept",
    );
    check(&page, "g7", "t7", 80, "a real height stops it");
    check(
        &page,
        "g8",
        "t8",
        80,
        "text content stops it — the box has a line box, and it is 20px tall",
    );

    // ── 4. WHAT IS *NOT* AN IN-FLOW CHILD. A box holding only out-of-flow boxes is still empty.
    check(
        &page,
        "g9",
        "t9",
        50,
        "a `float: left` child does not stop it — a float is not in flow",
    );
    check(
        &page,
        "g10",
        "t10",
        50,
        "a `position: absolute` child does not stop it either",
    );
    check(
        &page,
        "h6",
        "u6",
        50,
        "whitespace-only text does not stop it — it generates no box. Pretty-printed HTML puts a \
         newline inside every \"empty\" element, so without this row the fix would be inert on \
         real markup",
    );

    // ── 5. THE RECURSIVE CLAUSE. These four rows are a proof and not four examples: the two
    //    plausible shortcuts fail in OPPOSITE directions, so any rule passing all four is
    //    recursing.
    check(
        &page,
        "g11",
        "t11",
        50,
        "an empty block wrapping an empty block collapses. \"No IN-FLOW CHILDREN\" — the first \
         shortcut — gets this wrong: the inner block IS an in-flow child",
    );
    check(&page, "h3", "u3", 50, "…and three deep, so it recurses");
    check(
        &page,
        "h5",
        "u5",
        50,
        "`height: 0` wrapping an empty block collapses too",
    );
    check(
        &page,
        "h1",
        "u1",
        60,
        "…but `height: 0` wrapping a block WITH TEXT does NOT. It contains no line box ITSELF, so \
         \"contains no line box\" — the second shortcut — collapses it and is wrong. Its child's \
         margins do not collapse, and that is the clause. This row and #t11 fail opposite \
         shortcuts, which is what makes the pair a proof",
    );

    // ── 6. SIBLINGS AND `clear` — the run joins, and `clear` is not special-cased.
    check(
        &page,
        "g12",
        "t12",
        50,
        "TWO empty siblings still contribute ONE margin between them all, not two or four",
    );
    check(
        &page,
        "g15",
        "t15",
        50,
        "`clear: both` with no float present collapses normally — the collapse must not be \
         special-cased on `clear`, which is exactly the shape of a clearfix div",
    );
}
