//! # G_VIDEO_PLAYER — the object that actually plays, and the clock it chooses
//!
//! Media step **M6**, the join. Ticks 234-249 landed demux, AAC decode, H.264 decode, the frame
//! timeline and the A/V-sync rule — five green gates, each driving the parts **by hand**. Nothing
//! owned them together, so the tree could demonstrate every step of playback and could not play.
//! `VideoPlayer` is that owner, and this gate asserts it against the real fixture rather than a
//! synthetic timeline, because the whole point is that the pieces meet on actual decoded bytes.
//!
//! ## How each assertion here can go RED
//!
//! - **Playback moves the picture.** The frame at t=0 and the frame late in the fixture must be
//!   *different bytes*. RED, run: have `frame()` ignore the transport and return `frames()[0]` —
//!   every position assertion still passes (the transport is correct; it is simply not consulted)
//!   and the video shows one still picture forever, which is precisely the bug this join exists to
//!   prevent and is invisible to any test that only checks `currentTime`.
//!
//! - **A paused video still shows a picture.** RED, run: gate `frame()` on `is_playing()`. The
//!   element goes blank on `pause()` and shows nothing at all before the first `play()` — the
//!   poster-frame state every video on the web sits in until it is clicked.
//!
//! - **The clock choice is made by the player, not the caller.** With an `AudioClock` supplied the
//!   position must come from the device and the wall-clock `dt` must be *discarded*. RED, run: make
//!   `tick` always call `advance` and then `sync_to_audio` — the position ends up audio's anyway on
//!   this fixture, so the obvious assertion passes; asserting against a `dt` deliberately far larger
//!   than the audio advance is what catches it.
//!
//! - **A video-only stream still advances.** RED, run: make `tick` require an `AudioClock` (or
//!   treat `None` as "no clock, do not move"). The muted/audio-less case — most `<video>` on the
//!   open web — freezes on frame one while every audio-track test stays green.

#![cfg(feature = "video")]

use manuk_media::{demux, AudioClock, TrackKind, VideoPlayer};

/// Constrained Baseline H.264 + AAC-LC in ONE fragmented file — see `data/README.md`.
const AV: &[u8] = include_bytes!("data/bear-av-baseline_frag.mp4");

fn player() -> VideoPlayer {
    let movie = demux(AV).expect("the muxed fixture demuxes");
    let video = movie
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Video)
        .expect("the fixture has a video track");
    VideoPlayer::decode(video, AV).expect("Constrained Baseline decodes")
}

#[test]
fn g_video_player() {
    let mut p = player();

    // The fixture must have enough frames for "the picture changed" to mean anything. A one-frame
    // timeline would make every assertion below vacuously true.
    let n = p.timeline().len();
    assert!(
        n >= 2,
        "the fixture must decode at least 2 frames to prove playback moves; got {n}"
    );
    let duration = p.transport().duration();
    assert!(duration > 0.0, "a decoded timeline has a real duration");

    // ── A picture before play() — the poster-frame state, not a blank element.
    let first = p
        .frame()
        .expect("a frame is on screen at t=0 before play()")
        .rgba
        .clone();
    assert!(
        !p.frame().unwrap().is_uniform(),
        "a decoded frame carries an image, not a flat field of one colour"
    );

    // ── Video-only path: no audio clock, the wall clock drives, and the PICTURE CHANGES.
    p.play();
    p.tick(duration, None);
    assert!(
        p.transport().position() > 0.0,
        "a stream with no audio track must still advance on the wall clock"
    );
    let last = p
        .frame()
        .expect("a frame is on screen at the end")
        .rgba
        .clone();
    assert_ne!(
        first, last,
        "playing to the end must show a DIFFERENT picture — a frame() that ignores the \
         transport passes every clock assertion and plays one still image forever"
    );

    // ── Paused videos show a picture.
    p.pause();
    assert!(
        p.frame().is_some(),
        "a paused video still shows its current frame"
    );

    // ── Audio-master path: the device clock wins and the wall-clock dt is discarded.
    let mut q = player();
    q.play();
    q.seek(0.0);
    let mut clock = AudioClock::new(44_100);
    clock.submit(4_410); // exactly 0.1s of audio consumed by the device
                         // A dt an order of magnitude larger than the audio advance: if `advance` contributes at all,
                         // the position lands far past 0.1s and this fails.
    q.tick(1.0, Some(&clock));
    let pos = q.transport().position();
    assert!(
        (pos - 0.1).abs() < 1e-9,
        "with an audio device present the position IS the device's, and the wall-clock delta is \
         discarded entirely; got {pos}"
    );
}
