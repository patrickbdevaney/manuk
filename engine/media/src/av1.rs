//! # AV1 decode — MEDIA.md tick 6, behind the trait M5 built for it
//!
//! **Borrowed, not built**: `re_rav1d` does the decode — the Rust port of dav1d, taken through
//! its own safe `dav1d` module (a fork of `dav1d-rs`). The upstream `rav1d` crate exposes only a
//! C-ABI drop-in surface (MEDIA.md trap #3); the `re_` fork is the one with a Rust API, which is
//! exactly why MEDIA.md names it.
//!
//! ## The decoder is asynchronous, and that is the shape of this module
//!
//! openh264 answers a sample with its frame synchronously, so [`crate::video::H264Decoder`] could
//! be a straight line. dav1d is a **queue**: `send_data` may say [`Error::Again`] ("drain me
//! first"), a consumed sample may produce its picture several calls later, and `dav1d_flush` is
//! a seek-reset that DISCARDS pending pictures rather than surfacing them. Three consequences,
//! each load-bearing:
//!
//! - **The presentation time rides THROUGH the decoder**, as the microsecond timestamp
//!   `send_data` accepts and the picture hands back. Pairing "the pts I sent" with "the picture I
//!   got" by call order is exactly the decode-order/presentation-order conflation the playback
//!   module warns about, wearing a new codec.
//! - **[`VideoDecoder::finish`] exists because of this module** (a defaulted no-op for H.264):
//!   without the final drain, the tail of a stream is silently shorter than the author's — the
//!   delayed frames were decoded and never shown.
//! - A sample may surface **several** pictures at once (delayed ones arriving together), so
//!   ready frames queue in `pending` and pop one per `decode_sample` call; the pts each carries
//!   is its own, so ordering is the timeline's job and it already sorts.
//!
//! ## The sequence header lives out of band, like H.264's SPS/PPS
//!
//! MP4 keeps the AV1 sequence-header OBU in the `av1C` box (`configOBUs`, after 4 fixed bytes),
//! not necessarily in any sample. It is sent to the decoder before the first sample — a stream
//! whose keyframes also carry their own sequence header re-reads it harmlessly; one that relies
//! on `av1C` alone is undecodable without it. The same out-of-band-config lesson `avcC` taught,
//! §5.2.4.1 shaped differently.

use std::collections::VecDeque;

use re_rav1d::dav1d;

use crate::video::{Frame, VideoDecoder, VideoError};
use crate::{Track, TrackKind};

/// Timestamps cross the decoder in microseconds: integral, and fine enough that no two frames of
/// any real stream collide.
const TICKS_PER_SECOND: f64 = 1_000_000.0;

/// Can this track's AV1 be decoded here? `av01.*` in any profile the 8-bit build reads.
pub fn can_decode(track: &Track) -> bool {
    track.kind == TrackKind::Video
        && track
            .codec
            .as_deref()
            .is_some_and(|c| c.starts_with("av01."))
}

pub struct Av1Decoder {
    inner: dav1d::Decoder,
    /// Decoded, converted, not yet handed out — see the module note on samples that surface
    /// several pictures at once.
    pending: VecDeque<Frame>,
}

impl Av1Decoder {
    pub fn new(track: &Track) -> Result<Self, VideoError> {
        if !can_decode(track) {
            return Err(VideoError::Unsupported(
                track.codec.clone().unwrap_or_else(|| "unknown".into()),
            ));
        }
        // Measured, not assumed (the first version of this module failed with 'no sample
        // produced a frame' and the fix was probed one variable at a time): the culprit was a
        // `flush()` in `finish` — dav1d's flush DISCARDS pending pictures — and NEITHER of these
        // settings is load-bearing for the fixture. They are kept on their own merits:
        // `max_frame_delay(1)` guarantees a sent sample's picture is drainable in the same
        // `decode_sample` call (the low-latency shape a browser and the MSE grow cycle want,
        // and what rerun's decoder sets), `strict_std_compliance(false)` prioritises delivering
        // frames over spec-lawyering errors.
        let mut settings = dav1d::Settings::new();
        settings.set_max_frame_delay(1);
        settings.set_strict_std_compliance(false);
        let mut inner = dav1d::Decoder::with_settings(&settings)
            .map_err(|e| VideoError::Failed(format!("dav1d open: {e}")))?;
        // The out-of-band sequence header — see the module note. The 4 fixed bytes are
        // marker/version/profile/level+flags (AV1-ISOBMFF §2.3); OBUs follow.
        if let Some(cfg) = track.codec_config.as_ref() {
            if cfg.len() > 4 {
                inner
                    .send_data(cfg[4..].to_vec(), None, None, None)
                    .map_err(|e| VideoError::BadConfig(format!("av1C rejected: {e}")))?;
            }
        }
        Ok(Self {
            inner,
            pending: VecDeque::new(),
        })
    }

    /// Pull every picture the decoder is ready to surface into `pending`.
    fn drain(&mut self) -> Result<(), VideoError> {
        loop {
            match self.inner.get_picture() {
                Ok(pic) => self.pending.push_back(frame_from(&pic)?),
                Err(dav1d::Error::Again) => return Ok(()), // nothing ready — the normal exit
                Err(e) => return Err(VideoError::Failed(format!("get_picture: {e}"))),
            }
        }
    }
}

impl VideoDecoder for Av1Decoder {
    fn decode_sample(
        &mut self,
        sample: &[u8],
        presentation_time: f64,
    ) -> Result<Option<Frame>, VideoError> {
        let ts = (presentation_time * TICKS_PER_SECOND).round() as i64;
        match self.inner.send_data(sample.to_vec(), None, Some(ts), None) {
            Ok(()) => {}
            Err(dav1d::Error::Again) => {
                // The decoder is full: drain what it has, then hand it the sample it kept.
                loop {
                    self.drain()?;
                    match self.inner.send_pending_data() {
                        Ok(()) => break,
                        Err(dav1d::Error::Again) => continue,
                        Err(e) => {
                            return Err(VideoError::Failed(format!("send_pending_data: {e}")))
                        }
                    }
                }
            }
            Err(e) => return Err(VideoError::Failed(format!("send_data: {e}"))),
        }
        self.drain()?;
        Ok(self.pending.pop_front())
    }

    fn finish(&mut self) -> Result<Vec<Frame>, VideoError> {
        // Drain WITHOUT `flush()`: dav1d's flush is a seek-reset that DISCARDS pending pictures,
        // not a surfacing call — running it here is how a stream loses its tail while looking
        // fully decoded. With `max_frame_delay(1)` this drain is usually already empty.
        self.drain()?;
        Ok(self.pending.drain(..).collect())
    }
}

/// Convert one decoded picture to the RGBA the paint path assumes everywhere.
///
/// BT.601 limited-range, the web-video default when a stream says nothing else; the four-colors
/// gate asserts the quadrants land on their names, which catches a swapped U/V or a scrambled
/// plane read — the two mistakes a "right size, wrong picture" conversion actually makes.
fn frame_from(pic: &dav1d::Picture) -> Result<Frame, VideoError> {
    if pic.bit_depth() != 8 {
        return Err(VideoError::Unsupported(format!(
            "{}-bit AV1 (8-bit build)",
            pic.bit_depth()
        )));
    }
    let (w, h) = (pic.width() as usize, pic.height() as usize);
    let layout = pic.pixel_layout();
    let y_plane = pic.plane(dav1d::PlanarImageComponent::Y);
    let y_stride = pic.stride(dav1d::PlanarImageComponent::Y) as usize;
    // Chroma subsampling per layout: (x shift, y shift). I400 has no chroma at all.
    let chroma = match layout {
        dav1d::PixelLayout::I400 => None,
        dav1d::PixelLayout::I420 => Some((1usize, 1usize)),
        dav1d::PixelLayout::I422 => Some((1, 0)),
        dav1d::PixelLayout::I444 => Some((0, 0)),
    };
    let (u_plane, v_plane, uv_stride) = match chroma {
        Some(_) => (
            Some(pic.plane(dav1d::PlanarImageComponent::U)),
            Some(pic.plane(dav1d::PlanarImageComponent::V)),
            pic.stride(dav1d::PlanarImageComponent::U) as usize,
        ),
        None => (None, None, 0),
    };

    let mut rgba = vec![255u8; w * h * 4];
    for row in 0..h {
        for col in 0..w {
            let y = y_plane[row * y_stride + col] as f32;
            let (u, v) = match chroma {
                Some((xs, ys)) => {
                    let idx = (row >> ys) * uv_stride + (col >> xs);
                    (
                        u_plane.as_ref().unwrap()[idx] as f32,
                        v_plane.as_ref().unwrap()[idx] as f32,
                    )
                }
                None => (128.0, 128.0),
            };
            let yv = (y - 16.0) * 1.164;
            let r = yv + 1.596 * (v - 128.0);
            let g = yv - 0.392 * (u - 128.0) - 0.813 * (v - 128.0);
            let b = yv + 2.017 * (u - 128.0);
            let px = (row * w + col) * 4;
            rgba[px] = r.clamp(0.0, 255.0) as u8;
            rgba[px + 1] = g.clamp(0.0, 255.0) as u8;
            rgba[px + 2] = b.clamp(0.0, 255.0) as u8;
        }
    }

    // The timestamp that rode through the decoder — see the module note on why call order is
    // the wrong pairing.
    let presentation_time = pic.timestamp().unwrap_or(0) as f64 / TICKS_PER_SECOND;
    Ok(Frame {
        width: w as u32,
        height: h as u32,
        rgba,
        presentation_time,
    })
}
