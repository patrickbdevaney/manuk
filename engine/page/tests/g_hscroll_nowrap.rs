//! # G_HSCROLL_NOWRAP — a horizontal row of `inline-block`s stays on ONE line, and scrolls
//!
//! `white-space: nowrap` around a row of `inline-block` children is how the pre-flexbox web — and a
//! great deal of the current web — builds **nav bars, tab strips, chip/filter rows, breadcrumb
//! trails, toolbars and image carousels**. The declaration says "this row is one line; let the
//! container scroll it".
//!
//! Tick 266 recorded this as a *scroll-geometry* gap ("an inline-block row yields NO horizontal
//! scroll range"). A probe (`probe_hscroll`) falsified that diagnosis: `display:flex` rows and wide
//! block children **already** produced a correct `scrollWidth`, and `nowrap` **already** worked for
//! plain text. The real defect was narrower and one line deep — `InlineItem::Atomic` was built with
//! `no_wrap` hardcoded `false`, so an atomic inline box never carried the `white-space` that every
//! `InlineItem::Word` beside it did. The line breaker's rule (`breakable = !(no_wrap &&
//! prev_no_wrap)`) therefore always found a break opportunity around an inline-block.
//!
//! The visible failure is not "the carousel doesn't scroll". It is that the row **silently wraps
//! into a stack**: five 100px tabs in a 200px bar become three rows, the bar grows to 3× its
//! declared height and pushes the page down, and `scrollWidth == clientWidth` so nothing scrolls
//! because — correctly, given the wrapped layout — there is nothing to scroll. The engine was
//! self-consistent and wrong, which is why a capability count could not see it.
//!
//! ## How each assertion here can go RED
//!
//! - **The row stays on one line.** RED, run: restore `false` in the `InlineItem::Atomic` arm of
//!   the line builder (`layout/src/lib.rs`). `scrollHeight` jumps 100 → 240 (three wrapped lines).
//!
//! - **The row is genuinely scrollable.** RED, same edit: `scrollWidth` collapses 500 → 200 and
//!   `maxX` → 0. This is the assertion that distinguishes "laid out wide" from "wrapped", and it is
//!   the one the tick-266 note was reaching for.
//!
//! - **`nowrap` still only applies WHEN DECLARED.** RED, run: make the atomic arm pass `true`
//!   unconditionally instead of reading `white-space`. The `#wrap` control below stops wrapping,
//!   and every ordinary `inline-block` gallery on the web becomes an infinite single line. A fix
//!   that cannot go red this way is not a fix, it is a blanket disable of inline-block wrapping.
//!
//! - **A horizontal scroll SNAPS.** RED, run: return `at` unchanged from `Page::snap_scroll`. The
//!   x-axis of snapping shipped in tick 266 but was **never exercised**, because no horizontal
//!   scroll range existed to exercise it with — untested code asserted to work by symmetry alone.

use manuk_text::FontContext;

/// Five 100px tabs in a 200px-wide bar. Round numbers: an assertion needing slop cannot tell a
/// one-line row from a wrapped one.
const HTML: &str = r#"<!doctype html><html><head><style>
  #bar {
    overflow-x: scroll; width: 200px; height: 100px;
    white-space: nowrap;
    scroll-snap-type: x mandatory;
  }
  #bar > div { display: inline-block; width: 100px; height: 80px; scroll-snap-align: start; }
  /* CONTROL: identical, but no `nowrap` — this one MUST still wrap. */
  #wrap { overflow-x: scroll; width: 200px; height: 100px; }
  #wrap > div { display: inline-block; width: 100px; height: 80px; }
</style></head><body>
  <div id="bar"><div>a</div><div>b</div><div>c</div><div>d</div><div>e</div></div>
  <div id="wrap"><div>a</div><div>b</div><div>c</div><div>d</div><div>e</div></div>
</body></html>"#;

fn ids(page: &manuk_page::Page, sel: &str) -> manuk_dom::NodeId {
    let root = page.dom().root();
    manuk_css::query_selector_all(page.dom(), root, sel)[0]
}

#[test]
fn g_hscroll_nowrap() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://bar.test/", &fonts, 800.0);
    let bar = ids(&page, "#bar");
    let wrap = ids(&page, "#wrap");

    let g = page
        .scroll_geometry(bar)
        .expect("#bar is a scroll container");
    let (scroll_w, scroll_h, client_w, client_h) = (g[3], g[2], g[5], g[4]);

    // ── The row is ONE line. Height is the tell: five 80px tabs wrapped 2-per-line stand 240 tall.
    assert_eq!(
        scroll_h, client_h,
        "a `white-space:nowrap` row of inline-blocks must stay on ONE line — scrollHeight {scroll_h} \
         vs clientHeight {client_h}. A taller scrollHeight means the row WRAPPED into a stack: the \
         nav bar silently becomes three rows and shoves the rest of the page down"
    );

    // ── It is genuinely wide, i.e. there is something to scroll. Distinguishes laid-out-wide from
    //    wrapped — a wrapped row is also self-consistent, just self-consistently wrong.
    assert_eq!(
        scroll_w, 500.0,
        "five 100px tabs on one line are 500px of content — got scrollWidth {scroll_w}. \
         At {client_w} the content exactly fits the bar, which is what WRAPPING looks like"
    );
    let max_x = scroll_w - client_w;
    assert_eq!(
        max_x, 300.0,
        "300px of the row must be off-screen and reachable"
    );

    // ── CONTROL: without `nowrap`, the identical row MUST still wrap. This is what separates
    //    "honours white-space" from "never breaks inline-blocks".
    let gw = page
        .scroll_geometry(wrap)
        .expect("#wrap is a scroll container");
    assert!(
        gw[2] > gw[4],
        "an inline-block row WITHOUT `nowrap` must still wrap — got scrollHeight {} vs \
         clientHeight {}. If this row is also one line, the fix disabled inline-block wrapping \
         wholesale rather than reading `white-space`",
        gw[2],
        gw[4]
    );
    assert_eq!(
        gw[3], gw[5],
        "a WRAPPED row has nothing to scroll horizontally — scrollWidth {} must equal clientWidth {}",
        gw[3], gw[5]
    );

    // ── The horizontal scroll actually applies, and SNAPS. Tick 266 built x-axis snapping and
    //    could never run it: there was no horizontal range in the engine to run it against.
    let a = page.set_element_scroll(bar, 120.0, 0.0);
    assert_eq!(
        a.0, 100.0,
        "scrolling to x=120 must LAND on the tab at 100 — got {a:?}. A bar that stops at 120 shows \
         two half-tabs and neither label reads"
    );

    // Deep into the row: a snapper that always picks the FIRST candidate is indistinguishable from
    // a correct one until the scroll travels past more than one snap point.
    let b = page.set_element_scroll(bar, 270.0, 0.0);
    assert_eq!(
        b.0, 300.0,
        "x=270 must snap to the NEAREST point (300), not back to 0 — got {b:?}"
    );

    // The LAST tab must be reachable. Snapping before clamping picks a point past the range and
    // clamps back to an unaligned offset, so the bar can never reach its own end.
    let end = page.set_element_scroll(bar, 9999.0, 0.0);
    assert_eq!(
        end.0, 300.0,
        "the bar must reach its own LAST tab at max scroll (300) — got {end:?}"
    );

    // The y-axis is pinned: a one-line row has no vertical range to wander into.
    assert_eq!(
        end.1, 0.0,
        "a one-line row must not scroll vertically — got {end:?}"
    );
}
