//! # WebM / Matroska demux — the EBML half of the container layer
//!
//! [`crate::demux`] has answered `Unsupported(WebM)` since M3, and the module doc said exactly why:
//! *"[`sniff`] recognises it — so the failure is a named `Unsupported`, not a parse error blamed on
//! the bytes — but there is no EBML reader here."* This is that reader.
//!
//! **Why it is the rung that was missing.** The MP4 ladder went demux → AAC → H.264 → playback, one
//! rung per tick, and each decoder step could be a decoder step because the container step had
//! already produced a sample table. WebM had no rung 1, so a VP9 or Opus decoder would have had
//! nothing to feed it: no tracks, no timestamps, no byte ranges. Everything a `<video src="x.webm">`
//! or an MSE append of a WebM segment could report — duration, dimensions, `buffered` — was
//! structurally unavailable, and would have stayed unavailable however good the decoder was.
//!
//! **Borrow, do not build** (`docs/loop/MEDIA.md`, the standing rule this crate is organised
//! around). `matroska-demuxer` is the EBML reader the way `re_mp4` is the box reader: it walks the
//! Segment, resolves `SeekHead`/`Cues`, parses `Tracks`, and yields frames out of `Cluster`s
//! including the `SimpleBlock` **lacing** and `BlockGroup` forms that a hand-rolled reader gets
//! wrong first. It is Zlib OR MIT OR Apache-2.0 and — the reason it was chosen over the four
//! alternatives — it has **zero transitive dependencies**, so it may sit in this crate's *default*
//! feature set without putting anything into the ~25 gate binaries that reach `manuk-media` through
//! `manuk-js`. That constraint is what ruled symphonia's `mkv` reader out: symphonia is already here
//! behind `--features audio` precisely so it stays out of that link, and it is audio-only anyway.
//!
//! ## The two places this module does real work rather than forwarding
//!
//! **1. Byte offsets, and they are VERIFIED, not asserted.** [`crate::Sample`] carries an
//! `offset`/`size` into the buffer that was parsed, because that is the coordinate space a decoder
//! reads from. `matroska-demuxer` hands back a frame's *copied bytes* and no offset, so the offset
//! has to be recovered. The obvious recovery — "the reader's position minus the frame length" — is
//! right for a plain `SimpleBlock` and **wrong** for two real cases that the fixtures contain:
//!
//! * **lacing**, where one block carries several frames read in a single pass, so every frame but
//!   the last ends before the reader's position; and
//! * **`BlockGroup`**, where a `BlockDuration` element is read *after* the block's data.
//!
//! Measured on `bear-vp9-opus.webm`: 217 of 218 frames land on the fast path and one does not. So
//! the fast path is a *hint*, and every offset — hint or not — is confirmed by comparing
//! `bytes[offset..offset+size]` against the frame the demuxer actually returned. On a mismatch the
//! frame is located by a forward scan bounded by the previous frame's end and the reader's current
//! position, which is a search inside one block. **A frame that cannot be located is a hard
//! [`DemuxError::Invalid`]** naming the frame, never a plausible-looking offset: a wrong offset
//! feeds a decoder garbage that decodes into a green frame, which is the silent-failure shape this
//! project keeps finding one layer below where it looks.
//!
//! **The shape of the wrong answer is worth recording, because the obvious gate does not see it.**
//! The naive recovery does not produce offsets that run off the end, and it does not produce
//! overlapping ranges either — the one bad frame comes out *shifted by six bytes*, still inside the
//! buffer and still disjoint from its neighbours. Both structural checks pass on it. Only a check
//! against the **codec's own framing** (a VP9 frame marker, an Opus TOC byte) sees it, which is what
//! `tests/webm_demux.rs` asserts, and which was found by running the RED probe rather than by
//! reading the test.
//!
//! **2. Frame durations, which Matroska mostly does not store.** MP4's `stts` gives every sample a
//! duration; a WebM `SimpleBlock` gives a timestamp and nothing else. Without durations every
//! sample's presentation span is empty, [`crate::Track::buffered`] filters them all out, and
//! `SourceBuffer.buffered` comes back empty — the exact "the append loop cannot advance" failure
//! `buffered` exists to prevent. Three sources, in order of authority: the block's own duration
//! (`BlockGroup`), the track's `DefaultDuration`, and otherwise the **delta to the next frame on the
//! same track**, with the last frame reusing the previous delta. `bear-vp9-opus.webm` needs all
//! three arms: its video track has a `DefaultDuration` and its Opus track has none, so the audio
//! timeline exists only because of the delta arm.
//!
//! ## Nanosecond timescale, on purpose
//!
//! Matroska stores timestamps in `TimecodeScale` units (1 ms by default) and `DefaultDuration` in
//! **nanoseconds**. The bear fixtures' `DefaultDuration` is `33_366_666` ns — 33.366 ms, which is
//! not an integer number of milliseconds. Expressing samples in the file's own 1 ms tick would round
//! every frame and drift ~30 ms over 82 frames. So [`crate::Track::timescale`] is set to
//! **1,000,000,000** and every timestamp is carried in nanoseconds, which is exact for both
//! quantities and for any `TimecodeScale` a file can declare. `presentation_start()` divides it back
//! to seconds and no caller sees the difference.
//!
//! ## What this does NOT claim
//!
//! **No codec is decoded here and none is advertised.** Reporting `vp9` means "the container says
//! this track is VP9", not "we can decode VP9" — the same distinction the MP4 arm draws, and the one
//! `docs/loop/MEDIA.md` warns is the difference between a working page and a black rectangle. The
//! decode registry `MediaSource.isTypeSupported` reads still answers **false** for every
//! `video/webm; codecs="…"` string, and `HTMLMediaElement.canPlayType` still answers `''` for WebM.
//! The one thing that changes at the JS boundary is the *bare* container form — `video/webm` with no
//! `codecs=` parameter — which now means what it already meant for MP4: we can open this container.
//!
//! **Codec strings are what the container states, not a guess.** `V_VP9` becomes `vp9` and not
//! `vp09.00.10.08`: the RFC 6381 long form encodes profile, level and bit depth, and a WebM track
//! only carries those in a `CodecPrivate` that neither bear fixture has. Inventing plausible digits
//! would produce a string a player string-compares against `isTypeSupported` and branches on.
//! `V_AV1` is the exception and only because it is derivable: its `CodecPrivate` **is** an `av1C`
//! record, so the profile/level/tier/depth come out of the bytes.

use crate::{DemuxError, Movie, Sample, Track, TrackKind};
use matroska_demuxer::{Frame, MatroskaFile, TrackType};
use std::cell::Cell;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::rc::Rc;

/// Every timestamp this module produces is in nanoseconds. See the module doc.
const NS_PER_SECOND: u32 = 1_000_000_000;

/// A reader that remembers where it is.
///
/// `MatroskaFile` owns its reader and exposes no accessor for it, so the only way to know how far it
/// has read — which is the hint the offset recovery starts from — is to wrap the reader before
/// handing it over.
struct Tracked<R> {
    inner: R,
    pos: Rc<Cell<u64>>,
}

impl<R: Read> Read for Tracked<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.pos.set(self.pos.get() + n as u64);
        Ok(n)
    }
}

impl<R: Seek> Seek for Tracked<R> {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let at = self.inner.seek(to)?;
        self.pos.set(at);
        Ok(at)
    }
}

/// Demux a whole WebM/Matroska buffer. Called by [`crate::demux`] once [`crate::sniff`] has said
/// EBML; the container check is not repeated here.
pub fn demux_webm(bytes: &[u8]) -> Result<Movie, DemuxError> {
    let pos = Rc::new(Cell::new(0u64));
    let reader = Tracked {
        inner: Cursor::new(bytes),
        pos: Rc::clone(&pos),
    };
    // A truncated buffer is the normal MSE case — "come back with more", not a broken file. The
    // demuxer reports it as an IO error because that is what a short read is at its layer.
    let mut mkv = MatroskaFile::open(reader).map_err(map_open_error)?;

    let mut tracks: Vec<Track> = mkv
        .tracks()
        .iter()
        .map(|t| {
            let kind = match t.track_type() {
                TrackType::Video => TrackKind::Video,
                TrackType::Audio => TrackKind::Audio,
                TrackType::Subtitle => TrackKind::Subtitle,
                _ => TrackKind::Other,
            };
            let video = t.video();
            let audio = t.audio();
            Track {
                id: t.track_number().get() as u32,
                kind,
                codec: codec_string(t.codec_id(), t.codec_private()),
                width: video.map_or(0, |v| v.pixel_width().get() as u32),
                height: video.map_or(0, |v| v.pixel_height().get() as u32),
                channels: audio.map_or(0, |a| a.channels().get() as u16),
                sample_rate: audio.map_or(0, |a| a.sampling_frequency() as u32),
                timescale: NS_PER_SECOND,
                duration: 0, // filled from Info below — Matroska stores duration per SEGMENT
                codec_config: t.codec_private().map(|p| p.to_vec()),
                samples: Vec::new(),
            }
        })
        .collect();

    // `Duration` lives on the Segment's `Info`, in TimecodeScale units, and is a float. A LIVE
    // stream has none at all (`bear-320x240-live.webm`) — 0 is the honest answer there, and
    // `Movie::duration_seconds` already treats it as "not known yet" rather than "zero long".
    let scale = mkv.info().timestamp_scale().get();
    let segment_ns = mkv
        .info()
        .duration()
        .filter(|d| d.is_finite() && *d > 0.0)
        .map(|d| (d * scale as f64) as u64)
        .unwrap_or(0);
    for t in &mut tracks {
        t.duration = segment_ns;
    }

    // ── The frame pass. Frames arrive interleaved across tracks in storage order.
    struct Raw {
        track: u64,
        timestamp_ns: i64,
        offset: u64,
        size: u64,
        is_sync: bool,
        duration_ns: Option<u64>,
    }
    let mut raw: Vec<Raw> = Vec::new();
    let mut frame = Frame::default();
    // The forward-scan floor: a frame is never located before the end of the previous one.
    let mut located_end: usize = 0;

    loop {
        match mkv.next_frame(&mut frame) {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) => return Err(map_frame_error(e, raw.len())),
        }
        let at = pos.get() as usize;
        let offset = locate(bytes, &frame.data, located_end, at, raw.len())?;
        located_end = offset + frame.data.len();

        let kind = tracks
            .iter()
            .find(|t| t.id as u64 == frame.track)
            .map(|t| t.kind)
            .unwrap_or(TrackKind::Other);
        raw.push(Raw {
            track: frame.track,
            timestamp_ns: (frame.timestamp as i64).saturating_mul(scale as i64),
            offset: offset as u64,
            size: frame.data.len() as u64,
            // `is_keyframe` is `None` for the `BlockGroup` form, where keyframe-ness is the ABSENCE
            // of a `ReferenceBlock` and this demuxer does not report it. Audio is all-sync by
            // construction; for video an unknown must be `false`, because the only consumer is a
            // seek looking for a frame that decodes standalone, and guessing `true` there hands the
            // decoder a frame it cannot start from.
            is_sync: frame.is_keyframe.unwrap_or(kind == TrackKind::Audio),
            duration_ns: frame
                .duration
                .map(|d| d.saturating_mul(scale))
                .or_else(|| default_duration_ns(mkv.tracks(), frame.track)),
        });
    }

    // ── Durations for the frames that carry none: the delta to the next frame on the SAME track.
    // Done after the pass because it needs the successor, and per track because the stream is
    // interleaved. The last frame of a track reuses the previous delta rather than reporting a zero
    // span, which `Track::buffered` would drop — losing the tail of every `buffered` range.
    for t in &tracks {
        let idx: Vec<usize> = raw
            .iter()
            .enumerate()
            .filter(|(_, r)| r.track == t.id as u64)
            .map(|(i, _)| i)
            .collect();
        let mut last_delta: Option<u64> = None;
        for (n, &i) in idx.iter().enumerate() {
            if raw[i].duration_ns.is_some() {
                continue;
            }
            let d = idx
                .get(n + 1)
                .map(|&j| (raw[j].timestamp_ns - raw[i].timestamp_ns).max(0) as u64)
                .filter(|d| *d > 0)
                .or(last_delta);
            raw[i].duration_ns = d;
            if let Some(d) = d {
                last_delta = Some(d);
            }
        }
        // The tail frame of a track that never learned a delta (a single-frame track) still needs a
        // span, or the track reports `buffered.length === 0` while holding media.
        for &i in &idx {
            if raw[i].duration_ns.is_none() {
                raw[i].duration_ns = last_delta;
            }
        }
    }

    for (n, r) in raw.iter().enumerate() {
        if let Some(t) = tracks.iter_mut().find(|t| t.id as u64 == r.track) {
            t.samples.push(Sample {
                id: n as u32,
                is_sync: r.is_sync,
                offset: r.offset,
                size: r.size,
                timescale: NS_PER_SECOND,
                // Matroska block timestamps are PRESENTATION timestamps. There is no separate
                // decode timestamp in the container — B-frame reordering is the codec's business
                // here — so the two are equal by construction, not by an assumption we could get
                // wrong.
                decode_timestamp: r.timestamp_ns,
                presentation_timestamp: r.timestamp_ns,
                duration: r.duration_ns.unwrap_or(0),
            });
        }
    }

    Ok(Movie {
        tracks,
        // There is no init/media segment split at the container level in WebM the way `moof` marks
        // one in MP4 — an MSE WebM stream is an EBML header plus Clusters, and the accumulated
        // buffer a `SourceBuffer` hands us always starts at the header. `fragmented` names the MP4
        // fragment form specifically, so the honest answer for every WebM file is `false`.
        fragmented: false,
    })
}

/// Where in `bytes` this frame's data lives.
///
/// `hint` is the reader's position; the frame usually ends exactly there. `floor` is the end of the
/// previous frame — the scan never looks behind it, so a repeated payload cannot match an earlier
/// copy. See the module doc for why the hint is not enough.
fn locate(
    bytes: &[u8],
    data: &[u8],
    floor: usize,
    hint_end: usize,
    frame_index: usize,
) -> Result<usize, DemuxError> {
    let len = data.len();
    if len == 0 {
        // A zero-length frame has no bytes to find and no bytes to decode. Anchor it at the floor.
        return Ok(floor.min(bytes.len()));
    }
    if hint_end <= bytes.len() && hint_end >= len {
        let start = hint_end - len;
        if start >= floor && &bytes[start..hint_end] == data {
            return Ok(start);
        }
    }
    // The slow path: lacing or a BlockGroup put the frame somewhere inside the block. Scan forward
    // from the previous frame's end, bounded by where the reader now is.
    let end = hint_end.min(bytes.len());
    if end > floor && end - floor >= len {
        let window = &bytes[floor..end];
        for start in 0..=(window.len() - len) {
            if &window[start..start + len] == data {
                return Ok(floor + start);
            }
        }
    }
    Err(DemuxError::Invalid(format!(
        "frame {frame_index} ({len} bytes) could not be located in the buffer it was read from"
    )))
}

/// `DefaultDuration` is per track and already in nanoseconds — the one Matroska field that is not in
/// `TimecodeScale` units.
fn default_duration_ns(tracks: &[matroska_demuxer::TrackEntry], number: u64) -> Option<u64> {
    tracks
        .iter()
        .find(|t| t.track_number().get() == number)
        .and_then(|t| t.default_duration())
        .map(|d| d.get())
}

/// A Matroska `CodecID` as the string a page compares against.
///
/// The short forms (`vp9`, `vp8`, `opus`, `vorbis`) are what a WebM `codecs=` parameter carries in
/// practice and what the WebM project's own guidance specifies; the RFC 6381 long forms need
/// per-codec configuration records that a WebM track is not required to carry. AV1 is the exception
/// because its `CodecPrivate` *is* the `av1C` record.
fn codec_string(codec_id: &str, private: Option<&[u8]>) -> Option<String> {
    match codec_id {
        "V_VP8" => Some("vp8".into()),
        "V_VP9" => Some("vp9".into()),
        "V_AV1" => Some(av1_codec_string(private).unwrap_or_else(|| "av01".into())),
        "V_THEORA" => Some("theora".into()),
        "V_MPEG4/ISO/AVC" => Some("avc1".into()),
        "V_MPEGH/ISO/HEVC" => Some("hvc1".into()),
        "A_OPUS" => Some("opus".into()),
        "A_VORBIS" => Some("vorbis".into()),
        "A_AAC" => Some("mp4a.40.2".into()),
        "A_MPEG/L3" => Some("mp3".into()),
        "A_FLAC" => Some("flac".into()),
        // A `CodecID` we do not recognise is `None` and not a guess, exactly as the MP4 arm does for
        // an unknown sample entry: a player is better served by "unknown" than by a string it will
        // branch on.
        _ => None,
    }
}

/// `av01.P.LLT.DD` from the `av1C` configuration record (AV1-ISOBMFF §2.3.1), which is what a WebM
/// AV1 track's `CodecPrivate` holds.
///
/// Byte 0 is `marker(1) | version(7)`; byte 1 is `seq_profile(3) | seq_level_idx(5)`; byte 2 is
/// `seq_tier(1) | high_bitdepth(1) | twelve_bit(1) | monochrome(1) | …`.
fn av1_codec_string(private: Option<&[u8]>) -> Option<String> {
    let p = private?;
    if p.len() < 3 || p[0] & 0x80 == 0 {
        return None;
    }
    let profile = p[1] >> 5;
    let level = p[1] & 0x1f;
    let tier = if p[2] & 0x80 != 0 { 'H' } else { 'M' };
    let high_bitdepth = p[2] & 0x40 != 0;
    let twelve_bit = p[2] & 0x20 != 0;
    let depth = match (twelve_bit, high_bitdepth) {
        (true, _) => 12,
        (false, true) => 10,
        (false, false) => 8,
    };
    Some(format!("av01.{profile}.{level:02}{tier}.{depth:02}"))
}

/// `MatroskaFile::open` on a buffer that has not fully arrived reads past the end. That is
/// [`DemuxError::Incomplete`] — a normal answer during an MSE append, not a broken file.
fn map_open_error(e: matroska_demuxer::DemuxError) -> DemuxError {
    match e {
        matroska_demuxer::DemuxError::IoError(io)
            if io.kind() == std::io::ErrorKind::UnexpectedEof =>
        {
            DemuxError::Incomplete
        }
        other => DemuxError::Invalid(other.to_string()),
    }
}

/// A short read *during the frame pass* means the last Cluster is truncated. Everything before it
/// parsed, but this demuxer returns a whole `Movie` or nothing, so the buffer is incomplete rather
/// than invalid — and a player that appended more bytes gets a real answer next time.
fn map_frame_error(e: matroska_demuxer::DemuxError, frames_so_far: usize) -> DemuxError {
    match e {
        matroska_demuxer::DemuxError::IoError(io)
            if io.kind() == std::io::ErrorKind::UnexpectedEof =>
        {
            DemuxError::Incomplete
        }
        other => DemuxError::Invalid(format!("after {frames_so_far} frames: {other}")),
    }
}
