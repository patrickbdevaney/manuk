//! # mp3_decode — the raw-stream organ decodes real MPEG audio to the right length
//!
//! `<audio src="episode.mp3">` is a stream, not an MP4 track; this drives the probe path
//! (`decode_audio_stream`) on the Chromium bear-audio fixture and asserts CONTENT: a 10-second
//! file must decode to ten seconds of frames — a decoder that silently drops half its packets
//! passes any "it produced samples" check and fails the clock. The ID3-tagged variant proves the
//! metadata tag is SKIPPED (probed past), not parsed as sync and not mistaken for audio.

use manuk_media::{decode_audio_stream, sniff_mpeg_audio};

const CBR: &[u8] = include_bytes!("data/bear-audio-10s-CBR-no-TOC.mp3");
const ID3: &[u8] = include_bytes!("data/id3_png_test.mp3");

#[test]
fn mp3_decode() {
    // ── The sniff routes both shapes and refuses non-audio.
    assert!(sniff_mpeg_audio(CBR), "a raw MPEG stream must sniff true");
    assert!(sniff_mpeg_audio(ID3), "an ID3-tagged MP3 must sniff true");
    assert!(!sniff_mpeg_audio(b"GIF89a not audio"));
    assert!(!sniff_mpeg_audio(&[0x00, 0x61, 0x73, 0x6d]));

    // ── The 10-second fixture decodes to ~10 seconds of PCM.
    let pcm = decode_audio_stream(CBR).expect("the CBR fixture must decode");
    assert!(pcm.channels > 0 && pcm.sample_rate > 0);
    let dur = pcm.duration_seconds();
    assert!(
        (dur - 10.0).abs() < 0.5,
        "a 10s file must decode to ~10s of frames, got {dur}s — a decoder dropping packets \
         passes every 'produced samples' check and fails the clock"
    );
    assert!(
        pcm.samples.iter().any(|&s| s != pcm.samples[0]),
        "decoded PCM must be non-uniform (the mis-fed-decoder honesty guard)"
    );

    // ── The ID3v2 tag (which embeds a PNG here) is skipped, and the audio behind it decodes.
    let tagged = decode_audio_stream(ID3).expect("the ID3-tagged fixture must decode");
    assert!(
        tagged.duration_seconds() > 1.0,
        "the tag must be probed PAST — treating its bytes as sync kills the whole stream"
    );

    // ── Garbage is a named refusal, never a panic.
    assert!(decode_audio_stream(b"not audio at all").is_err());
}
