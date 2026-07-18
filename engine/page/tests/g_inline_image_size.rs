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
    ] {
        let got = width_height_of(open, close, style);
        assert_eq!(
            got, want,
            "G_INLINE_IMAGE_SIZE: expected `{want}` — {why}.\n  got: {got}"
        );
    }
}
