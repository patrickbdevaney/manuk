//! # av1_decode — the AV1 organ decodes real MP4 samples to the right picture
//!
//! Same shape as `video_decode` (M5): a real fixture through the real demux into the real
//! decoder, asserted on picture CONTENT — dimensions alone pass a scrambled decode, and
//! non-uniformity alone passes a channel swap. The four-colors pattern (see
//! `tests/data/README.md` for provenance) makes both visible: each quadrant must land on its
//! named color, so a U/V swap, a stride misread or a plane scramble each paint the wrong flag.

use manuk_media::{can_decode_video, demux, FrameTimeline, TrackKind};

const AV1: &[u8] = include_bytes!("data/four-colors-av1.mp4");

/// Mean RGB of a small patch centered in one quadrant — a patch, not a pixel, so chroma
/// subsampling edges and film-grain-free dither cannot flake the assert.
fn quadrant_mean(rgba: &[u8], w: usize, h: usize, right: bool, bottom: bool) -> [u8; 3] {
    let (cx, cy) = (
        if right { 3 * w / 4 } else { w / 4 },
        if bottom { 3 * h / 4 } else { h / 4 },
    );
    let mut sum = [0u64; 3];
    let mut n = 0u64;
    for row in cy.saturating_sub(4)..(cy + 4).min(h) {
        for col in cx.saturating_sub(4)..(cx + 4).min(w) {
            let px = (row * w + col) * 4;
            for c in 0..3 {
                sum[c] += rgba[px + c] as u64;
            }
            n += 1;
        }
    }
    [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8]
}

#[test]
fn av1_decode() {
    let movie = demux(AV1).expect("the AV1 fixture must demux (re_mp4 reads av01 sample entries)");
    let track = movie
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Video)
        .expect("the fixture carries a video track");
    assert!(
        track.codec.as_deref().unwrap_or("").starts_with("av01."),
        "the fixture's codec string must be av01.*, got {:?}",
        track.codec
    );
    assert!(
        can_decode_video(track),
        "with the av1 feature compiled in, can_decode must say yes to av01"
    );

    let timeline = FrameTimeline::decode(track, AV1).expect("the AV1 track must decode");
    assert!(!timeline.is_empty(), "at least one frame decodes");
    let frame = timeline.frames().first().unwrap();
    assert_eq!(
        (frame.width, frame.height),
        (track.width as u32, track.height as u32),
        "decoded dimensions must match the track's own declaration"
    );
    assert!(
        !frame.is_uniform(),
        "a mis-fed decoder emits a correctly-sized flat field — same honesty guard as M5"
    );

    let (w, h) = (frame.width as usize, frame.height as usize);
    let tl = quadrant_mean(&frame.rgba, w, h, false, false);
    let tr = quadrant_mean(&frame.rgba, w, h, true, false);
    let bl = quadrant_mean(&frame.rgba, w, h, false, true);
    let br = quadrant_mean(&frame.rgba, w, h, true, true);
    println!("quadrants: tl={tl:?} tr={tr:?} bl={bl:?} br={br:?}");

    // The four-colors pattern: yellow / red / blue / green. Dominance asserts (channel A well
    // above channel B) rather than exact bytes, so the BT.601 matrix's rounding is irrelevant
    // while a U/V swap — which turns red into blue and yellow into cyan — still fails loudly.
    let dominant = |c: [u8; 3], hi: usize, lo: usize| c[hi] > c[lo].saturating_add(60);
    assert!(
        dominant(tl, 0, 2) && dominant(tl, 1, 2),
        "top-left must be YELLOW (R and G far above B), got {tl:?}"
    );
    assert!(
        dominant(tr, 0, 1) && dominant(tr, 0, 2),
        "top-right must be RED, got {tr:?}"
    );
    assert!(
        dominant(bl, 2, 0) && dominant(bl, 2, 1),
        "bottom-left must be BLUE, got {bl:?}"
    );
    assert!(
        dominant(br, 1, 0) && dominant(br, 1, 2),
        "bottom-right must be GREEN, got {br:?}"
    );
}
