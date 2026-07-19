//! # G_AV_SYNC — audio is master, and the master clock does not accumulate floats
//!
//! Media step **M6**, second half. Tick 249 built a clock that advances by a wall-clock delta —
//! which `docs/loop/MEDIA.md` (trap #9) names as the *fallback*, correct for the muted/video-only
//! case that is most of the open web's `<video>`, and wrong the moment there is an audio track.
//!
//! This runs on the tree's **first multi-track fixture**. Every other fixture carries exactly one
//! track, so nothing had ever demuxed a real fragment with both a video and an audio `traf`.
//!
//! ## How each assertion here can go RED
//!
//! - **The master clock is exact.** `AudioClock` stores the integer sample-frame count the device
//!   reports and divides once. RED, run: keep an `f64 position` and add `frames / sample_rate` per
//!   callback instead — the accumulation assertion below fails while every short-horizon assertion
//!   stays green, which is the entire problem with the bug. `1024/44100` is not representable in
//!   binary floating point, the error is one-directional, and at ~43 callbacks a second it surfaces
//!   as lip-sync drift late in a long video that nobody can reproduce in the first minute.
//!
//! - **Sync SNAPS to audio.** RED, run: make `sync_to_audio` average the two clocks, or take
//!   `max(video, audio)` — both leave the video clock authoritative in part, and the assertion that
//!   a deliberately-wrong video position is *fully* discarded fails.
//!
//! - **The correction lands on video, never on audio.** Asserted by construction: the audio clock is
//!   taken by `&` and cannot be written by the sync path at all. RED, run: take it by `&mut` and
//!   nudge `frames_played` toward the transport — it compiles, it "fixes" drift, and it is the one
//!   correction a listener can hear.

use manuk_media::{demux, AudioClock, FrameTimeline, TrackKind, Transport};

/// Constrained Baseline H.264 + AAC-LC in ONE fragmented file — see `data/README.md`.
const AV: &[u8] = include_bytes!("data/bear-av-baseline_frag.mp4");

const FRAME_INTERVAL: f64 = 1001.0 / 30000.0;

fn tracks() -> (manuk_media::Track, manuk_media::Track) {
    let m = demux(AV).expect("the muxed fixture demuxes");
    let v = m
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Video)
        .expect("video track")
        .clone();
    let a = m
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Audio)
        .expect("audio track")
        .clone();
    (v, a)
}

#[test]
fn one_file_yields_both_a_video_timeline_and_real_pcm() {
    let (v, a) = tracks();

    assert!(
        manuk_media::can_decode_video(&v),
        "muxing must not have changed the profile; codec was {:?}",
        v.codec
    );
    assert!(manuk_media::can_decode_audio(&a));

    let tl = FrameTimeline::decode(&v, AV).expect("video decodes out of the muxed file");
    let pcm = manuk_media::decode_track(&a, AV).expect("audio decodes out of the muxed file");

    assert_eq!(tl.len(), 3, "3 video frames");
    assert_eq!(pcm.sample_rate, 44100);
    assert_eq!(pcm.channels, 2);
    assert_eq!(
        pcm.frames(),
        4096,
        "the AAC track's declared duration in its own timescale IS the PCM frame count"
    );

    // The two tracks do NOT have the same duration, and that is the realistic case rather than a
    // defect in the fixture: 0.1001s of video against 0.0929s of audio. A player must survive one
    // stream ending before the other.
    assert!(
        (tl.duration() - 0.1001).abs() < 1e-6,
        "video duration {}",
        tl.duration()
    );
    assert!(
        (pcm.duration_seconds() - 4096.0 / 44100.0).abs() < 1e-9,
        "audio duration {}",
        pcm.duration_seconds()
    );
    assert!(
        pcm.duration_seconds() < tl.duration(),
        "this fixture's audio is SHORTER than its video — the assertions below depend on it"
    );
}

#[test]
fn the_master_clock_is_exact_at_every_horizon() {
    let mut clock = AudioClock::new(44100);
    assert_eq!(clock.position(), 0.0);

    // One typical device buffer.
    clock.submit(1024);
    assert!(
        (clock.position() - 1024.0 / 44100.0).abs() < 1e-15,
        "one buffer in, position must be the exact rational, got {}",
        clock.position()
    );

    // ── THE ASSERTION A SHORT TEST CANNOT MAKE: an hour of callbacks, exactly.
    //    ~43 callbacks a second for 3600s. An f64 accumulator adds `1024/44100` (not representable
    //    in binary) 155k times with one-directional error; the integer count is exact forever.
    let mut clock = AudioClock::new(44100);
    let buffers: u64 = 155_000;
    for _ in 0..buffers {
        clock.submit(1024);
    }
    let expected = (buffers * 1024) as f64 / 44100.0;
    assert_eq!(
        clock.frames_played(),
        buffers * 1024,
        "the device's own count must be carried as an integer"
    );
    assert!(
        (clock.position() - expected).abs() < 1e-9,
        "after {buffers} buffers (~1 hour) the position must still be exact: got {}, want {expected}",
        clock.position()
    );

    // A seek moves the device playhead rather than being tracked alongside it.
    clock.seek(10.0);
    assert_eq!(clock.frames_played(), 441_000);
    assert!((clock.position() - 10.0).abs() < 1e-12);
    clock.seek(-1.0);
    assert_eq!(clock.frames_played(), 0, "a seek before zero clamps");
}

#[test]
fn syncing_snaps_the_transport_to_audio_and_never_the_reverse() {
    let (v, _a) = tracks();
    let tl = FrameTimeline::decode(&v, AV).expect("video decodes");
    let mut t = Transport::new(tl.duration());
    let mut clock = AudioClock::new(44100);

    t.play();

    // Put the transport somewhere deliberately WRONG — as if the wall clock had run ahead while the
    // device was still filling its first buffer.
    t.advance(FRAME_INTERVAL * 2.0);
    let video_said = t.position();
    assert!(video_said > 0.0, "the wall clock did move");

    // The device has consumed exactly one frame interval's worth of audio.
    clock.submit((FRAME_INTERVAL * 44100.0).round() as u64);
    let audio_says = clock.position();

    // Drift is visible BEFORE the correction — a player wants the size of the correction, not just
    // the fact of one.
    assert!(
        (t.drift_from(&clock) - (video_said - audio_says)).abs() < 1e-12,
        "drift must report video-minus-audio"
    );

    t.sync_to_audio(&clock);

    // ── THE DISCRIMINATOR: the video position is discarded ENTIRELY, not blended.
    assert!(
        (t.position() - audio_says).abs() < 1e-12,
        "sync must SNAP to the audio clock: got {}, audio said {audio_says}, video had said \
         {video_said}. An average would land at {}, and max() would leave {video_said} — both keep \
         the video clock authoritative in part, which is exactly what audio-is-master forbids.",
        t.position(),
        (video_said + audio_says) / 2.0
    );
    assert!(
        t.drift_from(&clock).abs() < 1e-12,
        "after syncing there is no drift left"
    );

    // ── THE TWO TIMESCALES ARE INCOMMENSURATE, and this assertion was FIRST WRITTEN WRONG.
    //
    // The obvious claim is "submit one frame interval of audio and frame 1 is on screen". It is
    // false. 44100 Hz and 30000 Hz share no useful factor: one frame interval is 1471.47 audio
    // samples, a device delivers whole samples, and 1471 samples lands **0.47 samples SHORT** of
    // frame 1's presentation time. So frame 0 is still held — which is `frame_at` being exactly
    // right, not a rounding bug to paper over.
    //
    // The lesson generalises past this fixture: an audio clock can never be assumed to land ON a
    // video frame boundary, so any sync policy phrased as "when the clocks are equal" is a policy
    // that fires rarely and by luck. Hold semantics are what make the incommensurability harmless.
    let just_under = tl.frame_at(t.position()).expect("a frame is due");
    assert!(
        (just_under.presentation_time - 0.0).abs() < 1e-9,
        "1471 whole samples is 0.47 samples short of frame 1, so frame 0 is still held; got pts={}",
        just_under.presentation_time
    );

    // Past the boundary by a clear margin, the frame shown follows the AUDIO position — the
    // correction landed on the video side.
    clock.submit((FRAME_INTERVAL * 0.5 * 44100.0).round() as u64);
    t.sync_to_audio(&clock);
    let shown = tl.frame_at(t.position()).expect("a frame is due");
    assert!(
        (shown.presentation_time - FRAME_INTERVAL).abs() < 1e-6,
        "at 1.5 frame intervals of audio, frame 1 is on screen, got pts={}",
        shown.presentation_time
    );

    // Frames the device outran are simply never shown — invisible, and the correct trade.
    clock.seek(FRAME_INTERVAL * 2.5);
    t.sync_to_audio(&clock);
    let shown = tl.frame_at(t.position()).expect("a frame is due");
    assert!(
        (shown.presentation_time - FRAME_INTERVAL * 2.0).abs() < 1e-6,
        "jumping the audio clock forward skips straight to the frame due there, got pts={}",
        shown.presentation_time
    );
}

#[test]
fn audio_running_out_before_video_does_not_strand_the_transport() {
    let (v, a) = tracks();
    let tl = FrameTimeline::decode(&v, AV).expect("video decodes");
    let pcm = manuk_media::decode_track(&a, AV).expect("audio decodes");

    let mut t = Transport::new(tl.duration());
    let mut clock = AudioClock::new(pcm.sample_rate);
    t.play();

    // Play every sample the audio track has. It ends at 0.0929s; the video runs to 0.1001s.
    clock.submit(pcm.frames() as u64);
    t.sync_to_audio(&clock);

    assert!(
        (t.position() - pcm.duration_seconds()).abs() < 1e-9,
        "the transport follows audio to the end of the audio"
    );
    assert!(
        !t.ended(),
        "audio ending is not the media ending — {}s of video remain",
        tl.duration() - pcm.duration_seconds()
    );
    assert!(t.is_playing(), "the video must keep playing past the audio");

    // The last stretch is video-only, so the wall clock takes over — the fallback path, used for
    // exactly the reason MEDIA.md gives it.
    t.advance(1.0);
    assert!(
        (t.position() - tl.duration()).abs() < 1e-9,
        "the wall clock carries the remaining video to the end"
    );
    assert!(t.ended(), "now the media has ended");
}
