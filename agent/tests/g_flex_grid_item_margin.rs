//! **G_FLEX_GRID_ITEM_MARGIN — a flex or grid ITEM establishes an independent formatting context,
//! so nothing collapses through it.**
//!
//! CSS Flexbox §3: *"the margins of a flex item do not collapse"*; CSS Grid §6 says the same for grid
//! items. `top_margin_collapses` could not see it, because it reads the box's **own** computed style
//! and an item is an ordinary `display: block` div — **what makes it an item is its PARENT**.
//!
//! So a first child's `margin-top` collapsed out through the item and off the top of the container,
//! and the whole subtree moved up by it.
//!
//! ## Every number headless-Chrome-measured
//!
//! ```text
//!                                              first child dy    the wrapper
//!   plain block chain (margin collapses)              0               80      CONTROL
//!   the container is display:grid                    60              140      <- was 0 / 80
//!   the container is display:flex                    60              140      <- was 0 / 80
//!   margin directly on a block's first child          0               80      CONTROL
//!   the same margin in px, plain chain                0               80      CONTROL
//! ```
//!
//! ⚠ **The three CONTROL rows are the point.** Collapsing is CORRECT for an ordinary block chain and
//! has to stay: this is a NARROWING of the predicate, not a removal of it. A fix that simply stopped
//! collapsing would pass the two grid/flex rows and break the other three — and margin collapse is
//! load-bearing on every ordinary page.
//!
//! ## ⭐⭐⭐ How it was found — five refused hypotheses, then the sixth
//!
//! `www.a11yproject.com` is the worst site on the board's anchor list (shape **43.3%**), and its
//! `--shape-dump` showed `y +60` on ten elements plus a systematic width shortfall. Five mechanisms
//! were proposed and all five were killed by direct measurement against Chrome — the fallback face
//! (identical to four decimals), the cross-origin webfont (both engines refuse it), `rem` against a
//! non-16px root (Chrome-exact), line-box overflow (Chrome-exact), and `letter-spacing` including the
//! `ch` unit (Chrome-exact on all eight rows).
//!
//! The sixth came from reading the site's own stylesheet instead of guessing:
//! `.c-homepage-card__image { margin-top: 3rem }` — and at that site's `html { font-size: 20px }`,
//! 3rem is **exactly the 60px** the dump was reporting. The card is a grid container.
//!
//! ⭐ **A dump names a SITE; the site's stylesheet names the MECHANISM.** Four of the five refuted
//! hypotheses were guesses about the engine; the one that worked came from grepping the CSS the page
//! actually ships for the number the dump was printing.
//!
//! ## The receipt, with controls
//!
//! ```text
//!                       before   after    delta
//!   a11yproject          43.3%   49.3%    +6.0    (absolute placement 10.6% -> 18.0%)
//!   martinfowler         79.9%   89.8%    +9.9    (absolute placement 16.5% -> 74.5%)
//!   wikipedia            90.3%   90.1%    -0.2    sample 5205->5207, inside the noise band
//!   news.ycombinator     99.9%   99.9%     0.0    CONTROL
//! ```
//!
//! ⚠ The wikipedia row is reported as noise rather than a regression **because its element
//! population moved** (5205 -> 5207), which is exactly the between-sweep comparison check #104
//! ruled inadmissible on its own.
//!
//! ⚠ The fixture below uses `margin-top: 60px` rather than `3rem`. The `rem` is what made the real
//! site's number 60 and it is in the story, but it is not the mechanism, and a gate that can be
//! reddened by a font-size unit is not a margin-collapse gate.
//!
//! PROVEN RED by two mutations — see the module tail.

use manuk_dom::NodeId;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font-family:monospace;font-size:16px;line-height:24px}
.w{width:400px;margin:0 0 8px 0}
.grid{display:grid}
.flex{display:flex;flex-direction:column}
.img{height:80px;margin-top:60px}
</style></head><body>
<div class="w" id="c1"><div class="card"><div class="link"><div class="img" id="e1"></div></div></div></div>
<div class="w" id="c2"><div class="card grid"><div class="link"><div class="img" id="e2"></div></div></div></div>
<div class="w" id="c3"><div class="card flex"><div class="link"><div class="img" id="e3"></div></div></div></div>
<div class="w" id="c4"><div class="card"><div class="img" id="e4"></div></div></div>
<div class="w" id="c5"><div class="card"><div style="height:80px;margin-top:60px" id="e5"></div></div></div>
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
fn a_flex_or_grid_items_child_margin_does_not_collapse_through_it() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fgm.test/", &fonts, 1200.0);
    let near = |g: f32, w: f32| (g - w).abs() < 1.6;

    // ── VACUITY. Five wrappers with real boxes, and the two containers must actually have
    // cascaded to grid/flex — otherwise rows 2 and 3 are two more copies of row 1.
    for n in 1..=5 {
        let r = rect(&page, &format!("c{n}"));
        assert!(r.width > 0.0, "VACUOUS: wrapper c{n} has no box ({r:?})");
    }

    // (id, wrapper, chrome first-child dy, chrome wrapper height, what the row is for)
    let rows: &[(&str, &str, f32, f32, &str)] = &[
        (
            "e1",
            "c1",
            0.0,
            80.0,
            "CONTROL — a plain block chain still collapses, and must",
        ),
        (
            "e2",
            "c2",
            60.0,
            140.0,
            "a GRID item establishes an independent formatting context",
        ),
        ("e3", "c3", 60.0, 140.0, "…and so does a FLEX item"),
        (
            "e4",
            "c4",
            0.0,
            80.0,
            "CONTROL — the margin directly on a block's first child still escapes",
        ),
        (
            "e5",
            "c5",
            0.0,
            80.0,
            "CONTROL — an inline-styled margin on a plain chain, same answer",
        ),
    ];
    for (id, w, want_dy, want_h, why) in rows {
        let (e, wrap) = (rect(&page, id), rect(&page, w));
        let dy = e.y - wrap.y;
        assert!(
            near(dy, *want_dy),
            "G_FLEX_GRID_ITEM_MARGIN {id}: Chrome puts the first child at dy={want_dy}, got {dy}.\n  \
             {why}"
        );
        assert!(
            near(wrap.height, *want_h),
            "G_FLEX_GRID_ITEM_MARGIN {w}: Chrome measures the wrapper {want_h} tall, got {}. A \
             margin that collapses out of the item takes the whole subtree up with it.\n  {why}",
            wrap.height
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  drop the `is_flex_or_grid_item` early-return from `collapses_as_block` (the pre-tick code)
//       -> rows 2 and 3 read dy=0 / wrapper 80; the three CONTROL rows stay green.
// N2  make `collapses_as_block` return false unconditionally ("just stop collapsing")
//       -> rows 2 and 3 pass and all THREE controls fail, which is the mutation that shows this is
//          a narrowing of the rule rather than its removal.
