//! **G_FLOAT_LINE_REFLOW — a float written into a line RE-FLOWS that line, in both directions.**
//!
//! `G_VI2_RESIDUALS`'s header carried one row as *NAMED, MEASURED, NOT BUILT*: the `c5` wrapper,
//! whose height Chrome measures **24** and which we produced **48**. This gate is that row built,
//! and the nine neighbouring shapes measured beside it so the fix is a rule rather than one number.
//!
//! ## THE MECHANISM
//!
//! `layout_block`'s float arm used to **commit** the pending inline run before placing the float.
//! t1002 had already fixed *where the float goes* (CSS 2.1 §9.5 rule 6 — the top of the line it was
//! written into, not below the whole run). The other half is that committing the flush freezes the
//! line: the text before the float keeps the x positions it was handed when nothing was in the way,
//! and the text *after* the float has no line left to join, so it opens a new one. Chrome re-flows
//! the line in both directions, and the fix is to lay the run out as a **trial** whose boxes are
//! discarded — the run keeps its nodes, `cur_y` does not advance, and the one real flush happens at
//! the end with this float already in the context.
//!
//! ⚠⚠ **THE REWIND IS UNCONDITIONAL, AND `c2` IS WHY.** A 380px float in a 400px block cannot join
//! the line, so Chrome drops it to `y=24` — and still keeps `yyyy` on the FIRST line. *A float that
//! cannot join a line does not break it either.* Rewinding only when the float joins would have
//! fixed `c1` and left `c2` at 48.
//!
//! ## THE BATTERY — ten shapes, every number headless-Chrome-measured (`--headless=new`,
//! 400px block, `monospace 16px/24px`, each case isolated behind a `clear:both` so no earlier
//! float overhangs the next wrapper)
//!
//! ```text
//!                                             wrapper h    float rect     the text
//!  c1  text, then a float that FITS              24       [  0  0 80x20]  before → x=80
//!  c2  text, then a 380px float that does NOT    24       [  0 24 380x20] stays at x=0, yyyy joins
//!  c3  CONTROL — float FIRST, then text          24       [  0  0 80x20]  x=80
//!  c4  text wrapping to TWO lines, then float    48       [  0 24 80x20]  line 1 untouched
//!  c5  a <p> block, then a float, then text      48       [  0 24 80x20]  yyyy → x=80
//!  c6  text<br>second, then a float              48       [  0 24 80x20]  `second` → x=80
//!  c7  text, then a RIGHT float                  24       [320  0 80x20]  unmoved (it fits)
//!  c8  text, then TWO floats                     24       [0 0] + [80 0]  x=160
//!  c9  text, a 60px-tall float, wrapping text    48       [  0  0 80x60]  BOTH lines indented
//! c10  `xxxx ` float ` yyyy` (spaces both sides) 24       [  0  0 80x20]  ONE space survives
//! ```
//!
//! ⭐ **`c10` is the row that says the run must not be SPLIT, only deferred.** Chrome puts `yyyy` at
//! x=128.16 — that is `80 + 38.53 + 9.63`, i.e. exactly one space between the two words even though
//! a float sits between them in source order. Collecting the run in two pieces around the float
//! would drop the space (each piece is `first` again); deferring one collection keeps it.
//!
//! ⚠ Chrome's `getClientRects().y` for these lines reads **2** where this gate asserts `line_top`
//! **0** — 24px of line-height over a 20px content area is 2px of half-leading above. The offset is
//! constant across every row, which is what makes it a coordinate convention rather than a defect.
//!
//! ⚠ This gate lives in `agent/tests/` for the reason surface audit #78 measured: `manuk-page` is in
//! neither the wall's nor CI's crate list, so a gate written there is executed by no runner.

use manuk_dom::NodeId;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font-family:monospace;font-size:16px;line-height:24px}
*{box-sizing:border-box}
.w{width:400px;margin:0 0 8px 0;position:relative}
.f{float:left;width:80px;height:20px;background:#333}
.clr{clear:both}
</style></head><body>
<div class="w" id="c1">xxxx xxxx xxxx<div class="f" id="e1"></div>yyyy</div>
<div class="clr"></div>
<div class="w" id="c2">xxxx xxxx xxxx<div class="f" id="e2" style="width:380px"></div>yyyy</div>
<div class="clr"></div>
<div class="w" id="c3"><div class="f" id="e3"></div>xxxx xxxx xxxxyyyy</div>
<div class="clr"></div>
<div class="w" id="c4">xxxx xxxx xxxx xxxx xxxx xxxx xxxx xxxx xxxx xxxx xxxx xxxx<div class="f" id="e4"></div>yyyy</div>
<div class="clr"></div>
<div class="w" id="c5"><p style="margin:0">para</p><div class="f" id="e5"></div>yyyy</div>
<div class="clr"></div>
<div class="w" id="c6">first<br>second<div class="f" id="e6"></div>yyyy</div>
<div class="clr"></div>
<div class="w" id="c7">xxxx<div class="f" id="e7" style="float:right"></div>yyyy</div>
<div class="clr"></div>
<div class="w" id="c8">xxxx<div class="f" id="e8"></div><div class="f" id="e8b" style="background:#777"></div>yyyy</div>
<div class="clr"></div>
<div class="w" id="c9">xx<div class="f" id="e9" style="height:60px"></div>yyyy yyyy yyyy yyyy yyyy yyyy yyyy yyyy</div>
<div class="clr"></div>
<div class="w" id="c10">xxxx <div class="f" id="e10"></div> yyyy</div>
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

/// Every non-blank text fragment whose owning element is `wrapper` or a descendant of it, as
/// `(text, x, line_top)` **relative to the wrapper**, in reading order.
///
/// ⚠ Selected by OWNERSHIP, not by geometry. A float overhangs a non-BFC parent — `c9`'s is 60px
/// tall in a 48px block — so a y-range filter would pull the next case's text into this one's list
/// and the gate would assert on whichever rows happened to overlap.
fn frags(page: &manuk_page::Page, wrapper: &str) -> Vec<(String, f32, f32)> {
    let dom = page.dom();
    let w = by_id(page, wrapper);
    let wr = *page
        .root_box
        .node_rects(dom)
        .get(&w)
        .unwrap_or_else(|| panic!("VACUOUS: no box for wrapper {wrapper}"));
    let own: std::collections::HashSet<NodeId> = dom.descendants(w).collect();
    let mut out: Vec<(String, f32, f32)> = Vec::new();
    page.root_box.walk(&mut |b| {
        if let manuk_layout::BoxContent::Inline(fs) = &b.content {
            for f in fs {
                if f.text.trim().is_empty() {
                    continue;
                }
                let owner = f.node.or(f.origin);
                if owner.is_some_and(|n| n == w || own.contains(&n)) {
                    out.push((f.text.trim().to_string(), f.x - wr.x, f.line_top - wr.y));
                }
            }
        }
    });
    out.sort_by(|a, b| (a.2, a.1).partial_cmp(&(b.2, b.1)).unwrap());
    out
}

#[test]
fn a_float_reflows_the_line_it_joins() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://float.test/", &fonts, 1200.0);
    let rects = page.root_box.node_rects(page.dom());
    let near = |g: f32, w: f32| (g - w).abs() < 1.6;

    // ── (wrapper, chrome height) — the number the whole tick is about. `c1`'s 24 is the row
    //    `G_VI2_RESIDUALS` carried unasserted; `c2`'s is the one that made the rewind unconditional.
    let heights: [(&str, f32); 10] = [
        ("c1", 24.0),
        ("c2", 24.0),
        ("c3", 24.0),
        ("c4", 48.0),
        ("c5", 48.0),
        ("c6", 48.0),
        ("c7", 24.0),
        ("c8", 24.0),
        ("c9", 48.0),
        ("c10", 24.0),
    ];
    for (w, want) in heights {
        let r = rects[&by_id(&page, w)];
        assert!(
            r.width > 0.0,
            "VACUOUS: wrapper {w} has no laid-out box at all ({r:?})"
        );
        assert!(
            near(r.height, want),
            "G_FLOAT_LINE_REFLOW {w}: Chrome measures {want} tall, got {}. A float written into a \
             line must not end it — neither the text before it nor the text after it starts a new \
             line box.",
            r.height
        );
    }

    // ── (float id, wrapper, chrome rect relative to the wrapper). §9.5 rule 6's half, which t1002
    //    landed and which must not regress while the re-flow half rides on the same call.
    let floats: [(&str, &str, (f32, f32, f32, f32)); 11] = [
        ("e1", "c1", (0.0, 0.0, 80.0, 20.0)),
        ("e2", "c2", (0.0, 24.0, 380.0, 20.0)),
        ("e3", "c3", (0.0, 0.0, 80.0, 20.0)),
        ("e4", "c4", (0.0, 24.0, 80.0, 20.0)),
        ("e5", "c5", (0.0, 24.0, 80.0, 20.0)),
        ("e6", "c6", (0.0, 24.0, 80.0, 20.0)),
        ("e7", "c7", (320.0, 0.0, 80.0, 20.0)),
        ("e8", "c8", (0.0, 0.0, 80.0, 20.0)),
        ("e8b", "c8", (80.0, 0.0, 80.0, 20.0)),
        ("e9", "c9", (0.0, 0.0, 80.0, 60.0)),
        ("e10", "c10", (0.0, 0.0, 80.0, 20.0)),
    ];
    for (id, w, want) in floats {
        let (a, b) = (rects[&by_id(&page, id)], rects[&by_id(&page, w)]);
        let got = (a.x - b.x, a.y - b.y, a.width, a.height);
        assert!(
            near(got.0, want.0)
                && near(got.1, want.1)
                && near(got.2, want.2)
                && near(got.3, want.3),
            "G_FLOAT_LINE_REFLOW float `{id}`: Chrome measures {want:?}, got {got:?}"
        );
    }

    // ── THE RE-FLOW ITSELF: where the line's own text ended up. `(wrapper, index, text, x, line_top)`
    //    — one or two probe fragments per case, chosen so each row can only pass for the right
    //    reason. Chrome's `getClientRects().y` is 2 higher throughout (half-leading); see header.
    let text: &[(&str, usize, &str, f32, f32, &str)] = &[
        ("c1", 0, "xxxx", 80.0, 0.0, "the text BEFORE the float moves right of it — the direction a committed flush can never produce"),
        ("c1", 3, "yyyy", 214.86, 0.0, "and the text AFTER it stays on the same line"),
        ("c2", 0, "xxxx", 0.0, 0.0, "a float that cannot fit beside the line leaves the line alone"),
        ("c2", 3, "yyyy", 134.86, 0.0, "…and still does not break it: `yyyy` joins line 1"),
        ("c3", 0, "xxxx", 80.0, 0.0, "CONTROL — a float written BEFORE the text was already correct and stays so"),
        ("c4", 0, "xxxx", 0.0, 0.0, "line 1 of a two-line run is ABOVE the float's band and must not be re-broken"),
        ("c4", 12, "yyyy", 263.02, 24.0, "only the LAST line is re-flowed, and the trailing text joins it"),
        ("c5", 1, "yyyy", 80.0, 24.0, "a float after a BLOCK child has no line to join, and the text after it wraps around"),
        ("c6", 1, "second", 80.0, 24.0, "`<br>` makes line 2 the last line, so that is the line the float joins"),
        ("c7", 0, "xxxx", 0.0, 0.0, "a RIGHT float takes the far edge; left-aligned text that still fits does not move"),
        ("c8", 0, "xxxx", 160.0, 0.0, "two floats on one line stack, and the text starts past BOTH"),
        ("c9", 1, "yyyy", 99.27, 0.0, "a 60px float indents every line it overlaps, not just the one it joined"),
        ("c9", 7, "yyyy", 80.0, 24.0, "…including the second one"),
        ("c10", 0, "xxxx", 80.0, 0.0, "white space on both sides of the float collapses to ONE space, which needs the run collected in ONE piece"),
        ("c10", 1, "yyyy", 128.16, 0.0, "80 + 38.53 + 9.63 — the surviving space, to the pixel"),
    ];
    for (w, i, want_t, x, y, why) in text {
        let fs = frags(&page, w);
        assert!(
            fs.len() > *i,
            "VACUOUS: {w} produced only {} text fragments, so row {i} asserts nothing",
            fs.len()
        );
        let (t, gx, gy) = &fs[*i];
        assert!(
            t == want_t && near(*gx, *x) && near(*gy, *y),
            "G_FLOAT_LINE_REFLOW {w} fragment {i}: Chrome puts {want_t:?} at x={x} line_top={y}, \
             got {t:?} at x={gx} line_top={gy}.\n  {why}\n  all fragments: {fs:?}"
        );
    }
}
