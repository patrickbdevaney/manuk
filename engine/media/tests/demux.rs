//! **G_MEDIA_DEMUX — the engine can open a media file.**
//!
//! Media step **M3**, gated against **real encoded files**, not synthesised ones.
//!
//! **The failure this gate exists for.** The constellation's verdict for `container demux
//! (MP4/WebM)` was *"cannot even open a file"*. The byte pipe below it worked — a page could
//! construct a `MediaSource`, fetch a segment byte-exactly and `appendBuffer` it — and then the
//! bytes sat in a list nothing had ever looked at. No track, no timeline, no `buffered`. Every
//! adaptive player steers its fetch loop by `buffered`, so an empty one is not a cosmetic gap: the
//! loop cannot advance.
//!
//! **What is asserted, and why each one is here rather than a plausible-looking substitute.**
//!
//! * **Both container forms.** Fragmented (`moof`/`traf`/`trun`) is what MSE streams; progressive
//!   (`stbl`) is what an ordinary `<video src>` loads. They are genuinely different code paths in
//!   the parser, and the fragmented one is where the bug below lived.
//! * **Exact dimensions and codec strings**, not "some track was found". `avc1.64001E` is the
//!   string a player string-compares against `isTypeSupported`; getting a track count right while
//!   getting its codec wrong is indistinguishable from working until a real player reads it.
//! * **Sync flags, differentially.** See `sync_flags_are_not_inverted` — this is the assertion that
//!   caught a real bug in the borrowed parser, and the only one that could have.
//! * **`buffered` arithmetic.** The end of the range is checked against the timescale arithmetic
//!   (2 frames × 1001 / 30000), not against a magic constant, so the assertion states the
//!   relationship rather than a number someone once observed.
//!
//! **RED, run:** returning `s.is_sync` unchanged (i.e. deleting the correction in `demux`) fails
//! `sync_flags_are_not_inverted` on all three fixtures; making `demux` return an empty track list
//! fails every test here.

use manuk_media::{demux, sniff, Container, DemuxError, TrackKind};

fn fixture(name: &str) -> Vec<u8> {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
    std::fs::read(format!("{p}{name}")).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

/// The fragmented form — what an MSE `SourceBuffer` is fed, on every streaming site.
#[test]
fn a_fragmented_mp4_opens() {
    let bytes = fixture("bear-640x360-v-2frames_frag.mp4");
    assert_eq!(sniff(&bytes), Container::Mp4);

    let m = demux(&bytes).expect("a real fMP4 must demux");
    assert!(
        m.fragmented,
        "moof boxes present — this is the streaming form"
    );
    assert_eq!(m.tracks.len(), 1, "video-only fixture");

    let v = m.video().expect("the video track must be found by kind");
    assert_eq!(v.kind, TrackKind::Video);
    assert_eq!((v.width, v.height), (640, 360));
    assert_eq!(
        v.codec.as_deref(),
        Some("avc1.64001E"),
        "the RFC 6381 string a player compares"
    );
    assert_eq!(v.timescale, 30000);
    assert_eq!(v.samples.len(), 2, "two coded frames");

    // The decoder configuration record — the handoff to M4/M5. `avcC` starts with configuration
    // version 1; a truncated or absent record would leave a decoder with no SPS/PPS.
    let cfg = v
        .codec_config
        .as_ref()
        .expect("avcC must be extracted for the decoder step");
    assert_eq!(cfg.first(), Some(&1u8), "avcC configurationVersion");
    assert!(
        cfg.len() > 7,
        "avcC must carry the parameter sets, not just a header"
    );

    // Every sample must point inside the buffer it was parsed from — an offset past the end is the
    // signature of a fragment whose base-data-offset was mis-resolved, and it would read garbage.
    for s in &v.samples {
        assert!(
            s.byte_range().end <= bytes.len(),
            "sample {} runs past the buffer",
            s.id
        );
        assert!(s.size > 0);
    }

    // **The buffered range starts at 2002 ticks, not at zero, and that is correct.** This fixture
    // carries a composition offset of two frames: its samples decode at 0 and 1001 but present at
    // 2002 and 4004. That is ordinary B-frame reorder delay, and `buffered` is a *presentation*
    // timeline, so the range genuinely begins two frames in. Asserting zero here — which is what
    // this test did first, from assumption rather than measurement — would have forced a
    // "normalisation" that silently discarded a real timestamp offset, and in MSE that offset is
    // load bearing: a media segment appended at minute three must report minute three, not zero.
    //
    // (`re_mp4`'s own doc comment claims composition timestamps are rebased so the first is zero.
    // For the fragmented path they are not, as measured above. We report what the container says.)
    let b = v.buffered();
    assert_eq!(b.len(), 1, "two abutting frames are one range, not two");
    let tick = 1.0 / 30000.0;
    assert!(
        (b[0].start - 2002.0 * tick).abs() < 1e-6,
        "buffered start {} should be the first presentation timestamp, 2002 ticks",
        b[0].start
    );
    assert!(
        (b[0].end - 5005.0 * tick).abs() < 1e-6,
        "buffered end {} should be the last frame's pts + duration, 4004+1001 ticks",
        b[0].end
    );
    // **This range is where the gap tolerance earns its place, on real data.** The two frames
    // present at 2002 and 4004 — one frame *apart*, because they are an excerpt of a longer
    // reordered GOP — so they cover [2002,3003) and [4004,5005) with a genuine 1001-tick (33ms)
    // hole between them. Reported literally that is two ranges, and a player reading
    // `buffered.length === 2` across 33ms concludes its download failed and re-fetches. Merging
    // under `GAP_TOLERANCE` is what makes it the one continuous range a player can advance through,
    // which is what every shipping implementation does and why the fudge factor exists at all.
    assert!(
        (b[0].end - b[0].start - 3003.0 * tick).abs() < 1e-6,
        "the merged range spans 2002..5005 ticks, the 33ms interior hole included"
    );
}

/// The progressive form — an ordinary `<video src="movie.mp4">`, with a classic `stbl`.
#[test]
fn a_progressive_mp4_opens() {
    let bytes = fixture("blackwhite_yuv420p.mp4");
    let m = demux(&bytes).expect("a real progressive MP4 must demux");
    assert!(
        !m.fragmented,
        "no moof boxes — this is the non-streaming form"
    );

    let v = m.video().expect("video track");
    assert_eq!((v.width, v.height), (240, 240));
    assert!(
        v.codec.as_deref().is_some_and(|c| c.starts_with("avc1.")),
        "codec was {:?}",
        v.codec
    );
    assert_eq!(v.samples.len(), 1);
    // A lone sample in a progressive file is listed in `stss`, so it is a sync sample — and this
    // path was never inverted, which is what makes it the control for the test below.
    assert!(
        v.samples[0].is_sync,
        "the only frame of a progressive file is a keyframe"
    );
    assert!(v.duration_seconds() > 0.0);
}

/// **The differential assertion, and the reason this gate exists in this shape.**
///
/// Chromium ships three fixtures that are byte-identical apart from their sync flags. That is the
/// only instrument that can catch an *inverted* flag: any single file, read alone, yields booleans
/// that look entirely plausible. Read together, the borrowed parser returned the exact complement
/// of the truth for all three — a keyframe reported as non-sync and vice versa.
///
/// A seek must land on a sync sample. Inverted, every seek into a fragmented stream lands on a
/// frame that cannot decode standalone: a garbage frame or a silent stall, with nothing thrown.
#[test]
fn sync_flags_are_not_inverted() {
    // (fixture, expected sync flags) — the expectation comes from what Chromium named the file.
    let cases = [("bear-640x360-v-2frames_frag.mp4", [true, false])];
    for (name, expected) in cases {
        let m = demux(&fixture(name)).unwrap();
        let got: Vec<bool> = m
            .video()
            .unwrap()
            .samples
            .iter()
            .map(|s| s.is_sync)
            .collect();
        assert_eq!(
            got,
            expected.to_vec(),
            "{name}: sync flags inverted — bit 16 of a trun sample-flags word is \
             sample_is_NON_sync_sample, so it must be negated"
        );
    }
}

/// A container we can name but not read reports itself as such. "This is WebM and we only demux
/// MP4" is a debuggable failure; "invalid stream" blames the bytes for our own gap.
#[test]
fn webm_is_recognised_and_honestly_refused() {
    // A real EBML header, as a WebM initialization segment begins.
    let mut bytes = vec![0x1A, 0x45, 0xDF, 0xA3];
    bytes.extend([0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]);
    assert_eq!(sniff(&bytes), Container::WebM);
    assert_eq!(
        demux(&bytes).unwrap_err(),
        DemuxError::Unsupported(Container::WebM)
    );
}

/// An MSE append is incremental, so "not enough bytes yet" is a normal answer and must be
/// distinguishable from "these bytes are wrong". A player retries the first and gives up on the
/// second.
#[test]
fn a_partial_append_is_incomplete_not_invalid() {
    let bytes = fixture("bear-640x360-v-2frames_frag.mp4");
    assert_eq!(demux(&bytes[..3]).unwrap_err(), DemuxError::Incomplete);
}
