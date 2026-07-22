//! # H.264 decode — media step M5
//!
//! Demux (M3) found the video track and named it. This turns its samples into pixels.
//!
//! **Borrowed, not built** (`docs/loop/MEDIA.md`): `openh264` does the decode. A codec is never
//! hand-written here.
//!
//! ## The trait is the point of this step, not the backend
//!
//! [`VideoDecoder`] exists on day one because the backend behind it is known to be temporary.
//! `openh264` decodes **Constrained Baseline only** — B-frame reordering is unimplemented — while
//! the open web's H.264 is overwhelmingly **High** profile (`libx264`'s default: CABAC, B-frames,
//! 8x8 transform). So this backend cannot play most `<video>` on the web, and saying otherwise
//! would be the kind of claim that reads as a feature and behaves as a bug.
//!
//! What it *can* decode is real and worth having: YouTube's no-MSE fallback is `avc1.42001E` —
//! Baseline, 360p — and it decodes with `cargo build` and **zero system dependencies**. The
//! High-profile backend (VA-API via `cros-codecs`, or feature-gated ffmpeg) drops in behind this
//! same trait later without a caller changing.
//!
//! ## The format mismatch that actually costs the tick
//!
//! **MP4 does not store H.264 the way a decoder eats it.** Inside an MP4 a sample is *AVCC*: a
//! sequence of NAL units each prefixed by a big-endian length field (1, 2 or 4 bytes — the width is
//! itself recorded in the `avcC` box). Annex-B, which every decoder including openh264 expects, is
//! instead delimited by `00 00 00 01` start codes. Handing a decoder the raw MP4 sample yields no
//! frame and no error worth reading, because the length prefix parses as a garbage NAL header.
//!
//! Worse, the **SPS and PPS are not in the samples at all** — in MP4 they live once, out of band, in
//! the `avcC` decoder configuration record. A decoder handed only the coded frames has never been
//! told the resolution, profile or reference-frame layout, so it discards everything until it is.
//! Both halves are handled in [`annex_b`] and [`avcc_parameter_sets`].

use openh264::decoder::Decoder;
use openh264::formats::YUVSource;

use crate::{Track, TrackKind};

/// One decoded frame, as 8-bit RGBA.
///
/// RGBA rather than the decoder's native YUV because `engine/paint` already has `DecodedImage` and
/// `blit_image`, and a `<video>` is an `<img>` that gets a new picture thirty times a second. The
/// conversion belongs here, once, rather than in the paint path on every frame.
#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Tightly packed `width * height * 4` bytes, RGBA order, alpha always 255.
    pub rgba: Vec<u8>,
    /// When this frame is shown, in seconds.
    pub presentation_time: f64,
}

impl Frame {
    /// True when every pixel is the same colour — a decode that "succeeded" into a flat green or
    /// black field, which is what a mis-fed decoder produces.
    ///
    /// Exists so a gate can assert the frame carries an *image* rather than merely the right number
    /// of bytes. Correctly-sized uniform output passes every dimension check.
    pub fn is_uniform(&self) -> bool {
        self.rgba
            .chunks_exact(4)
            .skip(1)
            .all(|px| px == &self.rgba[0..4])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoError {
    /// The track is not video, or is a video codec with no decoder here.
    Unsupported(String),
    /// The track carries no `avcC`, so no sample can be interpreted.
    MissingConfig,
    /// The `avcC` record is malformed or truncated.
    BadConfig(String),
    /// The decoder rejected the stream.
    Failed(String),
}

impl std::fmt::Display for VideoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoError::Unsupported(c) => write!(f, "no video decoder for {c}"),
            VideoError::MissingConfig => write!(f, "track has no decoder configuration"),
            VideoError::BadConfig(m) => write!(f, "malformed avcC: {m}"),
            VideoError::Failed(m) => write!(f, "decode failed: {m}"),
        }
    }
}

impl std::error::Error for VideoError {}

/// A source of decoded video frames.
///
/// One implementation today ([`H264Decoder`]). Defined now, while there is exactly one, because the
/// known-temporary backend behind it is the whole reason this shape has to exist — see the module
/// docs. Retrofitting it after callers exist costs multiples of writing it here.
pub trait VideoDecoder {
    /// Decode one coded sample. Returns `Ok(None)` when the decoder consumed the sample but has no
    /// frame to emit yet, which is normal and not an error.
    fn decode_sample(
        &mut self,
        sample: &[u8],
        presentation_time: f64,
    ) -> Result<Option<Frame>, VideoError>;

    /// The samples have run out: surface anything still inside the decoder.
    ///
    /// A defaulted no-op because openh264 answers synchronously and holds nothing back. dav1d is
    /// a queue — pictures it delayed are only reachable through an explicit flush, and skipping
    /// this call silently truncates the tail of every AV1 stream (`av1::Av1Decoder` overrides).
    fn finish(&mut self) -> Result<Vec<Frame>, VideoError> {
        Ok(Vec::new())
    }
}

/// Can this track's video be decoded here? The honest answer to `isTypeSupported`, for video.
///
/// **Deliberately narrower than "is it H.264".** The codec string carries the profile in its second
/// byte (`avc1.42001E` -> `0x42`, Baseline; `avc1.64001E` -> `0x64`, High) and only the Baseline
/// family decodes. Answering `true` for High would be accepted at `isTypeSupported` and then fail
/// mid-stream, which is strictly worse for a player than an honest `false` — it has a fallback and
/// this is how it gets to use it.
pub fn can_decode(track: &Track) -> bool {
    if track.kind != TrackKind::Video {
        return false;
    }
    let Some(codec) = track.codec.as_deref() else {
        return false;
    };
    // AV1 — but ONLY where the decoder is actually compiled in. A lane built without the `av1`
    // feature must keep answering no: advertising a codec this build cannot decode is the
    // isTypeSupported lie MEDIA.md's MSE warning exists to prevent.
    #[cfg(feature = "av1")]
    if crate::av1::can_decode(track) {
        return true;
    }
    // `avc1.PPCCLL` — PP is the profile_idc as two hex digits.
    let Some(rest) = codec.strip_prefix("avc1.") else {
        return false;
    };
    let Ok(profile) = u8::from_str_radix(rest.get(0..2).unwrap_or(""), 16) else {
        return false;
    };
    // 66 = Baseline. Constrained Baseline is 66 with the constraint_set1 flag, and both decode.
    profile == 66
}

/// Pull the SPS and PPS out of an `avcC` decoder configuration record, as Annex-B.
///
/// Layout (ISO/IEC 14496-15 §5.2.4.1), all big-endian:
///
/// ```text
///   [0]      configurationVersion == 1
///   [1]      AVCProfileIndication      <- 66 Baseline, 100 High
///   [2]      profile_compatibility
///   [3]      AVCLevelIndication
///   [4]      111111xx                   xx = lengthSizeMinusOne
///   [5]      111xxxxx                   xxxxx = numOfSequenceParameterSets
///   then     { u16 length, length bytes } * numSPS
///   then u8  numOfPictureParameterSets
///   then     { u16 length, length bytes } * numPPS
/// ```
///
/// Returns the parameter sets already start-code prefixed, plus the NAL length size the samples
/// use. That length size is read here rather than assumed to be 4: it is legal for it to be 1 or 2,
/// and assuming 4 against a 2-byte stream desynchronises on the very first NAL.
pub fn avcc_parameter_sets(avcc: &[u8]) -> Result<(Vec<u8>, usize), VideoError> {
    if avcc.len() < 7 {
        return Err(VideoError::BadConfig(format!(
            "record is {} bytes, minimum is 7",
            avcc.len()
        )));
    }
    let length_size = (avcc[4] & 0b11) as usize + 1;
    let mut out = Vec::new();
    let mut pos = 5;

    // SPS count is the low 5 bits; the top 3 are reserved ones.
    let num_sps = (avcc[pos] & 0b0001_1111) as usize;
    pos += 1;
    pos = copy_parameter_sets(avcc, pos, num_sps, &mut out)?;

    if pos >= avcc.len() {
        return Err(VideoError::BadConfig("truncated before PPS count".into()));
    }
    let num_pps = avcc[pos] as usize;
    pos += 1;
    copy_parameter_sets(avcc, pos, num_pps, &mut out)?;

    if out.is_empty() {
        return Err(VideoError::BadConfig(
            "record declares no parameter sets".into(),
        ));
    }
    Ok((out, length_size))
}

fn copy_parameter_sets(
    avcc: &[u8],
    mut pos: usize,
    count: usize,
    out: &mut Vec<u8>,
) -> Result<usize, VideoError> {
    for _ in 0..count {
        if pos + 2 > avcc.len() {
            return Err(VideoError::BadConfig(
                "truncated parameter-set length".into(),
            ));
        }
        let len = u16::from_be_bytes([avcc[pos], avcc[pos + 1]]) as usize;
        pos += 2;
        if pos + len > avcc.len() {
            return Err(VideoError::BadConfig("truncated parameter-set body".into()));
        }
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(&avcc[pos..pos + len]);
        pos += len;
    }
    Ok(pos)
}

/// Rewrite a length-prefixed AVCC sample into Annex-B start-code form.
///
/// A NAL whose declared length runs past the end of the sample **truncates the conversion rather
/// than failing it**: a real stream can be appended mid-fragment, and emitting the NALs that are
/// whole is what lets the decoder show the frames that did arrive. Returning an error would throw
/// away good data because the tail was short.
pub fn annex_b(sample: &[u8], length_size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(sample.len() + 16);
    let mut pos = 0;
    while pos + length_size <= sample.len() {
        let mut len = 0usize;
        for i in 0..length_size {
            len = (len << 8) | sample[pos + i] as usize;
        }
        pos += length_size;
        if len == 0 || pos + len > sample.len() {
            break;
        }
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(&sample[pos..pos + len]);
        pos += len;
    }
    out
}

/// H.264 decoding, backed by OpenH264. Constrained Baseline only — see the module docs.
pub struct H264Decoder {
    inner: Decoder,
    length_size: usize,
    /// The Annex-B SPS/PPS from `avcC`, prepended to the first sample.
    ///
    /// Taken (left `None`) once sent. Re-sending them per frame is harmless to a decoder but makes
    /// the "have I been configured yet" state implicit in a `Vec`'s emptiness; making it explicit
    /// here is what keeps a seek or a re-append from silently skipping configuration.
    parameter_sets: Option<Vec<u8>>,
}

impl H264Decoder {
    /// Build a decoder for a track, reading the NAL length size and parameter sets from its `avcC`.
    pub fn new(track: &Track) -> Result<Self, VideoError> {
        if !can_decode(track) {
            return Err(VideoError::Unsupported(
                track.codec.clone().unwrap_or_else(|| "unknown".into()),
            ));
        }
        let avcc = track
            .codec_config
            .as_ref()
            .ok_or(VideoError::MissingConfig)?;
        let (parameter_sets, length_size) = avcc_parameter_sets(avcc)?;
        let inner = Decoder::new().map_err(|e| VideoError::Failed(e.to_string()))?;
        Ok(Self {
            inner,
            length_size,
            parameter_sets: Some(parameter_sets),
        })
    }
}

impl VideoDecoder for H264Decoder {
    fn decode_sample(
        &mut self,
        sample: &[u8],
        presentation_time: f64,
    ) -> Result<Option<Frame>, VideoError> {
        let mut bitstream = self.parameter_sets.take().unwrap_or_default();
        bitstream.extend_from_slice(&annex_b(sample, self.length_size));
        if bitstream.is_empty() {
            return Ok(None);
        }

        let decoded = self
            .inner
            .decode(&bitstream)
            .map_err(|e| VideoError::Failed(e.to_string()))?;
        let Some(yuv) = decoded else {
            return Ok(None);
        };

        let (width, height) = yuv.dimensions();
        // openh264 writes RGB8; the extra alpha pass is cheaper than carrying a 3-byte format
        // through the paint path, which assumes RGBA everywhere.
        let mut rgb = vec![0u8; width * height * 3];
        yuv.write_rgb8(&mut rgb);
        let mut rgba = vec![255u8; width * height * 4];
        for (px, src) in rgba.chunks_exact_mut(4).zip(rgb.chunks_exact(3)) {
            px[0..3].copy_from_slice(src);
        }

        Ok(Some(Frame {
            width: width as u32,
            height: height as u32,
            rgba,
            presentation_time,
        }))
    }
}

/// Decode the first frame of a video track — the picture that replaces the poster.
///
/// `buffer` is the same byte buffer the track was demuxed from; sample offsets are absolute within
/// it. Samples are walked in decode order until one produces a frame, because a decoder is entitled
/// to consume a sample without emitting anything yet.
pub fn decode_first_frame(track: &Track, buffer: &[u8]) -> Result<Frame, VideoError> {
    let mut decoder = H264Decoder::new(track)?;
    for s in &track.samples {
        let range = s.byte_range();
        if range.end > buffer.len() {
            continue;
        }
        if let Some(frame) = decoder.decode_sample(&buffer[range], s.presentation_start())? {
            return Ok(frame);
        }
    }
    Err(VideoError::Failed(
        "no sample in the track produced a frame".into(),
    ))
}
