//! **G_MEDIA_AAC — the engine can turn AAC into sound-shaped numbers.**
//!
//! Media step **M4**, against a real encoded file.
//!
//! **The failure this gate exists for.** M3 could find the audio and name it (`mp4a.67`, 44100 Hz,
//! stereo) and could not produce a single sample of it. Naming a codec and decoding it are
//! different claims, and the whole discipline of this track is refusing to let the first pass for
//! the second.
//!
//! **The assertion that makes this a decode gate rather than a "the function returned Ok" gate.**
//! The decoded PCM frame count must equal the track's **declared duration in its own timescale** —
//! 121856 units at a 44100 timescale is 121856 frames, exactly. Those two numbers come from
//! completely independent places: the duration from the container's `moov`/`trun` headers, the
//! frame count from summing what the AAC decoder actually emitted, packet by packet. A decoder that
//! silently dropped packets, doubled a buffer, mis-read the channel count, or returned early would
//! land on a different number. Getting them to agree to the sample is not something a stub can do.
//!
//! **RED, run:** decoding only the first packet (`break` after one) yields 1024 frames against
//! 121856 — off by the whole track.
//!
//! **Run it with `cargo test -p manuk-media --features audio`.** The decoder is behind an opt-in
//! feature and this target declares `required-features`, so a bare `cargo test -p manuk-media`
//! silently skips this file. That is a deliberate trade and it has a reason: cargo unifies features
//! across a workspace build, so a default-on decoder lands in every configuration that builds
//! `manuk-media` at all — including the `--no-default-features` headless check on `manuk-shell` —
//! and compiling symphonia there cost ~100s on EVERY wall. The WALL ratchet caught that and refused
//! the tick, correctly. The cost of the fix is this line of documentation.

use manuk_media::{can_decode_audio, decode_track, demux, DecodeError, TrackKind};

fn fixture(name: &str) -> Vec<u8> {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
    std::fs::read(format!("{p}{name}")).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

#[test]
fn a_real_aac_track_decodes_to_pcm_of_exactly_the_right_length() {
    let bytes = fixture("bear-mpeg2-aac-only_frag.mp4");
    let movie = demux(&bytes).expect("the AAC fixture must demux");

    let track = movie.audio().expect("an audio track must be found by kind");
    assert_eq!(track.kind, TrackKind::Audio);
    // `mp4a.67` — object type indication 0x67 is MPEG-2 AAC-LC, which is what this file is. Not
    // `mp4a.40.2`; that is the MPEG-4 spelling, and reporting one for the other would be a codec
    // string a player compares against and rejects.
    assert_eq!(track.codec.as_deref(), Some("mp4a.67"));
    assert_eq!(track.sample_rate, 44100);
    assert_eq!(track.channels, 2);
    assert_eq!(track.samples.len(), 119, "119 AAC packets");

    // The AudioSpecificConfig, rebuilt from the esds descriptor. Without it no packet can be
    // interpreted at all, so its absence is a decode failure and not a quality issue.
    let asc = track
        .codec_config
        .as_ref()
        .expect("AAC decode needs the AudioSpecificConfig");
    // AAC-LC (object type 2), frequency index 4 (44100), channel configuration 2:
    //   00010 0100 0010 000  =  0x12 0x10
    assert_eq!(asc.as_slice(), &[0x12, 0x10], "AudioSpecificConfig bits");

    assert!(can_decode_audio(track), "AAC must be reported as decodable");

    let pcm = decode_track(track, &bytes).expect("a real AAC track must decode");
    assert_eq!(pcm.sample_rate, 44100);
    assert_eq!(pcm.channels, 2);

    // **THE ASSERTION.** Frame count from the decoder, duration from the container headers — two
    // independent sources that must agree exactly.
    assert_eq!(
        pcm.frames() as u64,
        track.duration,
        "decoded PCM frames must equal the track's declared duration in timescale units \
         ({} at {}Hz); a mismatch means packets were dropped, doubled, or truncated",
        track.duration,
        track.timescale
    );
    assert_eq!(
        pcm.samples.len(),
        pcm.frames() * 2,
        "interleaved stereo: two samples per frame"
    );

    // ~2.76s of audio. Checked as a relationship against the timescale, not as a remembered number.
    let expected_seconds = track.duration as f64 / track.timescale as f64;
    assert!(
        (pcm.duration_seconds() - expected_seconds).abs() < 1e-6,
        "decoded duration {} should match the container's {expected_seconds}",
        pcm.duration_seconds()
    );

    // Real audio, not a buffer of zeros. A decoder that returned correctly-sized silence would
    // satisfy every assertion above — this is the one that says sound came out.
    let peak = pcm.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(
        peak > 0.01,
        "decoded PCM is silent (peak {peak}) — correctly-sized silence passes every length check"
    );
    assert!(
        pcm.samples.iter().all(|s| s.is_finite() && s.abs() <= 1.5),
        "samples must be finite and in range; NaN or a wild magnitude would be a device-blowing bug"
    );
}

/// The honest refusal. MP3, Opus, Vorbis, FLAC and AC-3 are all on the web and none are wired up,
/// so a video track — or any non-AAC audio — must be declined rather than accepted and then failed
/// mid-stream.
#[test]
fn a_track_we_cannot_decode_is_refused_up_front() {
    let bytes = fixture("bear-640x360-v-2frames_frag.mp4");
    let movie = demux(&bytes).unwrap();
    let video = movie.video().unwrap();

    assert!(!can_decode_audio(video), "a video track is not audio");
    match decode_track(video, &bytes) {
        Err(DecodeError::Unsupported(c)) => assert!(c.starts_with("avc1."), "named the codec: {c}"),
        other => panic!("expected an Unsupported refusal naming the codec, got {other:?}"),
    }
}
