//! # G_STATIC_POS_LINE_START — an out-of-flow box that OPENS a line starts where the LINE starts
//!
//! CSS 2.1 §10.3.7 / §10.6.4: an `position: absolute` box with `auto` insets sits at its **static
//! position** — where it would have been had it stayed in the flow. `refine_inline_static_positions`
//! resolves that by walking the in-flow siblings that PRECEDE the box and taking the furthest point
//! flow reached. When there are none it did:
//!
//! ```rust,ignore
//!     if before.is_empty() { continue; }      // <- leaves the block-level default: the content edge
//! ```
//!
//! ⚠⚠⚠ **which is only harmless while the line starts at the content edge.** A centred or
//! right-aligned line does not. Measured against Chrome on a 400px block holding two 40px
//! `inline-block`s:
//!
//! ```text
//!                                       Chrome   before   after
//!   abspos FIRST on a centred line        160        0      160
//!   abspos BETWEEN two centred items      200      200      200   <- was already right
//!   abspos LAST on a centred line         240      240      240   <- was already right
//!   abspos first, text-align: left         40       40       40   <- must not move
//!   abspos first, text-align: right       360      360      360   <- must not move
//! ```
//!
//! **The middle rows are why this survived 1,080 ticks**: the common shape — a separator, badge or
//! icon *between* two things — has a preceding sibling and took the working branch. Only the box
//! that opens the line fell through, and it fell through to a value that is correct for the default
//! alignment.
//!
//! ## How it was found, which is the part worth keeping
//!
//! Not by looking at absolute positioning. Sweep t1080 named `reading_order` the top binding
//! conjunct of M1 for the third sweep running, and `MANUK_RO_PARTITION=1` localised one site's 12
//! inversions to **one 7-sibling `<footer>`** — `<a>` links separated by
//! `.line-between { display:inline-block; position:absolute; margin-top:15.5px }` inside a
//! `text-align:center` footer. That is an insetless abspos in a centred IFC, so a ten-row battery
//! was built for exactly that.
//!
//! ⚠⚠ **The battery REFUTED its own hypothesis and the finding is its negative row.** Nine of the
//! ten rows already agreed with Chrome to the pixel — including all three that match the footer's
//! actual shape (separator *between* items, with and without a negative `margin-left`). So the
//! centred static position was not that site's defect. The one row that disagreed is the one nobody
//! was looking at: the box that comes FIRST. A battery that had only reproduced the site would have
//! found nothing and reported the area clean.
//!
//! ## How this goes RED
//!
//! - Restore `if before.is_empty() { continue; }` → `#first_centre` reports x=0 against Chrome's
//!   160, and every other row still passes — which is the pair that separates "the centred static
//!   position is broken" from "the line-opening case is".
//! - Take the leftmost x over ALL lines instead of the first → `#first_wrap` reports 5 against
//!   Chrome's 10, because its second line holds a wider item and therefore starts further left.
//!
//! ONE `#[test]` per file — `PageContext` is per-process (see `g_caption_paint.rs`).

use manuk_text::FontContext;

const W: f32 = 800.0;

/// Widths are carried by fixed-size `inline-block`s, so every number below is font-independent:
/// `(400 − 80) / 2 = 160` holds whatever face resolves.
const HTML: &str = r##"<!doctype html><html><head><style>
body { margin: 0; font: 16px/1 monospace }
div { width: 400px; height: 40px; position: relative }
i { display: inline-block; width: 40px; height: 10px; font-style: normal }
b { display: inline-block; width: 10px; height: 10px; position: absolute }
</style></head><body>
<div style="text-align:center"><b id="first_centre"></b><i></i><i></i></div>
<div style="text-align:left"><b id="first_left"></b><i></i><i></i></div>
<div style="text-align:right"><b id="first_right"></b><i></i><i></i></div>
<div style="text-align:center"><i></i><b id="between"></b><i></i></div>
<div style="text-align:center"><i></i><i></i><b id="last"></b></div>
<div style="text-align:center"><i></i><b id="explicit" style="left:5px"></b><i></i></div>
<div style="text-align:center;height:80px;width:100px"><b id="first_wrap"></b><i></i><i></i><i style="width:90px"></i></div>
</body></html>"##;

#[test]
fn an_out_of_flow_box_that_opens_a_line_takes_the_lines_start_edge() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://example.test/", &fonts, W);
    let dom = page.dom();
    let rects = page.root_box.node_rects(dom);
    let x = |id: &str| -> f32 {
        let sel = format!("#{id}");
        let n = manuk_css::query_selector_all(dom, dom.root(), &sel)
            .first()
            .copied()
            .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
        rects
            .get(&n)
            .unwrap_or_else(|| panic!("no box for {sel}"))
            .x
    };

    // ── The defect. `(400 − 40 − 40) / 2 = 160`.
    assert!(
        (x("first_centre") - 160.0).abs() < 1.0,
        "an abspos box that OPENS a centred line starts where the LINE starts, not where the \
         content box does — Chrome says 160, got {}",
        x("first_centre")
    );

    // ── The two rows that must NOT move: the same markup under the alignments where the old
    //    behaviour happened to be right. Without these the gate cannot tell a fix from a shift.
    assert!(
        x("first_left").abs() < 1.0,
        "text-align:left is unchanged at 0 — got {}",
        x("first_left")
    );
    assert!(
        (x("first_right") - 320.0).abs() < 1.0,
        "text-align:right starts at 400 − 80 = 320 — got {}",
        x("first_right")
    );

    // ── The rows that were ALREADY correct, kept because a fix that broke them would be a trade.
    assert!(
        (x("between") - 200.0).abs() < 1.0,
        "a separator BETWEEN two centred items follows the first one (160 + 40) — got {}",
        x("between")
    );
    assert!(
        (x("last") - 240.0).abs() < 1.0,
        "an abspos box after both items follows the second (160 + 80) — got {}",
        x("last")
    );
    assert!(
        (x("explicit") - 5.0).abs() < 1.0,
        "an EXPLICIT `left` is not a static position and centring must not touch it — got {}",
        x("explicit")
    );

    // ── The wrap row, and its widths are chosen so the two candidate answers DIFFER. In a 100px
    //    block, items 40 + 40 wrap the third (90px) onto line two: line one starts at
    //    `(100 − 80) / 2 = 10` and line two starts at `(100 − 90) / 2 = 5`. A box that opens the
    //    box takes the FIRST line's start, not the leftmost start on any line.
    //
    //    ⚠ The first version of this row used three 40px items, and the two answers COINCIDED at
    //    10 — so the "leftmost over all lines" mutation came back GREEN and the row was testing
    //    nothing. A green mutation is a reading (t1057), and here it read the fixture, not the code.
    assert!(
        (x("first_wrap") - 10.0).abs() < 1.0,
        "the box opens the FIRST line, so it takes the first line's start (10, Chrome-measured), \
         not the leftmost start on any line (5) — got {}",
        x("first_wrap")
    );
}
