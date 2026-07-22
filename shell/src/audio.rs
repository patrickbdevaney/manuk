//! # Audio output — the device end of the media pipeline (tick 350)
//!
//! Decode (M4) proved AAC turns into correct PCM and stopped there, deliberately: a sound card is
//! not headlessly gateable, and `engine/media/src/audio.rs` records that bundling the device with
//! the decode would mean the decode could only be proven by listening to it. This module is the
//! other half, split along the same line:
//!
//! - [`AudioFeed`] is the **pump** — pure arithmetic that hands decoded samples to whoever asks,
//!   in whatever chunk sizes they ask for. Everything that can be got wrong (a dropped sample at
//!   a chunk boundary, a pause that drifts, a restart on an MSE re-decode) lives here, where a
//!   test can drive it. The gate asserts sample-exact delivery; it never needs a device.
//! - [`AudioOut`] is the **device** — `cpal` (BORROWED, per the standing rule), opened
//!   best-effort. On a box with no output device (headless CI) it is `None` and the browser plays
//!   video silently, which is degraded and honest; it is never a crash and never a gate.
//!
//! ## The callback owns the deadline, so the pump must never block on anything slow
//!
//! `cpal` calls [`AudioFeed::fill`] from the device's real-time thread. A miss is an audible
//! glitch. So the feed holds fully-decoded PCM (decode already happened on the load path) and
//! `fill` is a `memcpy` and a cursor add — no decode, no allocation, no I/O behind the lock.
//!
//! ## Silence is a contract, not an accident
//!
//! Every path that does not deliver samples — paused, exhausted, lock poisoned — must **write
//! zeros** into the whole buffer. The device plays whatever is in that buffer; leaving it
//! untouched replays stale samples from the last callback, which is heard as a stutter-loop. The
//! gate pre-fills buffers with garbage and asserts they come back zeroed.

use manuk_media::Pcm;

/// Decoded PCM plus a read cursor: the single source of what the device plays next.
///
/// Shared as `Arc<Mutex<AudioFeed>>` between the owner ([`crate::media::MediaSet`], which loads
/// and re-loads streams) and the device callback. The Arc identity is load-bearing across MSE
/// re-decodes: the device stream captured its clone at open time, so a re-decode must mutate the
/// feed **in place** ([`AudioFeed::replace_pcm`]) rather than swap in a fresh Arc the device has
/// never heard of.
pub struct AudioFeed {
    /// Interleaved, `channels` per frame — the layout a device consumes (see `Pcm::samples`).
    samples: Vec<f32>,
    channels: u16,
    sample_rate: u32,
    /// Next sample (not frame) to hand out. Always frame-aligned: `fill` advances by whole
    /// copies and `seek_seconds` rounds to a frame boundary.
    cursor: usize,
    playing: bool,
    /// Linear gain 0..1 (tick 360) — the `.volume` IDL property's landing point. Applied to
    /// the samples actually delivered; the muted/paused/exhausted silence contract is UPSTREAM
    /// of gain and never scaled (zeros times anything must stay written zeros).
    gain: f32,
    /// **Mute is silent CONSUMPTION, never pause** (tick 352). A muted `fill` advances the
    /// cursor exactly as an audible one and zeros the buffer — so the device clock keeps
    /// running (the A/V mastery rule still governs while muted) and unmute is seamless and IN
    /// SYNC, because the cursor is where the sound would have been. Mute-as-pause fails both
    /// ways: the master freezes, and unmute resumes stale audio from where it was muted,
    /// desynced by the whole muted interval.
    muted: bool,
}

impl AudioFeed {
    /// A feed starts playing, matching the autoplay stance of the video side (`MediaSet::load`):
    /// nothing yet routes a click to `play()`, so a feed that waited could never be started.
    pub fn new(pcm: Pcm) -> Self {
        Self {
            samples: pcm.samples,
            channels: pcm.channels,
            sample_rate: pcm.sample_rate,
            cursor: 0,
            playing: true,
            gain: 1.0,
            muted: false,
        }
    }

    /// Hand the device its next buffer. Returns how many samples were **consumed**; the remainder
    /// of `out` — all of it, when paused or exhausted — is zeroed (see the module note on
    /// silence). A muted feed consumes at full rate and delivers only zeros — see the field note
    /// on why mute must never be pause.
    pub fn fill(&mut self, out: &mut [f32]) -> usize {
        if !self.playing || self.cursor >= self.samples.len() {
            out.fill(0.0);
            return 0;
        }
        let n = out.len().min(self.samples.len() - self.cursor);
        if self.muted {
            out.fill(0.0);
        } else {
            out[..n].copy_from_slice(&self.samples[self.cursor..self.cursor + n]);
            if self.gain != 1.0 {
                for s in &mut out[..n] {
                    *s *= self.gain;
                }
            }
            out[n..].fill(0.0);
        }
        self.cursor += n;
        n
    }

    pub fn set_playing(&mut self, playing: bool) {
        self.playing = playing;
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// `.volume` — clamped to 0..1; the page-side accessor clamps too, but the device boundary
    /// re-clamps because a gain above 1 CLIPS and a negative gain inverts phase.
    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain.clamp(0.0, 1.0);
    }

    pub fn gain(&self) -> f32 {
        self.gain
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Where playback stands, in seconds of consumed frames. This is the device-side clock the
    /// A/V-sync rule ("audio is master") will eventually slave the video transport to.
    pub fn position_seconds(&self) -> f64 {
        if self.channels == 0 || self.sample_rate == 0 {
            return 0.0;
        }
        (self.cursor / self.channels as usize) as f64 / self.sample_rate as f64
    }

    /// Jump to `t`, rounded to a frame boundary — a cursor between two channels of one frame
    /// would swap left and right for the rest of the track.
    pub fn seek_seconds(&mut self, t: f64) {
        if self.channels == 0 || self.sample_rate == 0 {
            return;
        }
        let frame = (t.max(0.0) * self.sample_rate as f64).round() as usize;
        let total_frames = self.samples.len() / self.channels as usize;
        self.cursor = frame.min(total_frames) * self.channels as usize;
    }

    /// The stream GREW (an MSE append re-decoded to a longer timeline): take the new PCM but keep
    /// the cursor and play state — the resume rule `MediaSet::load_mse` established for video,
    /// because a feed that restarted here is audio that never gets past its own opening. A
    /// *shorter* replacement clamps the cursor to the end rather than reading out of bounds.
    pub fn replace_pcm(&mut self, pcm: Pcm) {
        self.samples = pcm.samples;
        self.channels = pcm.channels;
        self.sample_rate = pcm.sample_rate;
        self.cursor = self.cursor.min(self.samples.len());
    }

    pub fn exhausted(&self) -> bool {
        self.cursor >= self.samples.len()
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

/// The feed-SET one device stream mixes (tick 370). Shared between the GUI (which adds each
/// element's feed as media loads and clears it on navigation) and the device callback (which
/// sums every contained feed into the output buffer).
///
/// Mixing is a SUM with a hard clamp to [-1, 1] — honest and artifact-free at the two-or-three
/// simultaneous streams a real page plays. Feeds whose rate/channels mismatch the stream config
/// are SKIPPED, not misread: pulling 48k samples at a 44.1k device rate is a pitch shift, and
/// cross-rate mixing needs a resampler (named residue).
pub type Mixer = std::sync::Arc<std::sync::Mutex<Vec<std::sync::Arc<std::sync::Mutex<AudioFeed>>>>>;

/// A live `cpal` output stream pulling from a shared [`AudioFeed`].
///
/// Holding the struct holds the stream; dropping it (navigation clears media) stops the device.
/// GUI-lane only: the headless build has no business opening sound hardware.
#[cfg(feature = "gui")]
pub struct AudioOut {
    _stream: cpal::Stream,
    /// The mixer the device callback pulls from (tick 370). Exposed so the A/V-sync rule can
    /// name its masters by **identity** — `MediaSet::advance` slaves a transport only to a feed
    /// the device is really consuming (the mixer contains it), because any other feed's cursor
    /// never moves and a motionless master freezes the picture.
    mixer: Mixer,
    /// The stream's fixed config, taken from the FIRST feed at open: feeds that disagree are
    /// skipped by the callback (see [`Mixer`]).
    channels: u16,
    sample_rate: u32,
}

#[cfg(feature = "gui")]
impl AudioOut {
    /// Open the default output device configured to the feed's own rate and channel count — the
    /// bitstream's numbers, which `decode_track` already corrected against the container header.
    ///
    /// Every failure returns `None`: no device on a headless box is the *normal* case, not an
    /// error, and the caller records the attempt so the browser does not re-probe the hardware
    /// every frame.
    pub fn open(first: std::sync::Arc<std::sync::Mutex<AudioFeed>>) -> Option<Self> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let device = cpal::default_host().default_output_device()?;
        let (channels, sample_rate) = {
            let f = first.lock().ok()?;
            (f.channels(), f.sample_rate())
        };
        if channels == 0 || sample_rate == 0 {
            return None;
        }
        let config = cpal::StreamConfig {
            channels,
            sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };
        let mixer: Mixer = std::sync::Arc::new(std::sync::Mutex::new(vec![first]));
        let cb_mixer = mixer.clone();
        let stream = device
            .build_output_stream(
                &config,
                move |out: &mut [f32], _| {
                    mix_into(&cb_mixer, channels, sample_rate, out);
                },
                |e| tracing::debug!("audio output stream error: {e}"),
                None,
            )
            .map_err(|e| tracing::debug!("audio output unavailable: {e}"))
            .ok()?;
        stream.play().ok()?;
        Some(Self {
            _stream: stream,
            mixer,
            channels,
            sample_rate,
        })
    }

    /// The mixer the device pulls from — the master-set for A/V sync, and where the GUI adds
    /// each newly-loaded element's feed.
    pub fn mixer(&self) -> &Mixer {
        &self.mixer
    }

    /// Add a feed to the mix, unless already present or config-mismatched (skipped honestly —
    /// see [`Mixer`]; a mismatch is a silent element, never a pitch shift).
    pub fn add(&self, feed: &std::sync::Arc<std::sync::Mutex<AudioFeed>>) {
        let ok = feed
            .lock()
            .map(|f| f.channels() == self.channels && f.sample_rate() == self.sample_rate)
            .unwrap_or(false);
        if !ok {
            return;
        }
        if let Ok(mut m) = self.mixer.lock() {
            if !m.iter().any(|x| std::sync::Arc::ptr_eq(x, feed)) {
                m.push(feed.clone());
            }
        }
    }
}

/// The mix: sum every config-matching feed into `out`, clamped to [-1, 1]. Pure and free of the
/// device so the gate drives it headlessly — the same split as pump/device (t350).
pub fn mix_into(mixer: &Mixer, channels: u16, sample_rate: u32, out: &mut [f32]) {
    out.fill(0.0);
    let Ok(feeds) = mixer.lock() else {
        return; // poisoned: silence, per the module contract
    };
    let mut scratch = vec![0.0f32; out.len()];
    for feed in feeds.iter() {
        let Ok(mut f) = feed.lock() else { continue };
        if f.channels() != channels {
            continue; // channel-count mismatch: a downmix matrix question, still skipped honestly
        }
        let src_rate = f.sample_rate();
        if src_rate == sample_rate {
            let n = f.fill(&mut scratch);
            for (o, s) in out.iter_mut().zip(scratch[..n].iter()) {
                *o = (*o + *s).clamp(-1.0, 1.0);
            }
            continue;
        }
        if src_rate == 0 {
            continue;
        }
        // Rate mismatch (tick 375): LINEAR resample — pull the source frames this buffer spans,
        // interpolate between neighbours per output frame. Policy arithmetic, not a codec (the
        // presentation-clock precedent); speech/effects-grade, and audible beats silent. The
        // source cursor advances at the SOURCE's own rate, so the feed's position — which
        // mastery reads — stays truthful.
        let ch = channels as usize;
        let out_frames = out.len() / ch;
        let ratio = src_rate as f64 / sample_rate as f64;
        // +1: the last output frame interpolates toward the frame AFTER its floor index.
        let need_frames = (out_frames as f64 * ratio).ceil() as usize + 1;
        let mut src = vec![0.0f32; need_frames * ch];
        let got = f.fill(&mut src) / ch;
        if got == 0 {
            continue;
        }
        for of in 0..out_frames {
            let pos = of as f64 * ratio;
            let i0 = pos as usize;
            if i0 >= got {
                break;
            }
            let i1 = (i0 + 1).min(got - 1);
            let t = (pos - i0 as f64) as f32;
            for c in 0..ch {
                let s = src[i0 * ch + c] * (1.0 - t) + src[i1 * ch + c] * t;
                let o = &mut out[of * ch + c];
                *o = (*o + s).clamp(-1.0, 1.0);
            }
        }
    }
}

/// # G_AUDIO_PUMP — decoded PCM reaches the device boundary sample-exact
///
/// The gate the observer's standing rule shaped: **decoded PCM correctness, never audible
/// playback** — a gate that needs a working sound card false-REDs on every headless box. So this
/// drives [`AudioFeed`] through the exact call pattern a `cpal` callback produces (repeated
/// odd-sized `fill`s) against the same real AAC fixture the decode gate uses, and asserts the
/// device boundary sees the decoder's samples exactly.
///
/// ## How each claim goes RED
///
/// - **sample-exact delivery** — advance the cursor *before* the copy: every chunk starts one
///   chunk-length late and the concatenation diverges from the decode. And the subtler cousin —
///   advance by `out.len()` instead of `n` — corrupts nothing mid-stream (full chunks have
///   `n == out.len()`) but overshoots at the tail, which only the exact-landing assertion sees;
///   the first run of this gate missed it, which is why that assertion exists.
/// - **silence on exhaustion / pause** — return early without `out.fill(0.0)`: the device replays
///   the previous callback's samples as a stutter-loop; the pre-filled garbage below detects it.
/// - **pause holds position** — advance the cursor while paused: resume audibly skips.
/// - **the MSE re-decode resumes** — reset `cursor` in `replace_pcm`: every appendBuffer restarts
///   the audio from zero (the same player-visible bug `load_mse` guards against for video).
#[cfg(test)]
mod tests {
    use super::*;

    /// Constrained Baseline H.264 + AAC-LC, fragmented — engine/media/tests/data/README.md.
    const AV: &[u8] = include_bytes!("../../engine/media/tests/data/bear-av-baseline_frag.mp4");

    fn decoded() -> Pcm {
        let movie = manuk_media::demux(AV).expect("fixture demuxes");
        let track = movie.audio().expect("fixture carries an audio track");
        manuk_media::decode_track(track, AV).expect("fixture AAC decodes")
    }

    /// # G_MIX — two feeds, one buffer, both audible (tick 370)
    ///
    /// Drives `mix_into` exactly as the device callback does, headlessly (the t350 split).
    ///
    /// ## How each claim goes RED
    ///
    /// - **the sum** — make `mix_into` stop after the first feed: the second element is silent,
    ///   which is the exact one-stream-wins state this tick closes.
    /// - **the clamp** — drop the `.clamp`: two loud streams overflow past ±1 and the device
    ///   wraps/distorts.
    /// - **the mismatch skip** — remove the config check: a 48k feed pulled at 44.1k is a pitch
    ///   shift misread as music.
    #[test]
    fn g_mix() {
        let pcm = decoded();
        let (ch, rate) = (pcm.channels, pcm.sample_rate);
        let a = std::sync::Arc::new(std::sync::Mutex::new(AudioFeed::new(pcm.clone())));
        let b = std::sync::Arc::new(std::sync::Mutex::new(AudioFeed::new(pcm.clone())));
        let mixer: Mixer = std::sync::Arc::new(std::sync::Mutex::new(vec![a.clone(), b.clone()]));

        // ── Both feeds contribute: the mix is the clamped sum, sample-exact.
        let want = pcm.samples.clone();
        let mut out = vec![f32::NAN; 600];
        mix_into(&mixer, ch, rate, &mut out);
        for (i, o) in out.iter().enumerate() {
            let expect = (want[i] + want[i]).clamp(-1.0, 1.0);
            assert!(
                (o - expect).abs() < 1e-6,
                "mix sample {i} must be the clamped SUM of both feeds: got {o}, want {expect}"
            );
        }
        // Both cursors advanced — feed B is not a spectator.
        assert!(a.lock().unwrap().position_seconds() > 0.0);
        assert!(
            b.lock().unwrap().position_seconds() > 0.0,
            "the second feed must be CONSUMED — one-stream-wins is the state this closes"
        );

        // ── The clamp: two LOUD feeds must saturate at ±1.0, not overflow past it. Synthetic
        //    0.8-constant streams because the real fixture is too quiet to clip — a clamp claim
        //    that no input can trigger is a green that cannot go red (the t350 lesson).
        let loud = Pcm {
            samples: vec![0.8f32; 512],
            channels: ch,
            sample_rate: rate,
        };
        let l1 = std::sync::Arc::new(std::sync::Mutex::new(AudioFeed::new(loud.clone())));
        let l2 = std::sync::Arc::new(std::sync::Mutex::new(AudioFeed::new(loud)));
        let loud_mix: Mixer = std::sync::Arc::new(std::sync::Mutex::new(vec![l1, l2]));
        let mut lo = vec![f32::NAN; 256];
        mix_into(&loud_mix, ch, rate, &mut lo);
        assert!(
            lo.iter().all(|&s| (s - 1.0).abs() < 1e-6),
            "0.8 + 0.8 must CLAMP to 1.0 — an unclamped sum overflows the device and distorts \
             (got {})",
            lo[0]
        );

        // ── A rate-mismatched feed RESAMPLES into the mix (tick 375): a constant stream must
        //    come out as the same constant (interpolation may not invent wobble), and the source
        //    cursor advances at the SOURCE's own rate — which is what mastery reads.
        let half = Pcm {
            samples: vec![0.5f32; 4096 * ch as usize],
            channels: ch,
            sample_rate: rate / 2,
        };
        let c = std::sync::Arc::new(std::sync::Mutex::new(AudioFeed::new(half)));
        let solo: Mixer = std::sync::Arc::new(std::sync::Mutex::new(vec![c.clone()]));
        let mut out2 = vec![f32::NAN; 400 * ch as usize];
        mix_into(&solo, ch, rate, &mut out2);
        assert!(
            out2.iter().all(|&s| (s - 0.5).abs() < 1e-6),
            "a half-rate CONSTANT stream must resample to the same constant — wobble here is \
             interpolation reading the wrong neighbours (got {})",
            out2[0]
        );
        let consumed = c.lock().unwrap().position_seconds();
        let expect = 400.0 / (rate as f64); // 400 device frames at half rate = 200 src frames +1
        assert!(
            (consumed - expect).abs() < 4.0 / rate as f64,
            "the source cursor advances at the SOURCE rate ({consumed}s vs ~{expect}s) — \
             consuming at the device rate is the pitch shift the t370 skip existed to prevent"
        );

        // ── A CHANNEL-count mismatch is still skipped (a downmix matrix question, not a rate).
        let alien = Pcm {
            samples: vec![0.7f32; 1024],
            channels: ch + 1,
            sample_rate: rate,
        };
        let d = std::sync::Arc::new(std::sync::Mutex::new(AudioFeed::new(alien)));
        let solo2: Mixer = std::sync::Arc::new(std::sync::Mutex::new(vec![d.clone()]));
        let mut out3 = vec![f32::NAN; 128];
        mix_into(&solo2, ch, rate, &mut out3);
        assert!(
            out3.iter().all(|&s| s == 0.0),
            "a channel-mismatched feed stays SKIPPED"
        );
        assert!((d.lock().unwrap().position_seconds() - 0.0).abs() < 1e-12);

        // ── A paused member is silence without silencing the mix.
        b.lock().unwrap().set_playing(false);
        let mut out3 = vec![f32::NAN; 400];
        mix_into(&mixer, ch, rate, &mut out3);
        let a_pos_frames = 600 / ch as usize; // feed A already consumed 600 samples above
        for (i, o) in out3.iter().enumerate() {
            let expect = want[a_pos_frames * ch as usize + i].clamp(-1.0, 1.0);
            assert!(
                (o - expect).abs() < 1e-6,
                "with B paused the mix is exactly A's stream: sample {i} got {o} want {expect}"
            );
        }
    }

    #[test]
    fn g_audio_pump() {
        let pcm = decoded();
        let want = pcm.samples.clone();
        let channels = pcm.channels as usize;
        assert!(
            want.len() > 1000,
            "the fixture must decode to a real signal, got {} samples",
            want.len()
        );
        // A mis-fed decoder emits a correctly-sized FLAT field — the same honesty guard the
        // video decode gate asserts on frame non-uniformity.
        assert!(
            want.iter().any(|&s| s != want[0]),
            "decoded PCM must be non-uniform"
        );

        // ── Sample-exact delivery across odd-sized chunk boundaries (313 is deliberately not a
        //    multiple of the channel count, the frame size, or anything else).
        let mut feed = AudioFeed::new(pcm.clone());
        let mut got: Vec<f32> = Vec::new();
        let mut buf = [f32::NAN; 313];
        loop {
            let n = feed.fill(&mut buf);
            if n == 0 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
            assert!(
                buf[n..].iter().all(|&s| s == 0.0),
                "the tail past the real samples must be zeroed, not left as stale garbage"
            );
            buf = [f32::NAN; 313];
        }
        assert_eq!(
            got, want,
            "the device boundary must see the decoder's samples exactly — an order-of-operations \
             bug (cursor advanced before the copy) skips samples at every chunk boundary"
        );
        assert!(feed.exhausted());
        assert_eq!(
            feed.cursor,
            want.len(),
            "the cursor must land EXACTLY on the end — advancing by the chunk size instead of \
             the copied count overshoots at the tail, and position_seconds then reports a time \
             past the track's own duration"
        );

        // ── Exhaustion delivers SILENCE into a pre-fouled buffer, forever, without panicking.
        let mut tail = [f32::NAN; 64];
        assert_eq!(feed.fill(&mut tail), 0);
        assert!(
            tail.iter().all(|&s| s == 0.0),
            "an exhausted feed must hand the device zeros — anything else replays stale samples \
             as a stutter-loop"
        );

        // ── Pause: silence, position held; resume: the EXACT next sample.
        let mut feed = AudioFeed::new(pcm.clone());
        let mut a = vec![0f32; 1024];
        assert_eq!(feed.fill(&mut a), 1024);
        let pos = feed.position_seconds();
        assert!(
            (pos - (1024 / channels) as f64 / pcm.sample_rate as f64).abs() < 1e-9,
            "position must be consumed frames over the sample rate"
        );
        feed.set_playing(false);
        let mut b = vec![f32::NAN; 512];
        assert_eq!(feed.fill(&mut b), 0, "a paused feed delivers no samples");
        assert!(
            b.iter().all(|&s| s == 0.0),
            "a paused feed delivers silence"
        );
        assert!(
            (feed.position_seconds() - pos).abs() < 1e-12,
            "pause must hold position — a cursor that creeps while paused skips on resume"
        );
        feed.set_playing(true);
        let mut c = vec![0f32; 512];
        assert_eq!(feed.fill(&mut c), 512);
        assert_eq!(
            &c[..],
            &want[1024..1536],
            "resume must continue at the exact sample where pause stopped"
        );

        // ── Seek lands on a frame boundary (never between the channels of one frame).
        feed.seek_seconds(0.25);
        assert_eq!(
            feed.cursor % channels,
            0,
            "a seek between channels swaps left/right for the rest of the track"
        );

        // ── The stream GREW (MSE append): the cursor survives the re-decode.
        let mut feed = AudioFeed::new(pcm.clone());
        let mut warm = vec![0f32; 1536];
        assert_eq!(feed.fill(&mut warm), 1536);
        let mut grown = pcm.clone();
        grown
            .samples
            .extend_from_slice(&want[..500.min(want.len())]);
        let grown_samples = grown.samples.clone();
        feed.replace_pcm(grown);
        assert_eq!(
            feed.cursor, 1536,
            "an MSE re-decode must RESUME — resetting here restarts the audio on every append"
        );
        let mut d = vec![0f32; 256];
        assert_eq!(feed.fill(&mut d), 256);
        assert_eq!(
            &d[..],
            &grown_samples[1536..1792],
            "after a grow, playback continues into the new timeline from the old position"
        );

        // ── A SHORTER replacement clamps rather than reading out of bounds.
        let mut short = pcm.clone();
        short.samples.truncate(100);
        feed.replace_pcm(short);
        assert!(
            feed.exhausted(),
            "a cursor past a shorter stream clamps to its end"
        );
        let mut e = [f32::NAN; 32];
        assert_eq!(feed.fill(&mut e), 0);
        assert!(e.iter().all(|&s| s == 0.0));
    }
}
