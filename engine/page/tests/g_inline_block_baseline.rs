//! # G_INLINE_BLOCK_BASELINE — a text-bearing `inline-block` sits on ITS OWN baseline
//!
//! CSS 2.1 §10.8.1: the baseline of an `inline-block` is **the baseline of its last in-flow line
//! box** — unless it has no in-flow line boxes, or its `overflow` computes to something other than
//! `visible`, in which case it is the bottom margin edge.
//!
//! We implemented only the fallback. So every `inline-block` that contains text sat entirely ABOVE
//! the line's baseline, and its line box grew by the whole strut descent — about 4px, on every row of
//! chips, nav items, badges, tags, buttons and inline lists on the modern web, compounding down the
//! page as `dy`.
//!
//! ## Measured (`--dump-dom` + `getBoundingClientRect`), container heights
//!
//! ```text
//!                                                        Chrome    ours (before)
//!   <span style="display:inline-block">Ay</span>Ay        19.19        23
//!   …the same with padding:5px                            29.19        33
//!   …the same with overflow:hidden                        23.38        23   ✓ already right
//!   an EMPTY inline-block, + text                         19.19        19   ✓ already right
//!   a 20×20 empty inline-block, + text                    24.19        24   ✓ already right
//! ```
//!
//! **The three rows that already matched are the fallback cases, and that is exactly why this
//! survived 690 ticks**: the rule we implemented is a real rule — it was simply applied to every box
//! instead of to the two kinds it belongs to. A gate built only from the failing case would not have
//! noticed if a fix broke them, so all five are asserted here.
//!
//! ## What it was worth
//!
//! `blog.rust-lang.org` — this loop's control site, byte-identical through six previous fixes —
//! went shape **73.7% → 99.3%** on 1664 scored elements. `chat.google.com` went **72.9% → 84.7%** and
//! crossed the 0.75 bar; `255md.com` 69.8 → 72.1; `en.wikipedia.org` 58.8 → 60.4.
//!
//! ## How this goes RED
//!
//! - **Restore the bottom-margin-edge rule for every box** (`atomic_baseline = height`) → `c1` and
//!   `c5` fail; the three fallback rows stay green.
//! - **Apply the last-line rule to `overflow:hidden` too** → only `c4` fails.
//! - **Change the line-box contribution without the placement, or vice versa** → the box is placed
//!   outside the line box it asked for, and the container height and the child's `y` disagree.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.2 sans-serif}
.ib{display:inline-block}
.ov{display:inline-block;overflow:hidden}
</style></head><body>
<div id="c1"><span class="ib" id="i1">Ay</span>Ay</div>
<div id="c2"><span class="ib" id="i2"></span>Ay</div>
<div id="c3"><span class="ib" id="i3" style="width:20px;height:20px"></span>Ay</div>
<div id="c4"><span class="ov" id="i4">Ay</span>Ay</div>
<div id="c5"><span class="ib" id="i5" style="padding:5px">Ay</span>Ay</div>
</body></html>"##;

fn rect_of(page: &manuk_page::Page, sel: &str) -> (f32, f32) {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let r = page
        .root_box
        .node_rects(dom)
        .get(&n)
        .copied()
        .unwrap_or_else(|| panic!("no box for {sel}"));
    (r.y, r.height)
}

fn assert_h(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let (_, got) = rect_of(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_INLINE_BLOCK_BASELINE: `{sel}` expected height {want} (MEASURED in Chrome), got {got}.\n  {why}"
    );
}

#[test]
fn g_inline_block_baseline() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://baseline.test/", &fonts, 1200.0);

    // ── THE RULE: an inline-block WITH text aligns its last line's baseline with the parent's, so
    // its own descent overlaps the strut's instead of stacking on top of it.
    assert_h(
        &page,
        "#c1",
        19.19,
        "a text-bearing inline-block beside text — the line is ONE line tall, not one line plus the \
         strut's descent. This read 23 before t795, on every chip/nav/badge row on the web",
    );
    assert_h(
        &page,
        "#c5",
        29.19,
        "…and with padding, which moves the box's own baseline down by the padding but does not \
         change which baseline is used",
    );

    // ── THE FALLBACK, three ways. These were already correct, and a fix that "simplified" the rule
    // would break them silently.
    assert_h(
        &page,
        "#c2",
        19.19,
        "an EMPTY inline-block has no in-flow line box, so §10.8.1's fallback applies and its bottom \
         margin edge IS its baseline",
    );
    assert_h(
        &page,
        "#c3",
        24.19,
        "…the same for a sized empty inline-block: 20px above the baseline plus the strut's descent",
    );
    assert_h(
        &page,
        "#c4",
        23.38,
        "`overflow: hidden` takes the fallback too, even though the box is full of text — and its \
         answer genuinely DIFFERS from #c1's, which is what makes this a real distinction rather \
         than a special case",
    );

    // ── The two halves must agree. The line box reserves `(baseline, height - baseline)` about the
    // baseline and the placement puts the box at `baseline - baseline`; if those ever disagree the
    // box hangs outside the line it asked for, so the child's top is asserted against the parent's.
    let (cy, _) = rect_of(&page, "#c1");
    let (iy, ih) = rect_of(&page, "#i1");
    assert!(
        (iy - cy).abs() < 1.01 && ih > 0.0,
        "G_INLINE_BLOCK_BASELINE: the inline-block's top ({iy}) must sit at its line box's top \
         ({cy}) — the line-box contribution and the placement are inverses of each other, and a box \
         placed outside the line it reserved is how this fix goes subtly wrong"
    );
}
