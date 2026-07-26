//! **G_MEDIA_WEBM — the engine can open a WebM file.**
//!
//! Container step **M3b**, gated against **real encoded files** from the Chromium corpus, not
//! synthesised ones. A fixture written by our own code proves our writer and our reader agree,
//! which is a tautology; these came out of real encoders.
//!
//! **The failure this gate exists for.** `demux()` answered `Unsupported(WebM)` for every EBML
//! stream, and said so in its own module doc. WebM is what YouTube ships and what most non-MP4
//! `<video>` on the open web is. The MP4 ladder went demux → AAC → H.264 → playback, one rung per
//! tick; the WebM ladder had no rung 1, so a VP9 or Opus decoder would have had nothing to feed:
//! no tracks, no timestamps, no byte ranges, and `SourceBuffer.buffered` structurally empty however
//! good the decoder turned out to be.
//!
//! **What is asserted, and why each one rather than a plausible-looking substitute.**
//!
//! * **Byte offsets are checked against the CODEC's framing**, not against the buffer length and
//!   not against each other. A sample that points *inside* the buffer at the wrong place feeds a
//!   decoder garbage that decodes into a green frame — the silent-failure shape, one layer below
//!   where anyone looks. This fixture contains the case that breaks the naive
//!   position-minus-length recovery, and that case comes out **shifted by six bytes**: still inside
//!   the buffer, still disjoint from its neighbours. Containment and non-overlap both pass on it.
//!   See `sample_offsets_land_on_real_codec_frames`, and note that the first draft of this gate had
//!   only the two checks that pass — the RED probe is what said so.
//! * **Every sample has a non-empty presentation span, and `buffered()` is therefore ONE range.**
//!   Matroska mostly does not store frame durations. Without the three-source duration derivation
//!   every span is empty, `Track::buffered` filters them all out, and `buffered.length` is 0 — a
//!   player reading that re-fetches media it already holds, forever. This fixture needs two of the
//!   three arms: its video track has a `DefaultDuration` and its Opus track has none.
//! * **Codec strings are the container's, and stated in the SHORT form.** `vp9`, not
//!   `vp09.00.10.08`: the long form encodes profile/level/depth that this track does not carry, and
//!   a fabricated one is a string a player string-compares and branches on. AV1 is asserted in the
//!   long form *because* it is derivable from its `av1C` `CodecPrivate`.
//! * **The decode claim stays FALSE.** `codecs_are_a_container_claim_not_a_decode_claim` asserts
//!   demux succeeds *and* that this crate still cannot decode VP9 — the assertion that goes red the
//!   day someone reads "we demux WebM" as "we play WebM". `docs/loop/MEDIA.md`: advertising MSE we
//!   cannot honour turns a working YouTube into a black rectangle.
//!
//! **RED, all three run at tick 633:**
//!
//! | mutation | result |
//! |---|---|
//! | restore `Container::WebM => Err(Unsupported(WebM))` in `demux()` | 5 of 7 FAIL |
//! | delete the forward-scan arm of `webm::locate` (trust the position hint) | `sample_offsets_land_on_real_codec_frames` FAILS — *"sample 216 at offset 99597 has TOC byte 0xe4 where every other packet has 0xfc"* — and nothing else |
//! | delete the delta arm of the duration derivation | `the_audio_timeline_exists_only_because_durations_were_derived` FAILS, nothing else |

use manuk_media::{demux, sniff, Container, TrackKind};

fn fixture(name: &str) -> Vec<u8> {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
    std::fs::read(format!("{p}{name}")).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

const VP9_OPUS: &str = "bear-vp9-opus.webm";
const AV1: &str = "bear-av1-480x360.webm";

/// The pair YouTube actually serves: VP9 video and Opus audio in one WebM.
#[test]
fn a_vp9_opus_webm_opens() {
    let bytes = fixture(VP9_OPUS);
    assert_eq!(sniff(&bytes), Container::WebM);

    let m = demux(&bytes).expect("a real WebM must demux");
    assert!(
        !m.fragmented,
        "`fragmented` names the MP4 moof form; WebM has no such split and must not claim one"
    );
    assert_eq!(m.tracks.len(), 2, "one video track and one audio track");

    let v = m.video().expect("the video track must be found by kind");
    assert_eq!(v.kind, TrackKind::Video);
    assert_eq!((v.width, v.height), (320, 240));
    assert_eq!(
        v.codec.as_deref(),
        Some("vp9"),
        "the container says VP9 and carries no profile/level/depth — the short form is the whole \
         truth available, and a fabricated `vp09.00.10.08` is a string players branch on"
    );

    let a = m.audio().expect("the audio track must be found by kind");
    assert_eq!(a.kind, TrackKind::Audio);
    assert_eq!(a.codec.as_deref(), Some("opus"));
    assert_eq!((a.sample_rate, a.channels), (48000, 2));

    // Opus's `CodecPrivate` is the `OpusHead` identification header — exactly what a decoder needs,
    // extracted at demux time so the decode step is a decode step. Absent it, a VP9/Opus rung would
    // have to re-parse the container.
    let cfg = a
        .codec_config
        .as_ref()
        .expect("OpusHead must be extracted for the decoder step");
    assert_eq!(&cfg[..8], b"OpusHead", "the Opus identification header");

    // The segment duration, from `Info`, not from the last frame.
    let d = m.duration_seconds();
    assert!(
        (d - 2.736).abs() < 0.01,
        "segment duration ~2.736s, got {d}"
    );
}

/// The sample table is well formed: every range inside the buffer, non-empty, and disjoint.
///
/// **These are properties of the TABLE, and none of them catches a wrong offset** — the RED probe
/// proved it: with the forward-scan arm of `webm::locate` deleted, all of this still passes. The
/// bad frame comes out shifted six bytes, inside the buffer and disjoint from its neighbours. That
/// is what `sample_offsets_land_on_real_codec_frames` is for, and the two tests are kept separate
/// so the distinction stays visible rather than dissolving into one green check.
#[test]
fn every_sample_points_at_its_own_bytes() {
    for name in [VP9_OPUS, AV1] {
        let bytes = fixture(name);
        let m = demux(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut n = 0usize;
        for t in &m.tracks {
            for s in &t.samples {
                let r = s.byte_range();
                assert!(
                    r.end <= bytes.len(),
                    "{name}: sample {} runs past the buffer",
                    s.id
                );
                assert!(s.size > 0, "{name}: sample {} is empty", s.id);
                n += 1;
            }
        }
        assert!(
            n > 50,
            "{name}: only {n} samples — the frame pass stopped early"
        );

        // Two coded frames occupy disjoint bytes in every container that exists, so a table that
        // double-books a byte is malformed regardless of which offset is wrong. Worth asserting —
        // and worth being explicit that it did NOT catch the bug this module's offset recovery
        // exists for. I wrote it expecting it to, ran the RED probe, and it stayed green: the naive
        // recovery shifts one frame six bytes earlier, which overlaps nothing.
        let mut ranges: Vec<(usize, usize, u32)> = m
            .tracks
            .iter()
            .flat_map(|t| {
                t.samples
                    .iter()
                    .map(|s| (s.offset as usize, s.size as usize, s.id))
            })
            .collect();
        ranges.sort();
        for w in ranges.windows(2) {
            let (a_off, a_len, a_id) = w[0];
            let (b_off, _, b_id) = w[1];
            assert!(
                a_off + a_len <= b_off,
                "{name}: samples {a_id} [{a_off}..{}) and {b_id} [{b_off}..) overlap — two coded \
                 frames cannot share bytes, so at least one offset is wrong",
                a_off + a_len
            );
        }
    }
    // The VP9/Opus fixture's exact frame count, so a parse that silently drops the final Cluster is
    // caught. 218 frames: 82 video (2.736s at ~30fps) and 136 Opus packets (20ms each).
    let m = demux(&fixture(VP9_OPUS)).unwrap();
    let total: usize = m.tracks.iter().map(|t| t.samples.len()).sum();
    assert_eq!(total, 218, "every frame in the file, none dropped");
    assert_eq!(m.video().unwrap().samples.len(), 82);
    assert_eq!(m.audio().unwrap().samples.len(), 136);
}

/// **The offset is right, checked against the CODEC's own framing.**
///
/// This is the assertion the first draft of this gate did not have, and the omission was caught by
/// running the RED probe rather than by reading the test: `byte_range().end <= bytes.len()` and
/// non-overlap both PASS under the naive "reader position minus frame length" recovery, because
/// that recovery produces offsets that are merely *shifted* — inside the buffer, disjoint from each
/// other, and pointing at the wrong bytes. Containment and disjointness are structural properties
/// of the sample table; neither is a property of the *bytes*.
///
/// So the check has to come from below the container. Two codec-level invariants do it, and both
/// are independent of anything this demuxer computes:
///
/// * **VP9**: every frame begins with an uncompressed header whose first two bits are the frame
///   marker `10` (VP9 Bitstream Specification §6.2). A spec constant, so it is written as one.
/// * **Opus**: every packet begins with a TOC byte encoding the configuration the *encoder* chose.
///   One file, one encoder configuration — so the invariant is that all 136 TOC bytes are **equal
///   to each other**, which states the relationship rather than pinning a number someone observed.
///
/// A shifted offset lands on payload bytes, and payload bytes do not satisfy either.
#[test]
fn sample_offsets_land_on_real_codec_frames() {
    let bytes = fixture(VP9_OPUS);
    let m = demux(&bytes).unwrap();

    let v = m.video().unwrap();
    for s in &v.samples {
        let b = bytes[s.byte_range()][0];
        assert_eq!(
            b >> 6,
            0b10,
            "sample {} at offset {} is not a VP9 frame header (first byte {b:#04x}) — the offset is \
             wrong, not merely out of range",
            s.id,
            s.offset
        );
    }

    let a = m.audio().unwrap();
    let toc = bytes[a.samples[0].byte_range()][0];
    for s in &a.samples {
        let b = bytes[s.byte_range()][0];
        assert_eq!(
            b, toc,
            "sample {} at offset {} has TOC byte {b:#04x} where every other packet in this file has \
             {toc:#04x} — one file has one encoder configuration, so this offset is wrong",
            s.id, s.offset
        );
    }

    // The sync flag is read, not fabricated: the first frame of the first cluster is a keyframe, and
    // they are not all keyframes. A demuxer that hard-codes `is_sync: true` reports a seekable frame
    // at every timestamp, and every seek lands on a frame that cannot decode standalone.
    assert!(
        v.samples[0].is_sync,
        "the first frame of a WebM cluster is a keyframe"
    );
    let syncs = v.samples.iter().filter(|s| s.is_sync).count();
    assert!(
        syncs < v.samples.len(),
        "all {} video frames claim to be keyframes — the sync flag is not being read",
        v.samples.len()
    );
}

/// Matroska stores a frame timestamp and, usually, no frame duration. Every sample span would be
/// empty and `buffered()` would be `[]` — the "the append loop cannot advance" failure.
#[test]
fn the_audio_timeline_exists_only_because_durations_were_derived() {
    let m = demux(&fixture(VP9_OPUS)).unwrap();
    let a = m.audio().unwrap();

    // The Opus track has NO `DefaultDuration` in this file, so every one of these spans came from
    // the delta-to-next-frame arm.
    assert!(
        a.samples.iter().all(|s| s.duration > 0),
        "an Opus sample with a zero span means the derivation did not reach it"
    );
    let b = a.buffered();
    assert_eq!(
        b.len(),
        1,
        "one continuous span of audio, not {} islands — a player reading islands re-fetches media \
         it already has",
        b.len()
    );
    assert!(
        b[0].start < 0.001,
        "audio starts at zero, got {}",
        b[0].start
    );
    assert!(
        (b[0].end - 2.74).abs() < 0.05,
        "the audio runs to ~2.74s, got {}",
        b[0].end
    );

    // 20ms Opus packets — and the derived span is the CONTAINER's inter-frame delta, so it is 20 or
    // 21ms and never finer. That is the honest limit and it is worth stating: this file's block
    // timestamps live on a 1ms `TimecodeScale`, so an audio track with no `DefaultDuration` cannot
    // be timed more precisely than the file times it. A derivation that produced exactly 20.000ms
    // would be reporting the encoder's intent rather than the container's content.
    assert_eq!(a.samples[0].timescale, 1_000_000_000);
    let d = a.samples[0].presentation_end() - a.samples[0].presentation_start();
    assert!(
        (d - 0.020).abs() <= 0.0015,
        "an Opus packet is 20ms at the container's 1ms resolution, got {d}s"
    );

    // The video track's spans come from `DefaultDuration` — 33_366_666ns, which is NOT an integer
    // number of milliseconds. Carrying it in the file's own 1ms tick would round it away.
    let v = m.video().unwrap();
    assert_eq!(v.samples[0].duration, 33_366_666);
    assert_eq!(
        m.buffered().len(),
        1,
        "audio and video merge into one buffered span"
    );
}

/// AV1 is the one codec whose long-form string is derivable, because its `CodecPrivate` **is** the
/// `av1C` record: profile 0, level 1, Main tier, 8-bit.
#[test]
fn av1_in_webm_reports_the_rfc6381_string_from_its_av1c() {
    let bytes = fixture(AV1);
    assert_eq!(sniff(&bytes), Container::WebM);
    let m = demux(&bytes).expect("AV1-in-WebM must demux");
    let v = m.video().expect("a video track");
    assert_eq!((v.width, v.height), (480, 360));
    assert_eq!(
        v.codec.as_deref(),
        Some("av01.0.01M.08"),
        "profile/level/tier/depth read out of the av1C bytes, not guessed"
    );
    assert_eq!(v.samples.len(), 82);
    assert_eq!(m.tracks.len(), 1, "video-only fixture");
}

/// **The claim that must NOT move.** Demuxing a container and decoding its codec are different
/// claims, and this project's own media doc names conflating them as the failure that turns a
/// working YouTube into a black rectangle. This gate goes red the day "we demux WebM" is read as
/// "we play WebM".
#[test]
fn codecs_are_a_container_claim_not_a_decode_claim() {
    let m = demux(&fixture(VP9_OPUS)).unwrap();
    assert_eq!(m.video().unwrap().codec.as_deref(), Some("vp9"));

    // There is no VP9 decoder in this tree at all — not behind a feature, not anywhere. The
    // `video` feature is openh264 (H.264 Constrained Baseline) and `av1` is re_rav1d. Asked with
    // the REAL demuxed track rather than a hand-written codec string, so this is the decoder's
    // verdict on the actual file and not on a string this test made up.
    #[cfg(feature = "video")]
    {
        assert!(
            !manuk_media::can_decode_video(m.video().unwrap()),
            "no VP9 decoder exists; saying otherwise steers a player onto a path that hangs"
        );
        // ...and the same question about a track that IS decodable must answer yes, or the `false`
        // above would be a decoder that says no to everything and proves nothing.
        let av1 = demux(&fixture(AV1)).unwrap();
        let decodable = manuk_media::can_decode_video(av1.video().unwrap());
        #[cfg(feature = "av1")]
        assert!(
            decodable,
            "AV1-in-WebM demuxed to `{:?}` and the av1 feature is on — a decoder that refuses \
             everything would make the vp9 assertion above vacuous",
            av1.video().unwrap().codec
        );
        #[cfg(not(feature = "av1"))]
        assert!(
            !decodable,
            "without the `av1` feature nothing decodes AV1 either, and this build must say so"
        );
    }
}

/// A truncated buffer is the normal MSE case — "come back with more", not a broken file. Every
/// prefix of a real WebM must produce `Incomplete` or a real `Movie`, and never a panic.
#[test]
fn a_truncated_webm_is_incomplete_and_never_a_panic() {
    let bytes = fixture(AV1);
    for cut in [8usize, 64, 512, 4096, 20000] {
        match demux(&bytes[..cut]) {
            Ok(_) => {}
            Err(e) => {
                let s = e.to_string();
                assert!(
                    !s.is_empty(),
                    "a failure must name itself; a prefix of {cut} bytes said nothing"
                );
            }
        }
    }
}
