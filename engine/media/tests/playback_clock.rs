//! # G_PLAYBACK_CLOCK — a still is not playback: the clock that decides which frame is now
//!
//! Media step **M6**. M5 proved one frame decodes. This proves the track becomes a *sequence on a
//! timeline* — which is the difference between a poster replacement and a video that plays.
//!
//! ## How each assertion here can go RED
//!
//! - **Frames differ.** The load-bearing claim, and the one a frame *count* cannot make: a decoder
//!   that emits the same picture three times, or that re-emits its first frame because the decode
//!   loop never advances, produces a timeline of the right length that plays a still. RED, run:
//!   have `FrameTimeline::decode` push `frames[0].clone()` for every sample and every count,
//!   duration, ordering and dimension assertion below still passes.
//!
//! - **HOLD, not ROUND** — the assertion the module was written around. `frame_at` must return the
//!   *last* frame due at or before `t`, never the *nearest*. This fixture's frames are 33.367ms
//!   apart, so `t = 0.025` is 75% of the way through frame 0's interval: a hold answers frame 0, a
//!   nearest-frame lookup answers frame 1 — showing the author's next picture ~8ms early, and for
//!   the whole back half of every frame interval in the stream. RED, run: replace the
//!   `partition_point` body with a nearest-by-absolute-difference scan and ONLY this assertion
//!   fails — every boundary-sampled claim stays green, which is exactly why the sample points here
//!   are deliberately *between* frames rather than on them.
//!
//! - **Nothing before the first frame.** RED, run: return `self.frames.first()` instead of `None`
//!   for `due == 0`.
//!
//! - **`ended` latches and `play()` rewinds.** RED, run: drop the `min(duration)` clamp in
//!   `advance` and the position runs past the media forever, so a player's progress bar overruns
//!   and `ended` never settles.

use manuk_media::{demux, FrameTimeline, TrackKind, Transport};

/// Constrained Baseline, 640x360, 3 frames at 30000 ticks/s — see `data/README.md`.
const BASELINE: &[u8] = include_bytes!("data/bear-baseline_frag.mp4");

/// The fixture's frame interval: 1001 ticks at 30000 Hz.
const FRAME_INTERVAL: f64 = 1001.0 / 30000.0;

fn video_track(bytes: &[u8]) -> manuk_media::Track {
    let movie = demux(bytes).expect("fixture demuxes");
    movie
        .tracks
        .into_iter()
        .find(|t| t.kind == TrackKind::Video)
        .expect("fixture has a video track")
}

fn timeline() -> FrameTimeline {
    FrameTimeline::decode(&video_track(BASELINE), BASELINE).expect("baseline fixture decodes")
}

#[test]
fn the_track_becomes_a_sequence_of_distinct_pictures_in_presentation_order() {
    let tl = timeline();

    assert_eq!(
        tl.len(),
        3,
        "the fixture carries 3 coded samples and each must yield a frame"
    );

    // The container's own arithmetic: 3003 ticks at 30000 Hz.
    assert!(
        (tl.duration() - 0.1001).abs() < 1e-6,
        "duration must come from the track's declared length, got {}",
        tl.duration()
    );

    // Strictly increasing presentation time — an index built in decode order would not be sorted
    // once a B-frame backend drops in behind `VideoDecoder`.
    let pts: Vec<f64> = tl.frames().iter().map(|f| f.presentation_time).collect();
    for w in pts.windows(2) {
        assert!(
            w[1] > w[0],
            "frames must be indexed in presentation order, got {pts:?}"
        );
    }

    // ── THE CLAIM A FRAME COUNT CANNOT MAKE: the pictures are DIFFERENT.
    //    Three copies of one frame is a timeline of the right length that plays a still.
    for (i, w) in tl.frames().windows(2).enumerate() {
        assert!(
            !w[0].is_uniform(),
            "frame {i} is a flat field — a mis-fed decoder's output, not a picture"
        );
        let differing = w[0]
            .rgba
            .iter()
            .zip(w[1].rgba.iter())
            .filter(|(a, b)| a != b)
            .count();
        // The threshold is CALIBRATED, not guessed. Measured on this fixture: pair 0->1 differs in
        // **60.4%** of bytes, pair 1->2 in **0.86%**. That spread is the point — 33ms apart in a
        // slow-panning scene is genuinely a small delta, and a first guess of "more than 1% must
        // differ" FAILED here on real, correct video. The failure mode being caught is a decoder
        // re-emitting one picture, which yields EXACTLY ZERO differing bytes, so the honest bar is
        // a floor well above zero and well below real motion: 0.1%, which pair 1->2 clears 8x.
        assert!(
            differing > w[0].rgba.len() / 1000,
            "frames {i} and {} are all but identical ({differing} of {} bytes differ) — \
             a decoder re-emitting one picture produces a sequence that PLAYS A STILL, and \
             every count, duration and ordering assertion above passes anyway",
            i + 1,
            w[0].rgba.len()
        );
    }
}

#[test]
fn a_frame_is_held_until_the_next_one_is_due_never_rounded_to_the_nearest() {
    let tl = timeline();
    let pts: Vec<f64> = tl.frames().iter().map(|f| f.presentation_time).collect();

    // Exactly on each boundary — the sample points that CANNOT tell hold from round.
    for (i, &t) in pts.iter().enumerate() {
        let f = tl.frame_at(t).expect("a frame is due at its own timestamp");
        assert!(
            (f.presentation_time - t).abs() < 1e-9,
            "at t={t} the frame due is {i}, got pts={}",
            f.presentation_time
        );
    }

    // ── THE DISCRIMINATOR: 75% of the way through frame 0's interval.
    //    hold -> frame 0.  nearest -> frame 1 (it crossed the halfway point at ~16.7ms).
    let late_in_frame_0 = FRAME_INTERVAL * 0.75;
    let held = tl
        .frame_at(late_in_frame_0)
        .expect("frame 0 is still on screen");
    assert!(
        (held.presentation_time - pts[0]).abs() < 1e-9,
        "at t={late_in_frame_0:.6} (75% through frame 0's interval) the screen must still show \
         FRAME 0 — got pts={}. A nearest-frame lookup answers frame 1 here and shows the author's \
         next picture early for the back half of every frame interval in the stream.",
        held.presentation_time
    );

    // One microsecond before frame 1 is due, frame 0 is still up.
    let just_before = pts[1] - 1e-6;
    assert!(
        (tl.frame_at(just_before).unwrap().presentation_time - pts[0]).abs() < 1e-9,
        "frame 0 must be held right up to the instant frame 1 is due"
    );

    // Nothing precedes the first frame — a gap the author left blank is not the opening picture.
    assert!(
        tl.frame_at(-0.001).is_none(),
        "no frame is due before the first sample's presentation time"
    );

    // Past the end, the last frame stays on screen — a finished video does not go black.
    assert!(
        (tl.frame_at(999.0).unwrap().presentation_time - pts[2]).abs() < 1e-9,
        "the last frame is held past the end of the media"
    );
}

#[test]
fn the_transport_plays_pauses_seeks_and_ends() {
    let tl = timeline();
    let mut t = Transport::new(tl.duration());

    assert_eq!(t.position(), 0.0);
    assert!(!t.is_playing(), "a video does not autoplay");
    assert!(!t.ended());

    // Paused: the clock does not move. That is the whole of "paused".
    t.advance(0.05);
    assert_eq!(t.position(), 0.0, "advancing while paused must be a no-op");

    t.play();
    assert!(t.is_playing());

    // Advance across the first frame boundary and the picture on screen changes — the two halves
    // of this tick (a clock and an index) meeting.
    t.advance(FRAME_INTERVAL * 1.5);
    let showing = tl.frame_at(t.position()).expect("mid-stream frame");
    assert!(
        (showing.presentation_time - FRAME_INTERVAL).abs() < 1e-6,
        "after advancing 1.5 frame intervals the second frame must be on screen, got pts={}",
        showing.presentation_time
    );

    t.pause();
    let held = t.position();
    t.advance(1.0);
    assert_eq!(t.position(), held, "a paused clock stays where it was");

    // Seeking does not change whether it is playing — scrubbing a paused video leaves it paused.
    t.seek(0.0);
    assert_eq!(t.position(), 0.0);
    assert!(!t.is_playing());

    // Run off the end: the position clamps and `ended` latches instead of overrunning.
    t.play();
    t.advance(999.0);
    assert!(
        (t.position() - tl.duration()).abs() < 1e-9,
        "the position clamps at the duration, got {}",
        t.position()
    );
    assert!(t.ended(), "reaching the end must latch `ended`");
    assert!(!t.is_playing(), "playback stops at the end");

    // Pressing play on a finished video restarts it rather than sitting inert at the end.
    t.play();
    assert_eq!(t.position(), 0.0, "play() from the end rewinds");
    assert!(t.is_playing());
    assert!(!t.ended());

    // A seek past the end clamps rather than escaping the media.
    t.seek(999.0);
    assert!((t.position() - tl.duration()).abs() < 1e-9);
    t.seek(-5.0);
    assert_eq!(t.position(), 0.0);
}
