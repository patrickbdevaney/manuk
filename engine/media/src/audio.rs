//! # AAC decode — media step M4
//!
//! Demux (M3) found the audio and said what it is. This turns it into PCM.
//!
//! **Borrowed, not built** (`docs/loop/MEDIA.md`, the standing rule): `symphonia` does the AAC
//! decode. Its role is confined to exactly that — the crate is pulled in with
//! `default-features = false, features = ["aac"]`, because its ISO-MP4 demuxer is audio-only (its
//! video `SampleEntry` is commented out, MEDIA.md trap #1) and acquiring it silently is how a
//! project ends up with two demuxers that disagree. Demux is [`crate::demux`]'s job; decode is this
//! module's; neither reaches into the other.
//!
//! **What this does not do: play.** There is no audio device here. Turning PCM into sound is
//! `cpal`'s job and a separate step, deliberately — a device is not headlessly gateable, and
//! bundling it would mean the decode could only be proven by listening to it. What *is* gateable,
//! and what the gate asserts, is that the decoded PCM is the right length: the frame count must
//! equal the track's declared duration in its own timescale. That is a claim about correctness
//! rather than about whether a function ran.

use symphonia::core::codecs::audio::{AudioCodecParameters, AudioDecoderOptions};
use symphonia::core::packet::PacketRef;

use crate::{Track, TrackKind};

/// Decoded PCM.
#[derive(Debug, Clone)]
pub struct Pcm {
    /// Interleaved samples, `channels` per frame, in `[-1.0, 1.0]`.
    ///
    /// Interleaved rather than planar because that is what an audio device consumes; keeping it
    /// planar here would mean interleaving at the device boundary on every callback, in the one
    /// place with a hard deadline.
    pub samples: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
}

impl Pcm {
    /// One frame is one sample per channel — the unit a timeline is measured in.
    pub fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.samples.len() / self.channels as usize
        }
    }

    pub fn duration_seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.frames() as f64 / self.sample_rate as f64
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The track is not audio, or is an audio codec with no decoder here.
    Unsupported(String),
    /// The track carries no decoder configuration, so no packet can be interpreted.
    MissingConfig,
    /// The decoder rejected the stream.
    Failed(String),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Unsupported(c) => write!(f, "no audio decoder for {c}"),
            DecodeError::MissingConfig => write!(f, "track has no decoder configuration"),
            DecodeError::Failed(m) => write!(f, "decode failed: {m}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Can this track's audio be decoded here? The honest answer to `isTypeSupported`, for audio.
///
/// Only AAC. MP3, Opus, Vorbis, FLAC and AC-3 all appear on the web and none of them are wired up,
/// so they must answer `false` rather than being accepted and then failing mid-stream.
pub fn can_decode(track: &Track) -> bool {
    track.kind == TrackKind::Audio
        && track
            .codec
            .as_deref()
            .is_some_and(|c| c.starts_with("mp4a."))
}

/// Decode every sample of an audio track into interleaved PCM.
///
/// `buffer` is the same byte buffer the track was demuxed from — sample offsets are absolute
/// within it.
///
/// **A packet that fails to decode does not abort the stream.** A real stream can carry a damaged
/// packet, and an adaptive one is routinely appended mid-GOP; dropping the frame and continuing is
/// what a player does, and aborting would turn one bad packet into silence for the whole track. If
/// *nothing* decodes, that is a real failure and is reported as one.
pub fn decode_track(track: &Track, buffer: &[u8]) -> Result<Pcm, DecodeError> {
    if !can_decode(track) {
        return Err(DecodeError::Unsupported(
            track.codec.clone().unwrap_or_else(|| "unknown".into()),
        ));
    }
    let config = track
        .codec_config
        .as_ref()
        .ok_or(DecodeError::MissingConfig)?;

    let mut params = AudioCodecParameters::new();
    params.codec = symphonia::core::codecs::audio::well_known::CODEC_ID_AAC;
    params
        .with_sample_rate(track.sample_rate)
        .with_extra_data(config.clone().into_boxed_slice());

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| DecodeError::Failed(e.to_string()))?;

    let mut out: Vec<f32> = Vec::new();
    let mut channels = track.channels;
    let mut rate = track.sample_rate;
    let mut decoded = 0usize;

    for s in &track.samples {
        let range = s.byte_range();
        if range.end > buffer.len() {
            continue;
        }
        let packet = PacketRef::new(
            0,
            s.decode_timestamp.into(),
            s.duration.into(),
            &buffer[range],
        );
        let Ok(buf) = decoder.decode_ref(&packet) else {
            continue;
        };
        decoded += 1;
        // The container's declared rate and channel count are a header; the decoder's are what the
        // bitstream actually says. Where they disagree the bitstream wins, because it is what the
        // samples were encoded as — and a device configured from the header would resample or
        // mis-map channels for the whole track.
        rate = buf.spec().rate();
        channels = buf.spec().channels().count() as u16;
        interleave(&buf, &mut out);
    }

    if decoded == 0 {
        return Err(DecodeError::Failed(
            "no packet in the track could be decoded".into(),
        ));
    }

    Ok(Pcm {
        samples: out,
        channels,
        sample_rate: rate,
    })
}

/// Decode a RAW audio stream — `<audio src="episode.mp3">` (tick 362), where there is no MP4
/// container and therefore no [`Track`]: the bytes ARE the stream. symphonia's probe finds the
/// format (MPEG audio today; the same seam serves FLAC/Ogg when their features turn on), its
/// packet loop replaces re_mp4's sample table, and the damaged-packet rule matches
/// [`decode_track`]: drop the frame, keep the stream, fail only if NOTHING decodes.
pub fn decode_audio_stream(bytes: &[u8]) -> Result<Pcm, DecodeError> {
    use symphonia::core::codecs::CodecParameters;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
    use symphonia::core::meta::MetadataOptions;

    let mss = MediaSourceStream::new(
        Box::new(std::io::Cursor::new(bytes.to_vec())),
        MediaSourceStreamOptions::default(),
    );
    let mut reader = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| DecodeError::Unsupported(format!("unprobeable stream: {e}")))?;

    let track = reader
        .default_track(TrackType::Audio)
        .ok_or_else(|| DecodeError::Unsupported("no audio track in stream".into()))?;
    let track_id = track.id;
    let params = match &track.codec_params {
        Some(CodecParameters::Audio(p)) => p.clone(),
        _ => return Err(DecodeError::MissingConfig),
    };
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| DecodeError::Failed(e.to_string()))?;

    let mut out: Vec<f32> = Vec::new();
    let mut channels = 0u16;
    let mut rate = 0u32;
    let mut decoded = 0usize;
    loop {
        let packet = match reader.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(_) => break, // a truncated tail is a stream that ends, not a failure of what played
        };
        if packet.track_id != track_id {
            continue;
        }
        let Ok(buf) = decoder.decode(&packet) else {
            continue; // the damaged-packet rule — see the doc
        };
        decoded += 1;
        rate = buf.spec().rate();
        channels = buf.spec().channels().count() as u16;
        interleave(&buf, &mut out);
    }
    if decoded == 0 {
        return Err(DecodeError::Failed(
            "no packet in the stream could be decoded".into(),
        ));
    }
    Ok(Pcm {
        samples: out,
        channels,
        sample_rate: rate,
    })
}

/// Is this an MPEG-audio stream at all? An ID3v2 tag or a raw MPEG sync word — the cheap routing
/// question a fetcher asks before paying for a full probe.
pub fn sniff_mpeg_audio(bytes: &[u8]) -> bool {
    if bytes.len() >= 3 && &bytes[..3] == b"ID3" {
        return true;
    }
    bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0
}

/// Append a decoded buffer to the interleaved output, converting to `f32`.
///
/// `GenericAudioBufferRef` is an enum over every sample format symphonia can produce, so the
/// conversion is done through its own `f32` copy rather than by matching each variant here — the
/// variant list is symphonia's to grow, and a match on it would silently drop a new format.
fn interleave(buf: &symphonia::core::audio::GenericAudioBufferRef<'_>, out: &mut Vec<f32>) {
    let frames = buf.frames();
    let channels = buf.spec().channels().count();
    if frames == 0 || channels == 0 {
        return;
    }
    let start = out.len();
    out.resize(start + frames * channels, 0.0);
    buf.copy_to_slice_interleaved(&mut out[start..]);
}
