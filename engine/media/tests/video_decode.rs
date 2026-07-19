//! # The video's real first frame — media step M5
//!
//! M3 could find the video track and name it (`avc1.*`, 640x360) and could not produce a single
//! pixel of it. Naming a codec and decoding it are different claims, and this file is the second
//! one.
//!
//! ## How each assertion here can go RED
//!
//! Every green below has a demonstrated failure mode, because a gate that cannot fail measured
//! nothing:
//!
//! - **Dimensions** come from two independent sources — the container's `tkhd`/`avc1` header versus
//!   what the decoder reports after reading the SPS. A decoder fed a mis-converted bitstream either
//!   emits nothing or emits the wrong size; it does not accidentally agree.
//! - **Non-uniformity** is the assertion that survives the failure mode a size check cannot see. A
//!   decoder handed AVCC length prefixes instead of Annex-B start codes, or handed frames without
//!   its SPS/PPS, produces either no frame at all or a flat field. Correctly-sized green passes
//!   every dimension check ever written. RED, run: drop the parameter sets from
//!   `H264Decoder::new` and the decode yields `None` for every sample.
//! - **The High-profile refusal** is the honesty claim. RED, run: widen `can_decode` to accept any
//!   `avc1.` prefix and the High fixture starts answering `true` for a stream this backend cannot
//!   decode — which is exactly the lie that strands a player with no fallback.

use manuk_media::{can_decode_video, decode_first_frame, demux, TrackKind};

/// Constrained Baseline, 640x360 — minted from the High-profile fixture with the *system* ffmpeg
/// binary as a dev tool. That is authoring a test file, not linking ffmpeg into the browser.
const BASELINE: &[u8] = include_bytes!("data/bear-baseline_frag.mp4");

/// The same content as High profile (`AVCProfileIndication = 100`), which this backend must refuse.
const HIGH: &[u8] = include_bytes!("data/bear-640x360-v-2frames_frag.mp4");

fn video_track(bytes: &[u8]) -> manuk_media::Track {
    let movie = demux(bytes).expect("fixture demuxes");
    movie
        .tracks
        .into_iter()
        .find(|t| t.kind == TrackKind::Video)
        .expect("fixture has a video track")
}

#[test]
fn baseline_first_frame_decodes_to_a_real_picture() {
    let track = video_track(BASELINE);
    assert!(
        can_decode_video(&track),
        "Baseline must be accepted; codec string was {:?}",
        track.codec
    );

    let frame = decode_first_frame(&track, BASELINE).expect("baseline decodes");

    // Container header vs decoder output — two independent sources for the same number.
    assert_eq!(
        (frame.width, frame.height),
        (track.width, track.height),
        "decoder dimensions must match the container's"
    );
    assert_eq!((frame.width, frame.height), (640, 360));

    // Tightly packed RGBA, no stride padding — the paint path indexes it directly.
    assert_eq!(
        frame.rgba.len(),
        (frame.width * frame.height * 4) as usize,
        "frame buffer must be exactly width*height*4"
    );

    // The assertion a size check cannot make: this is an IMAGE, not a correctly-sized flat field.
    assert!(
        !frame.is_uniform(),
        "every pixel is identical — the decoder produced a flat field, not a picture"
    );

    assert!(
        frame.rgba.chunks_exact(4).all(|px| px[3] == 255),
        "alpha must be opaque across the frame"
    );
}

/// The honest `isTypeSupported`. High profile is most of the web's H.264 and this backend cannot
/// decode it, so it must answer `false` rather than accept and fail mid-stream.
#[test]
fn high_profile_is_refused_rather_than_accepted_and_failed() {
    let track = video_track(HIGH);
    assert!(
        !can_decode_video(&track),
        "High profile must be refused up front; codec string was {:?}",
        track.codec
    );
    assert!(
        decode_first_frame(&track, HIGH).is_err(),
        "a refused track must not decode"
    );
}

/// The AVCC->Annex-B rewrite, checked directly rather than only through a decode.
#[test]
fn avcc_lengths_become_start_codes() {
    // Two NALs, 4-byte lengths: [3]{aa bb cc} and [2]{dd ee}.
    let sample = [0, 0, 0, 3, 0xaa, 0xbb, 0xcc, 0, 0, 0, 2, 0xdd, 0xee];
    assert_eq!(
        manuk_media::video::annex_b(&sample, 4),
        vec![0, 0, 0, 1, 0xaa, 0xbb, 0xcc, 0, 0, 0, 1, 0xdd, 0xee]
    );

    // A NAL whose declared length overruns the sample truncates rather than failing: the whole
    // NALs before it are still good data and a real stream can be appended mid-fragment.
    let truncated = [0, 0, 0, 3, 0xaa, 0xbb, 0xcc, 0, 0, 0, 9, 0xdd];
    assert_eq!(
        manuk_media::video::annex_b(&truncated, 4),
        vec![0, 0, 0, 1, 0xaa, 0xbb, 0xcc]
    );
}
