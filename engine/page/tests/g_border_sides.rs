//! # G_BORDER_SIDES — a border has FOUR colours and FOUR styles, not one of each
//!
//! `ComputedStyle` held `border_color: Rgba` and `border_style: BorderStyle` as **scalars**, beside
//! a per-side `border_width: Sides<f32>`, and `stylo_map` filled the colour from
//! `clone_border_top_color()`. So every box on the web painted all four of its edges in its **top**
//! edge's colour and its top edge's line style:
//!
//! ```text
//!   border-top-color:blue right:red bottom:orange left:green   ->  all four painted BLUE
//!   border-*-style: solid / dashed / dotted / double           ->  all four painted SOLID
//! ```
//!
//! ⚠⚠⚠ **The top edge was right, which is why nothing looked broken.** A defect that is correct on
//! the first thing you check survives every casual look; it took a test that *distinguishes the
//! sides on purpose* to see it. That test is CSS 2.1's `*-applies-to-NNN` family — 483 of the
//! suite's 3,022 remaining failures at t1078 — whose reference files tell a box's top edge from its
//! bottom by giving them different colours:
//!
//! ```text
//!   margin-top-applies-to-012-ref.xht   border-top: blue solid 10px; border-bottom: orange solid 10px
//!   Chrome                              blue bar, gap, ORANGE bar
//!   ours                                blue bar, gap, BLUE bar
//! ```
//!
//! The idioms it breaks are ordinary, not exotic: the `border-left: 3px solid <brand>` accent bar
//! on a card, callout or blockquote; the `border-bottom` rule under a heading or an active tab; the
//! coloured horizontal rules in a data table; and the dashed-only-on-one-side drop zone.
//!
//! ## Three parse defects fell out of the same scalar
//!
//! - **`border-color: red blue` collapsed to `red`** — the 1-to-4-value box-side shorthand was not
//!   expanded, so two of the four sides were the wrong colour whenever an author used the short
//!   form that exists precisely to set them differently.
//! - **`border-top-color` and its three siblings had no arm at all** in MinimalCascade. Not
//!   mis-parsed: absent.
//! - ⚠ **`border-left-style: none` zeroed ALL FOUR widths.** `s.border_width = Sides::all(0.0)` was
//!   correct while the style was a scalar and is a whole-border delete now — and
//!   `border: 1px solid; border-right-style: none` is how every segmented control and button group
//!   on the web joins its cells.
//!
//! ## How this goes RED — and the two mutations that came back GREEN, which are the finding
//!
//! - Restore `border.colors = [top; 4]` in `border_of` → every colour row fails. **RED.**
//! - Restore `clone_border_top_color()` for all four in `stylo_map` → every colour row fails and
//!   every style row passes. **RED**, and the pair with the one above separates the paint half from
//!   the cascade half.
//! - Restore `s.border_style = Sides::all(st)` in `set_side_style` → `#styles` and `#sideshort`
//!   fail. **RED** — the line style is the half that comes from MinimalCascade.
//! - Restore `s.border_style = Sides::all(st)` in the `border-<side>` shorthand arm → `#sideshort`
//!   fails while `#styles` (longhands) passes. **RED**, and that pair is what tells a broken
//!   shorthand from broken storage.
//!
//! ⚠⚠⚠ **Two mutations came back GREEN, and a green mutation is a reading** (t1057): restoring
//! `s.border_color = Sides::all(c)` in MinimalCascade's `border-<side>` arm, and restoring
//! `s.border_width = Sides::all(0.0)` in `set_side_style`, change **nothing** on this page. The
//! reason is the standing one: the shipping cascade is **Stylo**, and it owns border COLOUR and
//! border WIDTH outright — `stylo_map` overwrites both, and Stylo already zeroes a `none` side's
//! width via `clone_border_right_style().none_or_hidden()`. Only the line STYLE is recovered from
//! MinimalCascade, so only the style half of the MinimalCascade fix is observable here. Those
//! MinimalCascade edits are still correct and still load-bearing for the no-`stylo` build the
//! layout batteries use — but this gate cannot see them, and saying so beats implying a guard it
//! has not earned.
//!
//! ONE `#[test]` per file — `PageContext` is per-process (see `g_caption_paint.rs`).

use manuk_paint::DisplayItem;
use manuk_text::FontContext;

const W: f32 = 800.0;

const HTML: &str = r##"<!doctype html><html><head><style>
body { margin: 0 }
div { width: 200px; height: 60px }
/* longhands */
#colors { border-style: solid; border-width: 20px;
          border-top-color: rgb(0,0,255); border-right-color: rgb(255,0,0);
          border-bottom-color: rgb(255,165,0); border-left-color: rgb(0,128,0) }
/* the per-side SHORTHAND, in the colour-first order the CSS 2.1 reference files use */
#shorthand { border-top: rgb(0,0,255) solid 20px; border-bottom: rgb(255,165,0) solid 20px }
/* the 1-to-4-value box-side form: block axis then inline axis */
#twovalue { border-style: solid; border-width: 20px; border-color: rgb(0,0,255) rgb(255,0,0) }
/* one side reset to `none` must not delete the other three */
#oneside { border: 20px solid rgb(0,0,255); border-right-style: none }
/* per-side line STYLE, via longhands */
#styles { border-width: 20px; border-color: rgb(0,0,255);
          border-top-style: solid; border-bottom-style: dashed }
/* …and via the per-side SHORTHAND, which is a different write in the same parser */
#sideshort { border: 20px solid rgb(0,0,255); border-bottom: 20px dashed rgb(0,0,255) }
</style></head><body>
<div id="colors"></div>
<div id="shorthand"></div>
<div id="twovalue"></div>
<div id="oneside"></div>
<div id="styles"></div>
<div id="sideshort"></div>
</body></html>"##;

/// Every painted `Rect`, as `(x, y, w, h, rgba)` — the border edges are `Rect`s, and a border with
/// no background emits nothing else.
fn rects(list: &manuk_paint::DisplayList) -> Vec<(f32, f32, f32, f32, (u8, u8, u8))> {
    list.items
        .iter()
        .filter_map(|it| match it {
            DisplayItem::Rect { rect, color } => Some((
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                (color.r, color.g, color.b),
            )),
            _ => None,
        })
        .collect()
}

/// The colour of the edge covering `(x, y)`, or `None` when nothing paints there.
fn at(rs: &[(f32, f32, f32, f32, (u8, u8, u8))], x: f32, y: f32) -> Option<(u8, u8, u8)> {
    rs.iter()
        .find(|(rx, ry, w, h, _)| x >= *rx && x < rx + w && y >= *ry && y < ry + h)
        .map(|r| r.4)
}

/// How many separate `Rect`s fall inside the band `y in [y0, y1)` — a solid edge is one, a dashed
/// edge is many. Counting is what tells `dashed` from `solid` without asserting a dash period.
fn segments(rs: &[(f32, f32, f32, f32, (u8, u8, u8))], y0: f32, y1: f32) -> usize {
    rs.iter()
        .filter(|(_, ry, _, h, _)| *ry >= y0 && ry + h <= y1)
        .count()
}

const BLUE: (u8, u8, u8) = (0, 0, 255);
const RED: (u8, u8, u8) = (255, 0, 0);
const ORANGE: (u8, u8, u8) = (255, 165, 0);
const GREEN: (u8, u8, u8) = (0, 128, 0);

#[test]
fn each_border_edge_carries_its_own_colour_and_line_style() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://example.test/", &fonts, W);
    let rs = rects(&page.display_list());
    let seen = || format!("{rs:?}");

    // Each div is 20 + 60 + 20 = 100 tall and 20 + 200 + 20 = 240 wide, stacked from y = 0.
    let row = |i: f32| i * 100.0;

    // ── #colors — the four longhands, the row that names the defect.
    let y = row(0.0);
    assert_eq!(at(&rs, 120.0, y + 5.0), Some(BLUE), "top — {}", seen());
    assert_eq!(
        at(&rs, 120.0, y + 95.0),
        Some(ORANGE),
        "bottom: this was BLUE, the top edge's colour, for 1,078 ticks — {}",
        seen()
    );
    assert_eq!(at(&rs, 5.0, y + 50.0), Some(GREEN), "left — {}", seen());
    assert_eq!(at(&rs, 230.0, y + 50.0), Some(RED), "right — {}", seen());

    // ── #shorthand — `border-bottom: <color> solid 20px`. Separate from #colors on purpose: a fix
    //    to the STORAGE without a fix to the per-side shorthand passes above and fails here.
    let y = row(1.0);
    assert_eq!(at(&rs, 120.0, y + 5.0), Some(BLUE), "top — {}", seen());
    assert_eq!(
        at(&rs, 120.0, y + 95.0),
        Some(ORANGE),
        "`border-bottom` must not repaint the top edge — {}",
        seen()
    );

    // ── #twovalue — `border-color: blue red` is blue on the block axis, red on the inline one.
    let y = row(2.0);
    assert_eq!(at(&rs, 120.0, y + 5.0), Some(BLUE), "top — {}", seen());
    assert_eq!(at(&rs, 120.0, y + 95.0), Some(BLUE), "bottom — {}", seen());
    assert_eq!(
        at(&rs, 5.0, y + 50.0),
        Some(RED),
        "the 1-to-4-value form: the SECOND value is the inline axis — {}",
        seen()
    );
    assert_eq!(at(&rs, 230.0, y + 50.0), Some(RED), "right — {}", seen());

    // ── #oneside — NEGATIVE: `border-right-style: none` removes ONE edge.
    let y = row(3.0);
    assert_eq!(
        at(&rs, 230.0, y + 50.0),
        None,
        "the right edge is `none`, so it paints nothing — {}",
        seen()
    );
    for (x, yy, which) in [
        (120.0, y + 5.0, "top"),
        (120.0, y + 95.0, "bottom"),
        (5.0, y + 50.0, "left"),
    ] {
        assert_eq!(
            at(&rs, x, yy),
            Some(BLUE),
            "…and the other three edges SURVIVE it — `Sides::all(0.0)` used to delete all four \
             ({which}) — {}",
            seen()
        );
    }

    // ── #styles — the line style is per side too. A solid edge is one Rect; a dashed edge is many.
    let y = row(4.0);
    assert_eq!(
        segments(&rs, y, y + 20.0),
        1,
        "a solid top edge is one Rect — {}",
        seen()
    );
    // A dashed edge is `dash = 3 x thickness`, `gap = 3 x thickness`, so a 200px-wide, 20px-thick
    // edge is exactly two segments. `> 1` is the whole discriminator against solid, which is 1 —
    // asserting the count itself would pin the gate to the dash period rather than to the defect.
    assert!(
        segments(&rs, y + 80.0, y + 100.0) > 1,
        "a DASHED bottom edge breaks into segments; it painted solid while the top said solid — {}",
        seen()
    );

    // ── #sideshort — the same claim through `border-bottom: 20px dashed`, the SHORTHAND. Its style
    //    write is a different line of the parser from the longhand above, and only this row can
    //    tell a shorthand that repaints all four sides from storage that cannot hold four.
    let y = row(5.0);
    assert_eq!(
        segments(&rs, y, y + 20.0),
        1,
        "`border-bottom: … dashed` must leave the TOP edge solid — {}",
        seen()
    );
    assert!(
        segments(&rs, y + 80.0, y + 100.0) > 1,
        "…and must make the bottom edge dashed — {}",
        seen()
    );
}
