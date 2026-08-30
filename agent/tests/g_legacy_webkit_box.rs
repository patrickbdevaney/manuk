//! **G_LEGACY_WEBKIT_BOX — two rows of surface audit #79's drift table, priced properly and
//! refuted; the behaviour they were about, banked.**
//!
//! Audit #79 ranked `MinimalCascade`'s missing properties by **declaration count** across 14 corpus
//! stylesheets and called two of them *"REAL LAYOUT"* drift:
//!
//! ```text
//!   -webkit-box-flex     63 declarations
//!   -ms-flex             63 declarations
//!   -webkit-box-orient   60 declarations
//! ```
//!
//! Both "real layout" rows evaporate under measurement, in two different ways.
//!
//! ## ⭐⭐⭐ `-ms-flex` — CHROME IGNORES IT, SO IMPLEMENTING IT WOULD MAKE US DIVERGE
//!
//! Measured: two children with `-ms-flex: 1` and `-ms-flex: 2` in a 300px flex container are **10px
//! each** in Chrome — their content width. `-ms-flex` is IE's spelling and no engine but IE honours
//! it. 63 declarations of a property whose correct implementation is *do nothing*.
//!
//! > **A property the corpus declares is not evidence the browser honours it.** That is the same
//! > shape as t1369's *"an unflipped pref is not evidence that a feature is broken"* — a count is a
//! > place to look, and the measurement is the finding.
//!
//! ## ⭐⭐ `-webkit-box-flex` — 63 declarations, and ONE site
//!
//! `display: -webkit-box` with `-webkit-box-orient: horizontal` laying children in a row is
//! **deliberately not implemented**, and `engine/css/src/lib.rs` says why: *"the dominant idiom is
//! text-only or `orient: vertical`."* Audit #79's declaration count looked like evidence against
//! that. It is not:
//!
//! ```text
//!   sites declaring -webkit-box-flex at all                    1 of 14
//!   …of which also declare -webkit-box-orient: vertical        1        (the IMPLEMENTED case)
//!   corpus-wide orient values      vertical 40 · horizontal 19 · initial 1
//! ```
//!
//! **The deferral's stated reason holds.** 63 declarations collapse to one site, and that site uses
//! the orientation that works. The row is withdrawn rather than built.
//!
//! ⚠ **A DECLARATION COUNT IS NOT A SITE COUNT, AND A SITE COUNT IS NOT A DIVERGENCE.** Audit #79
//! already learned half of this — it re-sorted its table by *what a property does* after finding
//! `transition` at the top — and this is the other half: sort by what it does, then price it by
//! SITE, then measure whether the browser even honours it.
//!
//! ## What is banked here
//!
//! The two behaviours those rows were about are correct and were ungated. Both are Chrome-measured:
//!
//! ```text
//!   -ms-flex: 1 / 2 in a 300px flex container      10px each   — IGNORED, and must stay ignored
//!   display:-webkit-box; orient:vertical           two 20px children stack to 40   — the idiom
//!                                                                                   authors use
//! ```
//!
//! PROVEN RED by two mutations — see the module tail.

use manuk_dom::NodeId;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0} body{font-family:monospace;font-size:16px}
.f{display:flex;width:300px;height:40px} .f > div{height:40px}
#c{-ms-flex:1} #d{-ms-flex:2}
.v{display:-webkit-box;-webkit-box-orient:vertical;width:300px}
.v > div{height:20px}
</style></head><body>
<div class="f"><div id="c">C</div><div id="d">D</div></div>
<div class="v" id="vv"><div>1</div><div>2</div></div>
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
fn ms_flex_is_ignored_and_a_vertical_webkit_box_stacks() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://lwb.test/", &fonts, 1200.0);
    let near = |g: f32, w: f32| (g - w).abs() < 2.0;

    // ── VACUITY. The flex container must actually be 300px and its children must actually be IN
    // it, or "they did not grow" is a statement about a layout that never happened.
    {
        let f = rect(&page, "c");
        let d = rect(&page, "d");
        assert!(
            near(f.y, d.y) && d.x > f.x,
            "VACUOUS: the two children are not laid out side by side ({f:?} / {d:?}), so this is \
             not a flex row and `-ms-flex` had nothing to grow"
        );
    }

    // ── ARM 1 · PINNED NEGATIVE — `-ms-flex` is IGNORED. Chrome gives each child its content width
    //    (10px), not 100/200. Implementing this property would make us DIVERGE.
    for id in ["c", "d"] {
        let w = rect(&page, id).width;
        assert!(
            near(w, 10.0),
            "G_LEGACY_WEBKIT_BOX #{id}: Chrome ignores `-ms-flex` — the child keeps its content \
             width of 10px, not a grown share of 300. Got {w}. This row exists so that a future \
             tick reading `-ms-flex: 63 declarations` off a drift table does not implement it."
        );
    }

    // ── ARM 2 · `display:-webkit-box` with `orient: vertical` STACKS — the dominant legacy idiom
    //    (40 of 60 orient declarations in the sampled corpus) and the case that is implemented.
    let h = rect(&page, "vv").height;
    assert!(
        near(h, 40.0),
        "G_LEGACY_WEBKIT_BOX: a vertical `-webkit-box` with two 20px children is 40 tall in Chrome, \
         got {h}"
    );
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  honour `-ms-flex` as `flex-grow` ("it is 63 declarations, surely it does something")
//       -> ARM 1 reads 100 and 200. This is the mutation the row exists to refuse, and it is the
//          only gate in the tree that would catch that mistake.
// N2  make `display:-webkit-box` an ordinary inline/blockless box so the vertical children do not
//     stack -> ARM 2 reads a height other than 40.
//
// ⚠ There is NO mutation here for the horizontal `-webkit-box` row, because it is not implemented
// and is deliberately not being implemented — see the module header for the price that decides it.
// Asserting Chrome's answer for that case would land a RED gate; asserting ours would pin a bug.
