//! # G_MEDIA_WEBM_AV1 — a WebM's AV1 track decodes to a real picture, and its VP9 neighbour does not
//!
//! t633 built the EBML reader and held every `codecs=` claim at false, on the stated ground that
//! "no VP9 and no Opus decoder exists anywhere in the tree". True, and incomplete: **AV1 has
//! decoded here since t354**, and AV1-in-WebM is the pairing a modern Chrome is actually served.
//! This gate is the join — real WebM bytes, the real EBML sample table, the real dav1d decoder —
//! and it is the evidence behind the `isTypeSupported` answer that moves with it.
//!
//! ## Why the negative half is not decoration
//!
//! A decoder that said yes to everything would pass every positive assertion in this file. So the
//! same `can_decode_video` must **refuse** the VP9 track of `bear-vp9-opus.webm` — same crate,
//! same feature set, same call. Without that, "AV1 decodes" is a claim about the fixture rather
//! than about the engine.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! | shift every WebM sample offset by 6 bytes (the t633 lacing bug) | RED — `decode_sample` **errors**; it does not merely return no frame |
//! | `av1::can_decode` returns `true` for any video track | RED — the VP9 refusal assert fires |
//! | `presentation_time` stops riding through dav1d (hardcode `0.0`) | RED — the per-frame pts assert fires, and nothing else does |
//!
//! Each mutation lands on a *different* assertion, which is the point of tabulating them: three
//! asserts that all fail together are one assert wearing three names.
//!
//! ## Cost
//!
//! Only the first [`PREFIX`] samples are decoded. dav1d in a `--profile test` build runs about a
//! second a frame, so decoding all 82 would put ~80s on any wall that ever runs this. The picture
//! assertions do not get better after the eighth frame; the wall cost does.

use manuk_media::{can_decode_video, demux, Av1Decoder, TrackKind, VideoDecoder};

const AV1_WEBM: &[u8] = include_bytes!("data/bear-av1-480x360.webm");
const VP9_WEBM: &[u8] = include_bytes!("data/bear-vp9-opus.webm");

/// How many samples to push through dav1d. See the cost note above.
const PREFIX: usize = 8;

#[test]
fn an_av1_webm_decodes_to_pictures_at_the_declared_size() {
    let movie = demux(AV1_WEBM).expect("the AV1 WebM must demux (t633's EBML reader)");
    let track = movie
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Video)
        .expect("the fixture carries a video track");

    assert_eq!(
        track.codec.as_deref(),
        Some("av01.0.01M.08"),
        "the codec string comes from the track's own av1C CodecPrivate, not from a guess"
    );
    assert_eq!((track.width, track.height), (480, 360));
    assert!(
        can_decode_video(track),
        "with the av1 feature compiled in, an av01 track in a WebM is decodable — the container \
         is not part of that question, and answering no is the false absence this gate closes"
    );

    let mut decoder = Av1Decoder::new(track).expect("dav1d must open on this track");
    let mut frames = Vec::new();
    for s in track.samples.iter().take(PREFIX) {
        let range = s.byte_range();
        assert!(
            range.end <= AV1_WEBM.len(),
            "sample byte range must land inside the file"
        );
        if let Some(f) = decoder
            .decode_sample(&AV1_WEBM[range], s.presentation_start())
            .expect("a real AV1 sample must not error the decoder")
        {
            frames.push(f);
        }
    }
    frames.extend(decoder.finish().expect("the drain must not error"));

    assert!(
        !frames.is_empty(),
        "the first {PREFIX} samples must produce at least one picture — zero frames is what a \
         six-byte offset shift produces, and it is the failure this gate was written for"
    );
    for f in &frames {
        assert_eq!(
            (f.width, f.height),
            (480, 360),
            "a decoded picture must match the track's declared size"
        );
        assert_eq!(
            f.rgba.len(),
            480 * 360 * 4,
            "RGBA must be tightly packed at that size"
        );
        assert!(
            !f.is_uniform(),
            "a flat field is what a mis-fed decoder produces; this fixture is a real photograph"
        );
    }

    // Presentation time rides THROUGH dav1d (av1.rs's module note). If it did not, every frame
    // would carry the same pts and the timeline would collapse — which dimension and content
    // checks cannot see.
    let mut times: Vec<f64> = frames.iter().map(|f| f.presentation_time).collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    times.dedup();
    assert_eq!(
        times.len(),
        frames.len(),
        "each picture must carry its own presentation time, not the decoder's call order"
    );
    assert_eq!(
        times[0], 0.0,
        "the first sample of this fixture starts at 0"
    );
}

#[test]
fn the_same_decoder_refuses_the_vp9_track_in_the_same_container() {
    let movie = demux(VP9_WEBM).expect("the VP9/Opus WebM must demux");
    let video = movie
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Video)
        .expect("bear-vp9-opus.webm carries a video track");

    // The SHORT form is deliberate (see `webm::codec_string`): a WebM VP9 track carries no `vpcC`
    // to derive `vp09.PP.LL.DD` from, and `vp9` is what a WebM `codecs=` parameter actually says.
    // Asserting it here is what keeps the JS-side refusal regex pointed at the string this
    // demuxer really emits rather than the one RFC 6381 would suggest.
    assert_eq!(
        video.codec.as_deref(),
        Some("vp9"),
        "the fixture's video track must really be VP9, or this proves nothing"
    );
    assert!(
        !can_decode_video(video),
        "there is no VP9 decoder in this tree and the board leaves VP9 on the floor deliberately \
         — a `true` here would be the black-rectangle lie MEDIA.md's MSE warning names"
    );

    // And the container is not what earns the yes: same reader, same file, opposite answer.
    let audio = movie
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Audio)
        .expect("bear-vp9-opus.webm carries an Opus track");
    assert_eq!(audio.codec.as_deref(), Some("opus"));
    assert!(
        !can_decode_video(audio),
        "an audio track is never video-decodable regardless of codec"
    );
}
