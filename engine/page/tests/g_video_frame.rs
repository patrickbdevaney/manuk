//! **G_VIDEO_FRAME — a decoded video frame paints where the poster was.**
//!
//! MEDIA.md's tick-1 deliverable, and the first time a `<video>` on this engine shows anything but a
//! still image the network handed it. Ticks 234/235/236 built the pipeline up to the frame — demux
//! (`re_mp4`), AAC→PCM (`symphonia`), H.264→RGBA (`openh264`) — and stopped one step short of the
//! screen: `decode_first_frame` returned a correct picture that nothing could display. This is that
//! step, and it is small on purpose.
//!
//! **The structural claim being tested is that video needs NO new paint code.** A `<video>` is already
//! a replaced element; a `<video poster>` already decodes and paints through the identical route as
//! `<img>` — `Page::images` keyed by the video's own `NodeId`, blitted into the content box. So
//! `set_video_frame` overwrites one map entry and the existing painter does the rest. If that claim
//! were wrong, the frame would land in the map and never reach a pixel — which is precisely why every
//! assertion below reads the **rasterized page**, not the map.
//!
//! **Why the fixture is asymmetric, and this is the lesson tick 236 and tick 237 both paid for.** A
//! correctly-sized flat field passes every size check ever written, and a flat field is exactly what a
//! mis-fed decoder, a mis-scaled blit, or a pattern-matrix bug emits. Tick 237's `drawImage` gate
//! passed a one-corner pixel assertion *by accident* for that reason. So the frame here is green on
//! its left half and blue on its right, and the gate asserts BOTH halves — a uniform fill of either
//! colour, in either position, fails. It also asserts the poster's red is **gone**, because "the new
//! frame painted" and "the frame painted OVER the poster" are different claims and only the second one
//! is what playback means.

use manuk_text::FontContext;

/// A 8x8 solid-RED PNG, inline — the poster. Red appears nowhere in the video frame, so any red left
/// in the video's box after a frame is handed over means the poster was never replaced.
const POSTER: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAEklEQVR4nGP4z8CAFWEXHbQSACj/P8Fu7N9hAAAAAElFTkSuQmCC";

const VW: u32 = 64;
const VH: u32 = 48;

/// Green | blue, split down the middle. Deliberately NOT uniform: see the module note.
fn split_frame(w: u32, h: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for _y in 0..h {
        for x in 0..w {
            if x < w / 2 {
                rgba.extend_from_slice(&[0, 255, 0, 255]); // green
            } else {
                rgba.extend_from_slice(&[0, 0, 255, 255]); // blue
            }
        }
    }
    rgba
}

fn page_html() -> String {
    format!(
        r#"<!doctype html><body style="margin:0;background:#fff">
<video id="v" poster="{POSTER}" width="{VW}" height="{VH}"></video>
</body>"#
    )
}

/// Read one pixel out of the rasterized page.
fn px(bytes: &[u8], canvas_w: u32, x: u32, y: u32) -> (u8, u8, u8) {
    let i = ((y * canvas_w + x) * 4) as usize;
    (bytes[i], bytes[i + 1], bytes[i + 2])
}

/// "Is this pixel mostly `ch`?" — tolerant of the blit's edge filtering, intolerant of a wrong colour.
fn dominant(p: (u8, u8, u8), ch: char) -> bool {
    let (r, g, b) = p;
    match ch {
        'r' => r > 150 && g < 100 && b < 100,
        'g' => g > 150 && r < 100 && b < 100,
        'b' => b > 150 && r < 100 && g < 100,
        _ => unreachable!(),
    }
}

#[test]
fn a_decoded_frame_paints_over_the_poster_in_the_video_box() {
    let fonts = FontContext::new();
    let (cw, chh) = (200u32, 120u32);

    let mut page = manuk_page::Page::load(&page_html(), "https://video.test/", &fonts, cw as f32);
    let root = page.dom().root();
    let v = manuk_css::query_selector_all(page.dom(), root, "#v")[0];

    // ── 1. BASELINE: the poster paints. If this fails the gate is measuring nothing downstream —
    //    a video box that shows nothing at all would trivially "lose the red" in step 3.
    let before = page.paint(&fonts, cw, chh);
    let bb = before.rgba_bytes();
    let (lx, rx, my) = (VW / 4, VW * 3 / 4, VH / 2);
    assert!(
        dominant(px(bb, cw, lx, my), 'r') && dominant(px(bb, cw, rx, my), 'r'),
        "G_VIDEO_FRAME: the POSTER must paint before any frame exists — got left {:?} right {:?}. \
         Without this baseline the 'poster is gone' claim below is vacuous.",
        px(bb, cw, lx, my),
        px(bb, cw, rx, my)
    );

    // ── 2. Hand over a decoded frame. No relayout, no new display item — one map entry.
    page.set_video_frame(v, VW, VH, split_frame(VW, VH));

    // ── 3. The frame is on screen, and it is a PICTURE, not a fill.
    let after = page.paint(&fonts, cw, chh);
    let ab = after.rgba_bytes();
    let left = px(ab, cw, lx, my);
    let right = px(ab, cw, rx, my);

    assert!(
        dominant(left, 'g'),
        "G_VIDEO_FRAME: the frame's LEFT half must be green — got {left:?}. \
         The frame reached Page::images but not the screen, or the blit is mis-scaled."
    );
    assert!(
        dominant(right, 'b'),
        "G_VIDEO_FRAME: the frame's RIGHT half must be blue — got {right:?}. \
         A uniform fill of the left-hand colour passes a one-sided check; this is the half that \
         catches it (see tick 237's drawImage near-miss)."
    );
    assert!(
        left != right,
        "G_VIDEO_FRAME: both halves sampled {left:?} — the box is a FLAT FIELD. That is exactly what \
         a mis-fed decoder or a collapsed blit emits, and it passes every size assertion."
    );
    assert!(
        !dominant(left, 'r') && !dominant(right, 'r'),
        "G_VIDEO_FRAME: the poster's red survived the frame ({left:?}, {right:?}) — the frame was \
         added ALONGSIDE the poster rather than replacing it. Playback means replacing it."
    );
}

#[test]
fn the_frame_does_not_resize_the_video_box() {
    // A `<video>`'s box comes from its attributes/CSS, never from the frame currently on screen —
    // otherwise the page reflows on the first frame, and again every time an adaptive stream changes
    // resolution mid-playback, which is what adaptive streaming does BY DESIGN. The frame is scaled
    // into the box that already exists.
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(&page_html(), "https://video.test/", &fonts, 200.0);
    let root = page.dom().root();
    let v = manuk_css::query_selector_all(page.dom(), root, "#v")[0];

    let before = page.node_rects()[&v];

    // A frame with a WILDLY different shape than the box (and a different aspect ratio).
    page.set_video_frame(v, VW * 5, VH, split_frame(VW * 5, VH));
    let after = page.node_rects()[&v];

    assert!(
        (before.width - after.width).abs() < 0.5 && (before.height - after.height).abs() < 0.5,
        "G_VIDEO_FRAME: handing over a {}x{} frame resized the video box from {:?} to {:?}. \
         The box is authored, not derived from the stream.",
        VW * 5,
        VH,
        (before.width, before.height),
        (after.width, after.height)
    );
}
