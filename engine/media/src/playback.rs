//! # The presentation clock — media step M6
//!
//! Decode (M5) turns samples into pictures. This decides **which picture is now**.
//!
//! **Nothing is borrowed here, and that is deliberate rather than an oversight.** Every other step
//! in this track binds a crate — `re_mp4` demuxes, `symphonia` decodes AAC, `openh264` decodes
//! H.264 — because a container parser and a codec are exactly the kind of large, adversarial,
//! well-specified thing that must never be hand-written. A presentation clock is the opposite: it
//! is small, it is entirely policy, and `docs/loop/MEDIA.md` (trap #9) records that no crate offers
//! one. So this module is ~200 lines of arithmetic and no dependency.
//!
//! ## The one decision that makes this correct or wrong: HOLD, never ROUND
//!
//! A video **holds** each frame until the next one is due. So the frame at time `t` is the **last**
//! frame whose presentation time is `<= t` — never the *nearest* one.
//!
//! This distinction is invisible at every frame boundary and wrong everywhere in between. At 30fps
//! a frame is due every 33.3ms; a nearest-frame lookup switches to frame N+1 at 16.7ms, so for the
//! entire second half of every frame interval it shows a picture the author has not reached yet.
//! Both implementations pass any assertion that samples the timeline exactly on frame boundaries,
//! which is the obvious way to write the test and the reason the gate deliberately samples
//! **between** them.
//!
//! ## Presentation order is not decode order
//!
//! Frames are sorted by presentation time before they are ever indexed. `openh264` is Constrained
//! Baseline and emits no B-frames today, so decode order and presentation order coincide and the
//! sort is a no-op on everything currently decodable — which is precisely why it has to be written
//! now rather than when it starts mattering. The moment a High-profile backend drops in behind
//! `VideoDecoder` (the trait exists for that reason), decode order stops being presentation order
//! and an index built in decode order silently plays the picture sequence scrambled. `Sample`
//! already carries both timestamps and its doc comment already says which one a player seeks
//! against; this is the consumer that honours it.
//!
//! ## The clock is separate from the frames, because the audio device owns time
//!
//! [`Transport`] holds a position and does not own the frames. MEDIA.md's A/V-sync rule is that the
//! **audio device clock is master** — video is slaved to it, because a dropped video frame is
//! invisible and a stretched audio sample is not. So the position must be settable from outside
//! (`Transport::seek`, and a future `sync_to_audio`), and advancing it by a wall-clock delta is the
//! *fallback* for a muted or video-only stream rather than the primary path. Keeping the position
//! out of the frame store is what leaves room for that.

use crate::video::{Frame, VideoDecoder, VideoError};
use crate::Track;

/// The codec ladder: which decoder reads this track. `avc1.*` (Baseline) is openh264; `av01.*`
/// is re_rav1d when the `av1` feature is compiled in. Everything else falls through to
/// [`crate::video::H264Decoder::new`], whose honesty guard names the codec in its refusal —
/// so a build without `av1` refuses AV1 with the same words it always did.
fn decoder_for(track: &Track) -> Result<Box<dyn VideoDecoder>, VideoError> {
    #[cfg(feature = "av1")]
    if crate::av1::can_decode(track) {
        return Ok(Box::new(crate::av1::Av1Decoder::new(track)?));
    }
    Ok(Box::new(crate::video::H264Decoder::new(track)?))
}

/// A decoded video track, indexed by presentation time.
///
/// Frames are held in presentation order. Decoding is eager: the fixtures and segments this runs on
/// are one GOP or a few seconds, and a lazy decoder would need to keep the decoder, the buffer and
/// the sample table alive together to answer a seek backwards — which is the design MSE's
/// `SourceBuffer` already implements one layer up.
#[derive(Debug, Clone)]
pub struct FrameTimeline {
    /// Sorted by `presentation_time`, ascending.
    frames: Vec<Frame>,
    duration: f64,
}

impl FrameTimeline {
    /// Decode every sample of `track` out of `buffer` and index the result by presentation time.
    ///
    /// A sample that the decoder consumes without emitting a frame is normal (it is buffering
    /// parameter sets or reference frames), not an error — the `None` case is skipped, exactly as
    /// `decode_first_frame` does.
    pub fn decode(track: &Track, buffer: &[u8]) -> Result<Self, VideoError> {
        let mut decoder = decoder_for(track)?;
        let mut frames: Vec<Frame> = Vec::new();

        for s in &track.samples {
            let range = s.byte_range();
            if range.end > buffer.len() {
                continue;
            }
            if let Some(frame) = decoder.decode_sample(&buffer[range], s.presentation_start())? {
                frames.push(frame);
            }
        }
        // The delayed tail — a queueing decoder (AV1) holds frames past the last sample, and
        // dropping them here truncates the end of every stream it decodes.
        frames.extend(decoder.finish()?);

        if frames.is_empty() {
            return Err(VideoError::Failed(
                "no sample in the track produced a frame".into(),
            ));
        }

        // Presentation order, not decode order. See the module note.
        frames.sort_by(|a, b| {
            a.presentation_time
                .partial_cmp(&b.presentation_time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // The track's own declared duration is authoritative when it has one: the last frame's
        // presentation time is when that frame STARTS, and the video runs until it ends. Falling
        // back to the last frame's timestamp would cut the final frame's display interval off the
        // end of the timeline and report `ended` one frame early.
        let declared = if track.duration > 0 {
            track.duration as f64 / track.timescale.max(1) as f64
        } else {
            0.0
        };
        let last_start = frames.last().map(|f| f.presentation_time).unwrap_or(0.0);
        let duration = declared.max(last_start);

        Ok(Self { frames, duration })
    }

    /// The frame on screen at `t` seconds — the **last** frame due at or before `t`.
    ///
    /// `None` only when `t` precedes the first frame's presentation time, which is a real state: a
    /// stream whose first sample starts at a non-zero offset has nothing to show before it, and
    /// answering with the first frame instead would show the video's opening picture during a gap
    /// the author left blank.
    pub fn frame_at(&self, t: f64) -> Option<&Frame> {
        // `partition_point` gives the count of frames due at or before `t`; the last of them is the
        // one being held. Binary search rather than a scan because this is called once per painted
        // frame, forever.
        let due = self.frames.partition_point(|f| f.presentation_time <= t);
        if due == 0 {
            None
        } else {
            self.frames.get(due - 1)
        }
    }

    /// Total presentation length, in seconds.
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// How many frames decoded.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Every frame, in presentation order.
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }
}

/// **The master clock** — the audio device's own count of what it has consumed.
///
/// `docs/loop/MEDIA.md` (trap #9) settles the policy: the audio device clock is master and video is
/// slaved to it, because **a dropped video frame is invisible and a stretched audio sample is not.**
/// The same trap entry is why `cpal` was chosen over `rodio` — `rodio`'s `Sink` *hides* the clock,
/// and the clock is the thing that is needed.
///
/// ## Why this holds an integer and not a position
///
/// An audio device reports **a count of sample frames it has consumed**. That count divided by the
/// sample rate is the position, exactly — it is a rational number and both parts are known. Storing
/// the integer and dividing on read is therefore *exact at every horizon*.
///
/// The obvious alternative — keep an `f64 position` and add `frames / sample_rate` on each callback
/// — is the same number for the first few seconds and accumulates rounding error forever after,
/// because `1024.0 / 44100.0` is not representable in binary floating point. At a typical 1024-frame
/// buffer that is ~43 additions a second, each carrying error, and the drift is one-directional
/// rather than cancelling. It is a bug that **cannot be caught by any short test** and shows up as
/// audio and video visibly parting company late in a long video — a lip-sync complaint nobody can
/// reproduce in the first minute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioClock {
    sample_rate: u32,
    frames_played: u64,
}

impl AudioClock {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate.max(1),
            frames_played: 0,
        }
    }

    /// Report sample frames consumed by the device — what a `cpal` output callback knows.
    pub fn submit(&mut self, frames: u64) {
        self.frames_played += frames;
    }

    /// The master position, in seconds. Exact: one division, never an accumulation.
    pub fn position(&self) -> f64 {
        self.frames_played as f64 / self.sample_rate as f64
    }

    pub fn frames_played(&self) -> u64 {
        self.frames_played
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// A seek moves the device's playhead — the count restarts from the new position.
    pub fn seek(&mut self, seconds: f64) {
        self.frames_played = (seconds.max(0.0) * self.sample_rate as f64).round() as u64;
    }
}

/// The transport state a `<video>` element exposes: position, playing, ended.
///
/// Deliberately holds no frames — see the module note on the audio clock being master.
#[derive(Debug, Clone, PartialEq)]
pub struct Transport {
    position: f64,
    duration: f64,
    playing: bool,
}

impl Transport {
    pub fn new(duration: f64) -> Self {
        Self {
            position: 0.0,
            duration: duration.max(0.0),
            playing: false,
        }
    }

    /// `HTMLMediaElement.currentTime`.
    pub fn position(&self) -> f64 {
        self.position
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// `!HTMLMediaElement.paused`.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// `HTMLMediaElement.ended` — the position has reached the end AND is not merely paused there.
    pub fn ended(&self) -> bool {
        self.position >= self.duration && self.duration > 0.0
    }

    /// `play()`. Playing from the end **rewinds first**, which is the spec's behaviour and the
    /// reason pressing play on a finished video restarts it rather than sitting inert at the end.
    pub fn play(&mut self) {
        if self.ended() {
            self.position = 0.0;
        }
        self.playing = true;
    }

    /// `pause()`. The position is kept: pausing is not stopping.
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// `currentTime = t`, clamped to the media. Seeking does not change whether it is playing —
    /// scrubbing a playing video leaves it playing.
    pub fn seek(&mut self, t: f64) {
        self.position = t.clamp(0.0, self.duration);
    }

    /// Advance by a wall-clock delta. A no-op while paused — that is the whole of "paused".
    ///
    /// Clamps at the duration and stops, so `ended` latches instead of the position running past
    /// the media forever.
    pub fn advance(&mut self, dt: f64) {
        if !self.playing || dt <= 0.0 {
            return;
        }
        self.position = (self.position + dt).min(self.duration);
        if self.position >= self.duration {
            self.playing = false;
        }
    }

    /// **Slave this clock to the audio device.** Use this instead of [`Transport::advance`] whenever
    /// there is an audio track; `advance` is the fallback for the muted and video-only case.
    ///
    /// This **SNAPS** — it assigns the audio position rather than blending toward it. Averaging the
    /// two clocks, or taking whichever is further along, would leave the video clock authoritative
    /// in part, and "partly authoritative" is precisely the state the audio-is-master rule exists to
    /// forbid: any video contribution to the position eventually has to be paid back by resampling
    /// audio, which is the one thing a listener can hear.
    ///
    /// The correction is therefore always applied to the *video* side. Whatever
    /// [`FrameTimeline::frame_at`] returns for the new position is the frame to show — if that skips
    /// frames the device got ahead and those frames are simply never displayed, and if it repeats a
    /// frame the device is behind and the current picture is held. Both are invisible; stretching
    /// the audio to avoid them would not be.
    pub fn sync_to_audio(&mut self, clock: &AudioClock) {
        self.position = clock.position().clamp(0.0, self.duration);
        if self.position >= self.duration && self.duration > 0.0 {
            self.playing = false;
        }
    }

    /// How far the transport is from the audio device, in seconds. Positive means video is ahead.
    ///
    /// Exposed for a future frame-drop policy: a player wants to know the size of the correction
    /// `sync_to_audio` is about to make, not merely that one happened.
    pub fn drift_from(&self, clock: &AudioClock) -> f64 {
        self.position - clock.position()
    }
}

/// **The join: demuxed bytes in, the picture that is on screen right now out.**
///
/// Every piece below this line already existed and was gated — [`FrameTimeline`] indexes frames by
/// presentation time, [`Transport`] holds a position, [`AudioClock`] is master, and
/// `Page::set_video_frame` paints RGBA into the box the poster occupied. What did not exist was
/// **anything that owned all of them at once.** Each gate drove the parts by hand, so the tree could
/// demonstrate every step of playback and could not play. This is the object a host actually holds.
///
/// ## It picks its own clock, and that choice is the reason it exists
///
/// MEDIA.md's rule is audio-is-master, but most `<video>` on the open web is muted or has no audio
/// track at all — for those, a master clock that never ticks would freeze the picture on frame one.
/// So [`VideoPlayer::tick`] takes a wall-clock delta *and* an optional device clock, and routes to
/// [`Transport::sync_to_audio`] when there is one and [`Transport::advance`] when there is not. A
/// caller cannot get this wrong by forgetting which case it is in, which is exactly what happens
/// when the choice is left at the call site of two similarly-named methods.
///
/// ## `frame()` answers while paused, and that is not an oversight
///
/// A paused video shows a picture. Gating the frame on `is_playing` would blank the element on
/// `pause()` and show nothing at all before `play()` — the first-frame-poster state every video on
/// the web sits in until it is clicked.
pub struct VideoPlayer {
    timeline: FrameTimeline,
    transport: Transport,
}

impl VideoPlayer {
    /// Decode a track's frames and arm the transport against the real decoded duration.
    ///
    /// The transport is built from [`FrameTimeline::duration`] rather than the container's declared
    /// duration: a partially-buffered stream has fewer frames than the header promises, and a
    /// transport that believes the header runs the position off the end of what can be shown and
    /// holds the last decoded frame while `ended` never latches.
    pub fn decode(track: &Track, buffer: &[u8]) -> Result<Self, VideoError> {
        let timeline = FrameTimeline::decode(track, buffer)?;
        let transport = Transport::new(timeline.duration());
        Ok(Self {
            timeline,
            transport,
        })
    }

    /// Advance playback. Pass the device clock when the stream has audio; `None` slaves the position
    /// to `dt` instead — see the type note on why this is one call and not two.
    pub fn tick(&mut self, dt: f64, audio: Option<&AudioClock>) {
        match audio {
            Some(clock) if self.transport.is_playing() => self.transport.sync_to_audio(clock),
            _ => self.transport.advance(dt),
        }
    }

    /// The picture on screen now — `None` only before the first frame is due.
    pub fn frame(&self) -> Option<&Frame> {
        self.timeline.frame_at(self.transport.position())
    }

    pub fn play(&mut self) {
        self.transport.play();
    }

    pub fn pause(&mut self) {
        self.transport.pause();
    }

    pub fn seek(&mut self, t: f64) {
        self.transport.seek(t);
    }

    pub fn transport(&self) -> &Transport {
        &self.transport
    }

    pub fn timeline(&self) -> &FrameTimeline {
        &self.timeline
    }
}
