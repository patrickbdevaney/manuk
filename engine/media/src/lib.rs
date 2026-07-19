//! # Media container demux — the plumbing under MSE
//!
//! Media step **M3**. Ticks 223/227/228 built the byte pipe: a player constructs a `MediaSource`,
//! attaches it to a `<video>`, fetches segments as `ArrayBuffer`s (byte-exact, `Range` honoured)
//! and hands them to `SourceBuffer.appendBuffer`. Everything up to that point works and **nothing
//! looks at the bytes**. `SourceBuffer.__chunks` is, in its own comment, "a faithful record of what
//! the page handed us and nothing more", and `buffered` is honestly empty because no timeline
//! exists to report.
//!
//! This crate is what reads them. It answers the three questions a player asks between appending
//! bytes and seeing a frame, none of which need a codec:
//!
//! 1. **What is in this stream?** Tracks, their kind, their `codecs=` string, dimensions, and the
//!    decoder configuration record (`avcC`/`av1C`/`vpcC`/AAC `AudioSpecificConfig`) that a decoder
//!    will need *later*. That record is the handoff to M4/M5 and is extracted now so the decoder
//!    step is a decoder step and not another parsing step.
//! 2. **Where is each frame, and when?** A sample table: byte range, decode and presentation
//!    timestamps, duration, sync flag. This is the unit a decoder is fed.
//! 3. **What is buffered?** Contiguous presentation-time ranges — the `TimeRanges` that
//!    `SourceBuffer.buffered` returns and that *every* adaptive player's fetch loop steers by. A
//!    player that cannot read `buffered` cannot decide what to download next, so this is the piece
//!    that makes the append loop a loop rather than a one-shot.
//!
//! ## What this deliberately does NOT do
//!
//! **This module does no decoding, and no codec is hand-written anywhere here**
//! (`docs/loop/MEDIA.md`, the standing rule). A demuxer that reports `avc1.64001E` is not a claim
//! that we can decode H.264 — it is a claim that we can find the H.264 in the file. Those are
//! different, and conflating them is precisely the failure MEDIA.md warns about: **advertising MSE
//! we cannot honour turns a working YouTube into a black rectangle**.
//!
//! Decode does now exist, in two **opt-in** sibling modules that this one never calls: [`audio`]
//! (M4, AAC via symphonia, `--features audio`) and [`video`] (M5, Constrained-Baseline H.264 via
//! openh264, `--features video`). Both are off by default so their dependencies — and openh264's C
//! compile — stay out of the ~25 gate binaries reached through `manuk-js -> manuk-media`.
//!
//! `MediaSource.isTypeSupported` still answers from `__mseCodecs`, which this crate does not touch
//! and which stays empty: **a stream needs both tracks decoded and then played**, and M5 decodes a
//! frame rather than playing a video. Flipping it on a partial pipeline is the black-rectangle
//! failure, so it flips at M6, not here.
//!
//! **WebM/Matroska is not demuxed yet.** [`sniff`] recognises it — so the failure is a named
//! `Unsupported`, not a parse error blamed on the bytes — but there is no EBML reader here. Saying
//! "I know what this is and I cannot read it" is a different and much more debuggable failure than
//! "invalid stream".

use std::ops::Range;

#[cfg(feature = "audio")]
pub mod audio;
#[cfg(feature = "audio")]
pub use audio::{can_decode as can_decode_audio, decode_track, DecodeError, Pcm};

#[cfg(feature = "video")]
pub mod video;
#[cfg(feature = "video")]
pub use video::{
    can_decode as can_decode_video, decode_first_frame, Frame, H264Decoder, VideoDecoder,
    VideoError,
};

/// The presentation clock (M6). Gated behind `video` because [`playback::FrameTimeline`] decodes
/// through [`video::H264Decoder`] — the same isolation reason `video` itself is opt-in: openh264
/// compiles C, and it must stay out of the ~25 gate binaries reached through `manuk-js`.
#[cfg(feature = "video")]
pub mod playback;
#[cfg(feature = "video")]
pub use playback::{FrameTimeline, Transport};

/// What a byte prefix looks like. Recognising a container we cannot read is worth doing: it turns
/// "the segment is corrupt" into "this is WebM and we only demux MP4", which is the truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    /// ISO base media file format — MP4, fragmented MP4, QuickTime.
    Mp4,
    /// EBML — WebM / Matroska. Recognised, not yet demuxed.
    WebM,
    Unknown,
}

/// Sniff the container from the leading bytes.
///
/// MP4 is identified by the `ftyp` box type at offset 4 (the first four bytes are its length), and
/// also by `styp`/`moof` — a *media* segment of a fragmented stream carries no `ftyp` at all, and a
/// player appends init and media segments separately, so a sniffer that only knows `ftyp` rejects
/// half of everything MSE will hand it.
pub fn sniff(bytes: &[u8]) -> Container {
    if bytes.len() >= 4 && bytes[..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        return Container::WebM;
    }
    if bytes.len() >= 8 {
        match &bytes[4..8] {
            b"ftyp" | b"styp" | b"moof" | b"moov" | b"sidx" => return Container::Mp4,
            _ => {}
        }
    }
    Container::Unknown
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Video,
    Audio,
    Subtitle,
    Other,
}

impl TrackKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TrackKind::Video => "video",
            TrackKind::Audio => "audio",
            TrackKind::Subtitle => "subtitle",
            TrackKind::Other => "other",
        }
    }
}

/// One coded frame.
#[derive(Debug, Clone, Copy)]
pub struct Sample {
    pub id: u32,
    /// A sync sample (keyframe) decodes standalone. A seek must land on one.
    pub is_sync: bool,
    /// Byte offset from the start of the *whole appended buffer*, not the segment.
    pub offset: u64,
    pub size: u64,
    /// Ticks per second for the three timestamps below.
    pub timescale: u32,
    /// When to decode, in timescale units.
    pub decode_timestamp: i64,
    /// When to display, in timescale units. B-frames make this differ from the decode timestamp,
    /// and it is the presentation timestamp — never the decode one — that `buffered` is expressed
    /// in and that a player seeks against.
    pub presentation_timestamp: i64,
    pub duration: u64,
}

impl Sample {
    /// Where this frame's bytes live in the buffer that was parsed.
    pub fn byte_range(&self) -> Range<usize> {
        self.offset as usize..(self.offset + self.size) as usize
    }

    pub fn presentation_start(&self) -> f64 {
        self.presentation_timestamp as f64 / self.timescale.max(1) as f64
    }

    pub fn presentation_end(&self) -> f64 {
        (self.presentation_timestamp as f64 + self.duration as f64) / self.timescale.max(1) as f64
    }
}

/// A contiguous span of presentation time, in seconds. One entry of a `TimeRanges`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRange {
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub id: u32,
    pub kind: TrackKind,
    /// The RFC 6381 `codecs=` string — `avc1.64001E`, `mp4a.40.2`, `vp09.00.10.08`. `None` when the
    /// sample entry is one we do not recognise; a player is better served by "unknown" than by a
    /// guess it will branch on.
    pub codec: Option<String>,
    pub width: u32,
    pub height: u32,
    /// Audio only.
    pub channels: u16,
    /// Audio only, in Hz.
    pub sample_rate: u32,
    pub timescale: u32,
    /// Track duration in timescale units. Zero for a media segment appended on its own.
    pub duration: u64,
    /// The decoder configuration record: `avcC`, `av1C`, `vpcC`, or the AAC `AudioSpecificConfig`.
    /// Consumed by [`audio`] (M4) and [`video`] (M5); extracting it at demux time is what keeps the
    /// decoder steps from re-parsing boxes.
    pub codec_config: Option<Vec<u8>>,
    pub samples: Vec<Sample>,
}

impl Track {
    pub fn duration_seconds(&self) -> f64 {
        self.duration as f64 / self.timescale.max(1) as f64
    }

    /// The contiguous presentation-time ranges this track's samples cover — `SourceBuffer.buffered`.
    ///
    /// **The gap tolerance is the whole subtlety.** Consecutive frames rarely abut to the bit:
    /// timescale rounding leaves sub-millisecond seams, and a stream whose audio and video sample
    /// durations do not divide evenly accumulates them. Reporting each frame as its own range
    /// would be arithmetically defensible and practically useless — a player reads `buffered.length`
    /// and a hundred ranges where there is one continuous second of media reads as unplayable
    /// swiss cheese, so it re-fetches what it already has, forever.
    ///
    /// So ranges are merged across gaps up to [`GAP_TOLERANCE`], the same "fudge factor" every
    /// shipping implementation applies for the same reason.
    pub fn buffered(&self) -> Vec<TimeRange> {
        let mut spans: Vec<(f64, f64)> = self
            .samples
            .iter()
            .map(|s| (s.presentation_start(), s.presentation_end()))
            .filter(|(a, b)| b > a)
            .collect();
        // Presentation order is not storage order when B-frames are present, so sort rather than
        // assuming the sample table is already monotonic in presentation time.
        spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut out: Vec<TimeRange> = Vec::new();
        for (start, end) in spans {
            match out.last_mut() {
                Some(last) if start <= last.end + GAP_TOLERANCE => {
                    if end > last.end {
                        last.end = end;
                    }
                }
                _ => out.push(TimeRange { start, end }),
            }
        }
        out
    }
}

/// How large a seam between two frames may be before it counts as a real gap, in seconds.
///
/// 100ms is a few frames at any ordinary rate — comfortably above timescale rounding noise, and
/// comfortably below a gap a viewer would perceive or a player would need to fill.
pub const GAP_TOLERANCE: f64 = 0.1;

#[derive(Debug, Clone)]
pub struct Movie {
    pub tracks: Vec<Track>,
    /// True when the stream carries `moof` boxes — i.e. it is the fragmented form MSE streams.
    pub fragmented: bool,
}

impl Movie {
    pub fn video(&self) -> Option<&Track> {
        self.tracks.iter().find(|t| t.kind == TrackKind::Video)
    }

    pub fn audio(&self) -> Option<&Track> {
        self.tracks.iter().find(|t| t.kind == TrackKind::Audio)
    }

    /// The longest track's duration, in seconds — what `MediaSource.duration` reports once the
    /// stream is known.
    pub fn duration_seconds(&self) -> f64 {
        self.tracks
            .iter()
            .map(|t| t.duration_seconds())
            .fold(0.0, f64::max)
    }

    /// The union of every track's buffered ranges — a time is only playable when *all* the media
    /// for it has arrived, but the union is what `SourceBuffer.buffered` reports for the buffer,
    /// and intersecting across source buffers is `MediaSource`'s job, not a track's.
    pub fn buffered(&self) -> Vec<TimeRange> {
        let mut all: Vec<TimeRange> = self.tracks.iter().flat_map(|t| t.buffered()).collect();
        all.sort_by(|a, b| {
            a.start
                .partial_cmp(&b.start)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut out: Vec<TimeRange> = Vec::new();
        for r in all {
            match out.last_mut() {
                Some(last) if r.start <= last.end + GAP_TOLERANCE => {
                    if r.end > last.end {
                        last.end = r.end;
                    }
                }
                _ => out.push(r),
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DemuxError {
    /// A container we can name but cannot read. Distinct from `Invalid` on purpose.
    Unsupported(Container),
    /// The bytes do not parse as the container they claim to be.
    Invalid(String),
    /// Not enough bytes yet — an MSE append is incremental, and "come back with more" is a normal
    /// answer, not a failure.
    Incomplete,
}

impl std::fmt::Display for DemuxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DemuxError::Unsupported(c) => write!(f, "unsupported container: {c:?}"),
            DemuxError::Invalid(m) => write!(f, "invalid stream: {m}"),
            DemuxError::Incomplete => write!(f, "incomplete stream"),
        }
    }
}

impl std::error::Error for DemuxError {}

/// Demux a whole buffer.
///
/// ## SYNC FLAGS — a borrowed bug, corrected here
///
/// `re_mp4` reports **inverted sync flags for every fragmented sample**, and this is the one place
/// that can be fixed. `reader.rs:443` computes
///
/// ```text
/// is_sync: (sample_flags >> 16) & 0x1 != 0
/// ```
///
/// but bit 16 of a `trun` sample-flags word is `sample_is_non_sync_sample` (ISO/IEC 14496-12 —
/// the low 16 bits are `sample_degradation_priority`, so the flag sits exactly at bit 16). It is
/// the *negation* of `is_sync`, and the negation is missing. The progressive path is unaffected:
/// it derives sync from the `stss` table (`reader.rs:254`), which is a positive list of sync
/// samples and is read correctly.
///
/// **This was found by differential test, not by reading the source** — the source looks right
/// until you check which flag bit 16 is. Chromium ships three fixtures that differ only in their
/// sync flags, and `re_mp4` returns the exact complement of the expected answer for all three:
///
/// | fixture | expected | `re_mp4` |
/// |---|---|---|
/// | `bear-640x360-v-2frames_frag` | `[true, false]` | `[false, true]` |
/// | `…-keyframe-is-non-sync-sample_frag` | `[false, false]` | `[true, true]` |
/// | `…-nonkeyframe-is-sync-sample_frag` | `[true, true]` | `[false, false]` |
///
/// **Why it matters more than it looks.** A seek must land on a sync sample. With the flag
/// inverted, a player seeking into a fragmented stream — which is every adaptive player, on every
/// seek — is handed a frame that cannot decode standalone, and the result is a green/garbage frame
/// or a silent stall. Nothing throws. It is exactly the silent-failure shape this project keeps
/// finding, and shipping it would have surfaced much later as "our H.264 decoder is broken", one
/// layer below where the bug actually is.
///
/// The correction is applied per sample by origin rather than per file, so a file carrying both a
/// populated `stbl` and later fragments is handled correctly instead of being assumed away.
///
/// **Why a whole buffer and not a streaming parser.** MSE appends incrementally, so the obvious
/// design is an incremental demuxer fed one segment at a time. That is the eventual design. It is
/// not this one, because a `SourceBuffer` already retains every appended chunk (it must — eviction
/// is its own spec'd algorithm), so the accumulated bytes are in hand anyway, and re-parsing them
/// is a box walk over a buffer that is bounded by the buffer's own quota. The sample *offsets* this
/// returns are then absolute within that accumulated buffer, which is exactly the coordinate space
/// a decoder wants to read from. An incremental parser buys latency we cannot yet spend, since
/// there is no decoder downstream to spend it on.
pub fn demux(bytes: &[u8]) -> Result<Movie, DemuxError> {
    match sniff(bytes) {
        Container::Mp4 => {}
        Container::WebM => return Err(DemuxError::Unsupported(Container::WebM)),
        Container::Unknown => {
            return if bytes.len() < 8 {
                Err(DemuxError::Incomplete)
            } else {
                Err(DemuxError::Unsupported(Container::Unknown))
            };
        }
    }

    let mp4 = re_mp4::Mp4::read_bytes(bytes).map_err(|e| DemuxError::Invalid(e.to_string()))?;
    let fragmented = !mp4.moofs.is_empty();

    let mut tracks = Vec::new();
    for (id, t) in mp4.tracks() {
        let kind = match t.kind {
            Some(re_mp4::TrackKind::Video) => TrackKind::Video,
            Some(re_mp4::TrackKind::Audio) => TrackKind::Audio,
            Some(re_mp4::TrackKind::Subtitle) => TrackKind::Subtitle,
            None => TrackKind::Other,
        };
        let timescale = t.timescale.max(1) as u32;
        // See `SYNC FLAGS` below. Samples up to this index came from the `stbl` sample table and
        // their sync flags are right; everything after came from a `trun` and is inverted.
        let stbl_samples = t.trak(&mp4).mdia.minf.stbl.stsz.sample_count as usize;
        let samples = t
            .samples
            .iter()
            .enumerate()
            .map(|(i, s)| Sample {
                id: s.id,
                is_sync: if i >= stbl_samples {
                    !s.is_sync
                } else {
                    s.is_sync
                },
                offset: s.offset,
                size: s.size,
                timescale: s.timescale.max(1) as u32,
                decode_timestamp: s.decode_timestamp,
                presentation_timestamp: s.composition_timestamp,
                duration: s.duration,
            })
            .collect();

        let (codec, channels, sample_rate, codec_config) = describe(&t, &mp4);

        tracks.push(Track {
            id: (*id).into(),
            kind,
            codec,
            width: t.width as u32,
            height: t.height as u32,
            channels,
            sample_rate,
            timescale,
            duration: t.duration,
            codec_config,
            samples,
        });
    }
    tracks.sort_by_key(|t| t.id);

    Ok(Movie { tracks, fragmented })
}

/// Codec string, audio parameters and decoder configuration record for one track.
///
/// `re_mp4` derives the RFC 6381 string for the video sample entries but **returns `None` for
/// `mp4a`** — it has no branch for it. AAC is the audio codec of essentially every MP4 on the web
/// (and the one YouTube is steered to), so `codecs="mp4a.40.2"` is not an edge case we can leave
/// unnamed: a player that reads a null codec for the audio track treats the stream as undecodable
/// before it ever asks whether we can decode it. So that string is built here, from the `esds`
/// descriptor, which is also where the channel count and sample rate a device needs live.
fn describe(t: &re_mp4::Track, mp4: &re_mp4::Mp4) -> (Option<String>, u16, u32, Option<Vec<u8>>) {
    if let Some(s) = t.codec_string(mp4) {
        return (Some(s), 0, 0, t.raw_codec_config(mp4));
    }

    let stsd = &t.trak(mp4).mdia.minf.stbl.stsd;
    if let re_mp4::StsdBoxContent::Mp4a(m) = &stsd.contents {
        let Some(esds) = &m.esds else {
            // An `mp4a` entry with no `esds` carries no AudioSpecificConfig, so the profile is
            // genuinely unknown. `mp4a.40` without the profile byte is the honest partial answer.
            return (
                Some("mp4a.40".to_string()),
                m.channelcount,
                m.samplerate.value() as u32,
                None,
            );
        };
        let dc = &esds.es_desc.dec_config;
        // Object type 0x40 is "MPEG-4 audio"; the audio object type (2 = AAC-LC, 5 = HE-AAC) is the
        // third field. RFC 6381 spells the OTI in hex and the audio object type in decimal —
        // `mp4a.40.2`, never `mp4a.40.02` — and players string-compare it, so the asymmetry is load
        // bearing rather than sloppy.
        let oti = dc.object_type_indication;
        let codec = if oti == 0x40 {
            format!("mp4a.{oti:02x}.{}", dc.dec_specific.profile)
        } else {
            format!("mp4a.{oti:02x}")
        };
        let rate =
            sample_rate_for(dc.dec_specific.freq_index).unwrap_or(m.samplerate.value() as u32);
        let chans = if dc.dec_specific.chan_conf > 0 {
            dc.dec_specific.chan_conf as u16
        } else {
            m.channelcount
        };
        let asc = audio_specific_config(
            dc.dec_specific.profile,
            dc.dec_specific.freq_index,
            dc.dec_specific.chan_conf,
        );
        return (Some(codec), chans, rate, Some(asc));
    }

    (None, 0, 0, None)
}

/// Rebuild the AAC `AudioSpecificConfig` from the `esds` descriptor's parsed fields.
///
/// **Why this is rebuilt rather than sliced out of the file.** An AAC decoder needs the
/// `AudioSpecificConfig` — it is the `extra_data` without which the first packet cannot be
/// interpreted at all. `re_mp4` parses the descriptor into fields and does not retain the original
/// bytes, so there is nothing to slice; the two bytes are re-encoded from the parsed values.
///
/// The layout (ISO/IEC 14496-3) is five bits of audio object type, four of sampling-frequency
/// index, four of channel configuration, then three flag bits (frame length, core-coder dependency,
/// extension) that are zero for the plain AAC-LC case every MP4 on the web uses:
///
/// ```text
///   AAAAA FFFF CCCC 000
/// ```
///
/// Only the two-byte form is emitted. A frequency index of 15 means the rate is written out
/// explicitly as a 24-bit field, which this descriptor does not carry — so that case would need
/// the original bytes and is not synthesised here rather than being guessed at.
fn audio_specific_config(audio_object_type: u8, freq_index: u8, chan_conf: u8) -> Vec<u8> {
    let bits = ((audio_object_type as u16 & 0x1F) << 11)
        | ((freq_index as u16 & 0x0F) << 7)
        | ((chan_conf as u16 & 0x0F) << 3);
    vec![(bits >> 8) as u8, (bits & 0xFF) as u8]
}

/// The MPEG-4 audio sampling-frequency table. Index 15 means "written out explicitly", which the
/// descriptor above does not carry, so it is reported as unknown rather than guessed.
fn sample_rate_for(freq_index: u8) -> Option<u32> {
    const RATES: [u32; 13] = [
        96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
    ];
    RATES.get(freq_index as usize).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_the_forms_a_player_appends() {
        assert_eq!(
            sniff(&[0, 0, 0, 0x18, b'f', b't', b'y', b'p']),
            Container::Mp4
        );
        // A media segment has no `ftyp` — this is the case a naive sniffer drops.
        assert_eq!(
            sniff(&[0, 0, 0, 0x10, b'm', b'o', b'o', b'f']),
            Container::Mp4
        );
        assert_eq!(
            sniff(&[0x1A, 0x45, 0xDF, 0xA3, 1, 2, 3, 4]),
            Container::WebM
        );
        assert_eq!(sniff(b"<!doctype html>"), Container::Unknown);
        assert_eq!(sniff(&[0, 0]), Container::Unknown);
    }

    #[test]
    fn webm_is_named_not_blamed() {
        // The point: a container we recognise and cannot read reports itself, rather than
        // surfacing as a parse failure that reads like corrupt bytes.
        let err = demux(&[0x1A, 0x45, 0xDF, 0xA3, 1, 2, 3, 4]).unwrap_err();
        assert_eq!(err, DemuxError::Unsupported(Container::WebM));
    }

    #[test]
    fn a_short_append_asks_for_more_rather_than_failing() {
        assert_eq!(demux(&[0, 0, 0]).unwrap_err(), DemuxError::Incomplete);
    }

    /// Sub-millisecond seams between consecutive frames are one range, not many. This is the
    /// arithmetic that decides whether a player's fetch loop converges.
    #[test]
    fn rounding_seams_do_not_shatter_a_range() {
        let s = |pts: i64| Sample {
            id: 0,
            is_sync: true,
            offset: 0,
            size: 1,
            timescale: 30000,
            decode_timestamp: pts,
            presentation_timestamp: pts,
            duration: 1001,
        };
        let t = Track {
            id: 1,
            kind: TrackKind::Video,
            codec: None,
            width: 0,
            height: 0,
            channels: 0,
            sample_rate: 0,
            timescale: 30000,
            duration: 0,
            codec_config: None,
            // Three abutting frames, then a real one-second hole, then one more.
            samples: vec![s(0), s(1001), s(2002), s(60000)],
        };
        let b = t.buffered();
        assert_eq!(b.len(), 2, "a real gap splits; rounding seams must not");
        assert!((b[0].start - 0.0).abs() < 1e-9);
        assert!((b[0].end - 3003.0 / 30000.0).abs() < 1e-9);
        assert!((b[1].start - 2.0).abs() < 1e-9);
    }
}
