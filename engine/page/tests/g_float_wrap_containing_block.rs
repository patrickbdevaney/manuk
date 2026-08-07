//! **G_FLOAT_WRAP_CONTAINING_BLOCK — a float that no longer fits must drop below the floats already
//! there, and "fits" is measured against its OWN CONTAINING BLOCK, not against the BFC root.**
//!
//! Four 50px `float: left` boxes fill a 200px block. The fifth was asked *"do you fit in 1200?"* —
//! the body's width — and placed at **x = 200, outside its own container**. Then the sixth at 250,
//! the seventh at 300: **a float row that never wraps and walks off the right edge for as long as
//! the markup goes on.** A float row that overflows its container is not one wrong box; it is
//! overlap and reading-order violations across everything the row was supposed to sit above.
//!
//! ## Why it survived: a plain nested block is not a BFC, and almost nothing on the web is
//!
//! `FloatContext`'s `left_edge`/`right_edge` belong to the nearest **block formatting context**, and
//! floats correctly share one across nested plain blocks — that is what makes a float escape its
//! parent. But CSS 2.1 §9.5.1 rules 1 and 2 pin a float to *its own containing block*, and the fit
//! test was asking the context. The two only agree when the containing block **is** the BFC root,
//! which is the case a fixture reaches for first and the case the real web almost never is:
//! `<div class="sidebar"><div class="widget"><img class="alignleft">` is three plain blocks deep.
//!
//! Measured against `google-chrome-stable --headless=new --hide-scrollbars --window-size=1200,800`,
//! five 50px `float:left` boxes in a 200px block, **the one variable being whether the block is a
//! BFC** — `(x, y)` of the fifth float, `y` relative to its container:
//!
//! ```text
//!                                                    Chrome      before      after
//!   #a5  overflow: hidden   — a BFC                 [  0, 30]   [  0, 30]   [  0, 30]
//!   #b5  overflow: visible  — NOT a BFC             [  0, 30]   [200,  0]   [  0, 30]
//!   #c5  …one plain block deeper, still not         [  0, 30]   [200,  0]   [  0, 30]
//!   #d5  the same non-BFC block, RIGHT floats       [150, 30]   [150, 30]   [150, 30]
//! ```
//!
//! ⚠⚠⚠ **`#d5` IS THE ROW THAT NAMES THE BRANCH, AND WITHOUT IT THE DIAGNOSIS IS WRONG.** Right
//! floats wrapped correctly in the very same non-BFC container the whole time. So this is not *"we
//! do not wrap floats"* and not *"a non-BFC block loses its width"* — both of which fit `#b5`/`#c5`
//! perfectly and would have sent the fix at the wrong function. A right float is placed at
//! `cb_right - w`, which lands it **inside** the container, so `right_offset` picks it up and
//! collapses the available width to zero on its own: the containing block's edge reaches the test
//! *by accident*. A left float has nothing on its right to bound it, so the context's far edge
//! stands in — and it was 1000px too generous.
//!
//! ⚠⚠ **`#a5` is the control that makes the claim narrow.** It differs from `#b5` by one
//! declaration. If it ever moves, the change stopped being about the containing block and became
//! about float wrapping in general.
//!
//! ## RED proof (run, not imagined)
//!
//! Restoring `let full = self.right_edge - self.left_edge;` and the `self.available(y, h)` fit test
//! gives **`#b5 x=200, Chrome gives 0`**, with `#a5` and `#d5` still passing — which is exactly what
//! makes them controls rather than more of the same.
//!
//! ⚠⚠ **A NON-RED, RECORDED RATHER THAN CLAIMED.** I wrote in this header that using
//! `self.left_offset(y, h).max(cb_left)` for the left fit bound — folding the BFC edge back in, the
//! very thing t792 removed from the *placement* expression — would re-break the negative-margin
//! Bootstrap row. **Then I ran it, and the whole gate passes.** The fit test only decides *whether*
//! a float fits in this band; *where* it lands is a separate expression, so a 15px-conservative
//! bound changes nothing here. `cb_left` is used raw because it is consistent with the placement and
//! cannot disagree with it, **not** because any assertion below enforces it. The row that would
//! catch the difference is a negative-margin container whose float only fits by less than the
//! margin, and it is not written.

use manuk_text::FontContext;

/// `#wA` and `#wB` differ by ONE declaration. `#wD` is the same non-BFC container with right floats.
const HTML: &str = r##"<!doctype html><html><head><style>
 body{margin:0;font:16px/20px monospace;width:1200px}
 .box{width:200px;margin-bottom:8px;background:#eee}
 .f{float:left;width:50px;height:30px;background:#c00}
 .r{float:right;width:50px;height:30px;background:#00c}
</style></head><body>
<div class="box" id="wA" style="overflow:hidden"><div class="f" id="a1"></div><div class="f" id="a2"></div><div class="f" id="a3"></div><div class="f" id="a4"></div><div class="f" id="a5"></div></div>
<div class="box" id="wB" style="overflow:visible"><div class="f" id="b1"></div><div class="f" id="b2"></div><div class="f" id="b3"></div><div class="f" id="b4"></div><div class="f" id="b5"></div></div>
<div style="clear:both"></div>
<div class="box" id="wC" style="overflow:visible"><div id="cin"><div class="f" id="c1"></div><div class="f" id="c2"></div><div class="f" id="c3"></div><div class="f" id="c4"></div><div class="f" id="c5"></div></div></div>
<div style="clear:both"></div>
<div class="box" id="wD" style="overflow:visible"><div class="r" id="d1"></div><div class="r" id="d2"></div><div class="r" id="d3"></div><div class="r" id="d4"></div><div class="r" id="d5"></div></div>
<div style="clear:both"></div>
<!-- a float WIDER than its containing block must still be placed, not dropped for ever -->
<div class="box" id="wE" style="overflow:hidden"><div id="e1" style="float:left;width:300px;height:30px;background:#c00"></div></div>
<!-- the Bootstrap grid: a negative-margin row starts OUTSIDE the BFC's content edge -->
<div style="width:400px;overflow:hidden" id="wF"><div id="frow" style="margin:0 -15px"><div id="f1x" style="float:left;width:50px;height:30px;background:#c00"></div></div></div>
</body></html>"##;

fn rect(page: &manuk_page::Page, id: &str) -> (i64, i64, i64, i64) {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), &format!("#{id}"))
        .first()
        .copied()
        .unwrap_or_else(|| panic!("#{id} matched nothing"));
    let b = *page
        .root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("#{id} has no box — it was not laid out at all"));
    (
        b.x.round() as i64,
        b.y.round() as i64,
        b.width.round() as i64,
        b.height.round() as i64,
    )
}

/// `(x, y)` with `y` relative to `container` — the containers are stacked, so an absolute `y` would
/// encode the fixture's ordering rather than the claim.
fn at(page: &manuk_page::Page, id: &str, container: &str) -> (i64, i64) {
    let (x, y, _, _) = rect(page, id);
    (x, y - rect(page, container).1)
}

fn check(page: &manuk_page::Page, id: &str, container: &str, want: (i64, i64), why: &str) {
    let got = at(page, id, container);
    assert_eq!(
        got, want,
        "G_FLOAT_WRAP_CONTAINING_BLOCK: #{id} is at {got:?} within #{container}, Chrome gives \
         {want:?}.\n  {why}"
    );
}

#[test]
fn a_float_that_no_longer_fits_its_containing_block_drops_to_the_next_band() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://float.test/", &fonts, 1200.0);

    // ── 0. THE CONTROL. Exactly the same markup with `overflow: hidden`, so the container IS the
    //    BFC root and the two edges coincide. This row passed throughout; if it ever moves, the
    //    claim stopped being about the containing block.
    check(
        &page,
        "a4",
        "wA",
        (150, 0),
        "control: the fourth float fills the 200px BFC exactly",
    );
    check(
        &page,
        "a5",
        "wA",
        (0, 30),
        "control: the fifth drops to the next band. This is float wrapping WORKING, in the case \
         where the containing block and the BFC root are the same box",
    );

    // ── 1. THE DEFECT. One declaration different, and the container is no longer the BFC root.
    check(
        &page,
        "b4",
        "wB",
        (150, 0),
        "the fourth float still fills the 200px container — placement was never the problem",
    );
    check(
        &page,
        "b5",
        "wB",
        (0, 30),
        "the fifth must drop. It was placed at x=200 — OUTSIDE its own container — because the fit \
         test asked the BFC root (the 1200px body) whether 50px would fit",
    );

    // ── 2. AND IT IS NOT ABOUT DEPTH: one more plain block between them changes nothing.
    check(
        &page,
        "c5",
        "wC",
        (0, 30),
        "a plain block nested inside the non-BFC container — `<div class=sidebar><div \
         class=widget>` is the ordinary shape of the web, and it was the broken one",
    );

    // ── 3. THE ROW THAT NAMES THE BRANCH. Right floats in the SAME non-BFC container were correct
    //    all along, which rules out "floats never wrap" and "a non-BFC block loses its width".
    check(
        &page,
        "d4",
        "wD",
        (0, 0),
        "four right floats fill the same 200px non-BFC container, right to left",
    );
    check(
        &page,
        "d5",
        "wD",
        (150, 30),
        "…and the fifth ALREADY dropped correctly, because a right float is placed at `cb_right - \
         w` and therefore lands inside the container, so the right-side offset picks it up and \
         zeroes the available width on its own. The containing block's edge reached this test by \
         accident, and only on this side",
    );

    // ── 4. A FLOAT WIDER THAN ITS CONTAINING BLOCK IS PLACED, NOT DROPPED FOR EVER. The escape
    //    hatch is `avail >= full`, and `full` had to become the container's width with the rest —
    //    comparing a container-sized gap to a BFC-sized one makes the escape unreachable.
    check(
        &page,
        "e1",
        "wE",
        (0, 0),
        "a 300px float in a 200px block overflows to the right at x=0; it must not scan downward \
         looking for a band that cannot exist",
    );

    // ── 5. THE NEGATIVE-MARGIN ROW (the Bootstrap grid). The fit bounds use `cb_left` raw, because
    //    folding the context's edge back in is exactly what t792 removed from the placement.
    check(
        &page,
        "f1x",
        "wF",
        (-15, 0),
        "`.row { margin: 0 -15px }` starts OUTSIDE its BFC's content edge, and its floated column \
         starts there too — the t792 rule, still holding. ⚠ This row does NOT discriminate the fit \
         bound: a `left_offset`-based one passes it, because the fit test decides whether a float \
         fits and a separate expression decides where it goes",
    );
}
