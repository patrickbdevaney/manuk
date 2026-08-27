//! **G_INLINE_IMAGE_SIZE — an inline `data:` image has its natural size in the FIRST layout.**
//!
//! A `data:` image carries its own bytes. There is nothing to fetch and nothing to wait for, yet
//! image sizing lived entirely in the **async subresource pass** — so on every path that does not run
//! that pass (`Page::load`, every gate, the WPT runner) an inline image laid out `0x0`: in the tree,
//! styled, painted nowhere. Decoding it before the first layout is the fix, and it is also the
//! honest one — the information was already in the document.
//!
//! The second half is the **aspect ratio crossing into taffy**. The block path derives an `auto` axis
//! from the other one through `ComputedStyle::aspect_ratio`, but a flex or grid item's size is
//! taffy's to decide and taffy was never told the ratio. An image given only a `height` therefore
//! came out **zero pixels wide** — the worst kind of failure, because the element is present and
//! measurable and simply cannot be seen.
//!
//! Together these are the avatar / logo / thumbnail / inline-icon case, which is most of the images
//! in a modern component library.

use manuk_text::FontContext;

/// A 16x16 solid-blue PNG, inline. The size is the point: every assertion below is a statement
/// about 16 and its ratio of 1.
const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAFUlEQVR4nGNgYPhPIhrVMKph2GoAAJLb/wFh5Z4RAAAAAElFTkSuQmCC";

fn width_height_of(open: &str, close: &str, style: &str) -> String {
    let html = format!(
        r#"<!doctype html><body style="margin:0">
{open}<img id="a" src="{PNG}" style="{style}">{close}
<div id="out">-</div><script>
var r = document.getElementById('a').getBoundingClientRect();
document.getElementById('out').textContent = Math.round(r.width) + 'x' + Math.round(r.height);
</script>"#
    );
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(&html, "https://gis.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    page.dom().text_content(out)
}

/// The same measurement for an arbitrary grid ITEM — the `<img>` helper above cannot express a
/// non-replaced subject, and a non-replaced subject is the only thing that separates "has an
/// intrinsic size" from "has an aspect ratio". See `#[test] a_non_replaced_grid_item_still_stretches`.
fn grid_item_size(item: &str) -> String {
    let html = format!(
        r#"<!doctype html><body style="margin:0">
<div style="display:grid;width:40px;height:40px">{item}</div>
<div id="out">-</div><script>
var r = document.getElementById('a').getBoundingClientRect();
document.getElementById('out').textContent = Math.round(r.width) + 'x' + Math.round(r.height);
</script>"#
    );
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(&html, "https://gis.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    page.dom().text_content(out)
}

#[test]
fn an_inline_data_image_is_sized_from_its_own_bytes() {
    // A 40x40 flex container, alignment pinned to start so nothing stretches the item and the
    // number under test is the image's own size and not the container's.
    let flex = r#"<div style="display:flex;flex-direction:row;width:40px;height:40px;align-items:flex-start;justify-content:flex-start">"#;
    let grid = r#"<div style="display:grid;width:40px;height:40px">"#;
    // `min-width/min-height:0` disables the automatic minimum size, so these measure the sizing
    // channel under test rather than the flex min-size floor (as WPT's own image-as-flexitem does).
    let unclamped = "min-width:0;min-height:0";

    for (open, close, style, want, why) in [
        (
            "<div>", "</div>", "", "16x16",
            "THE BUG: an inline image carries its own bytes, so it must have its natural 16x16 in \
             the FIRST layout. `0x0` means image sizing only ever happened in the async subresource \
             pass, and every path that does not run that pass renders the image as nothing",
        ),
        (
            flex, "</div>", unclamped, "16x16",
            "the same natural size when the image is a FLEX ITEM — the avatar-in-a-row case",
        ),
        (
            grid, "</div>", "", "16x16",
            "and as a GRID ITEM",
        ),
        (
            flex, "</div>", "min-width:0;min-height:0;width:30px", "30x30",
            "a flex item given only a WIDTH derives its height through the ratio",
        ),
        (
            flex, "</div>", "min-width:0;min-height:0;height:30px", "30x30",
            "and given only a HEIGHT it must derive its WIDTH — this is the half that was missing, \
             because the ratio never crossed into taffy. `0x30` is a zero-width image: present, \
             laid out, invisible",
        ),
        (
            flex, "</div>", "min-width:0;min-height:0;max-width:8px", "8x8",
            "a max-width clamp on a flex item transfers through the ratio rather than squashing the \
             image (the `max-width:100%` reset every site ships)",
        ),
        (
            "<div>", "</div>", "max-width:8px", "8x8",
            "and the same clamp on a plain block image",
        ),
        // ── ⚠⚠⚠ **A REPLACED GRID ITEM DOES NOT STRETCH TO ITS CELL** (t1345). The `grid` row above
        //    read 40x40 — the avatar inflated to the whole track — because `normal` and `stretch`
        //    were ONE value in the cascade and taffy was handed `stretch` for every grid container.
        //    All six rows are headless-Chrome-measured on this fixture; each names a different way a
        //    plausible fix goes wrong.
        (
            r#"<div style="display:grid;width:40px;height:40px;align-items:stretch;justify-items:stretch">"#,
            "</div>", "", "40x40",
            "⭐ an EXPLICIT `stretch` still stretches — Chrome 40x40. This is the row that proves              `normal` and `stretch` are two values and not one: the row above it, same container              with no declaration, is 16x16. A fix that made replaced grid items never stretch              passes that row and fails this one",
        ),
        (
            r#"<div style="display:grid;width:40px;height:40px;align-items:normal;justify-items:normal">"#,
            "</div>", "", "16x16",
            "and an EXPLICIT `normal` behaves as `start` — Chrome 16x16, the same as the undeclared              default, which is what makes `normal` the initial value rather than a synonym",
        ),
        (
            r#"<div style="display:grid;width:40px;height:40px;align-items:center;justify-items:center">"#,
            "</div>", "", "16x16",
            "`center` was never a stretch and is unchanged — the control that catches a fix which              forces START over every declared alignment",
        ),
        (
            r#"<div style="display:grid;width:40px;height:40px">"#,
            "</div>", "width:8px", "8x8",
            "an item with a SPECIFIED size was already exempt (stretch only applies to an `auto`              axis) and must stay exactly so — 8x8, not 8x40",
        ),
        (
            r#"<div style="display:grid;width:100px;height:100px">"#,
            "</div>", "", "16x16",
            "a bigger cell does not change the answer: the item is at its natural size, not at some              fraction of the track",
        ),
        (
            r#"<div style="display:flex;flex-direction:row;width:40px;height:40px">"#,
            "</div>", "", "40x40",
            "⚠⚠ AND THE BOUNDARY: in FLEX, Chrome DOES stretch the same image — the default              `align-items:stretch` fills the cross axis and the ratio carries the main axis with it,              so 40x40. The rule is grid-only. A fix applied in `to_taffy_style` (which cannot see              the parent) breaks this row",
        ),
        (
            "<div>", "</div>", "", "16x16",
            "the plain block control, repeated as the LAST row so a fix that reorders the table \
             still measures it",
        ),
    ] {
        let got = width_height_of(open, close, style);
        assert_eq!(
            got, want,
            "G_INLINE_IMAGE_SIZE: expected `{want}` — {why}.\n  got: {got}"
        );
    }

    // ⚠⚠⚠ **THE ROW THAT NAMES THE PREDICATE, AND IT WAS MISSING FOR ONE MUTATION.** The grid-item
    // `start` rule fires on a box with an INTRINSIC SIZE, not on a box with an aspect ratio, and the
    // two are trivially confusable because every replaced element in this file has both. Written first
    // as `if ccs.aspect_ratio.is_none() { continue }`, the mutation **passed the entire gate above** —
    // six new Chrome-measured rows and none of them could see it. Chrome-measured:
    //
    // ```text
    //   <div style="aspect-ratio:1/1">x</div>   in a 40x40 grid   ->  40x40   STRETCHES
    //   <div>x</div>                            in a 40x40 grid   ->  40x40   STRETCHES
    //   <img src=16x16>                         in a 40x40 grid   ->  16x16   does NOT
    // ```
    //
    // ⭐ An arm that cannot distinguish the failures it covers has one assertion's cost and no
    // assertion's value. These two rows cost one `Page::load` each and refute the whole class.

    for (item, want, why) in [
        (
            r#"<div id="a" style="aspect-ratio:1/1">x</div>"#,
            "40x40",
            "a DECLARED `aspect-ratio` on a non-replaced box does NOT exempt it from stretch —              Chrome fills the cell. Reading 16x16 (or any content size) means the exemption was              keyed on the ratio rather than on an intrinsic size, which is the difference between              'this box is a picture' and 'this box has a shape'",
        ),
        (
            r#"<div id="a">x</div>"#,
            "40x40",
            "and the plain control: an ordinary block grid item stretches to its cell, which is the              behaviour `normal` has for everything that is not replaced",
        ),
    ] {
        let got = grid_item_size(item);
        assert_eq!(
            got, want,
            "G_INLINE_IMAGE_SIZE: expected `{want}` — {why}.\n  got: {got}"
        );
    }
}
