//! # G_VTT_CAPTIONS — the cue list is a LIST because two people talk at once
//!
//! Media step **M7**. Before this, `video.textTracks` was `[]` and `addTextTrack()` returned
//! `{cues: [], activeCues: [], mode: 'disabled'}` — an inert object reporting success.
//!
//! ## How each assertion here can go RED
//!
//! - **Overlapping cues.** RED, run: make `active_at` return the FIRST match instead of all of them
//!   (`Option<&Cue>` — it compiles and reads as reasonable). The second speaker vanishes for the
//!   whole span where both are live, and every single-cue assertion here still passes.
//! - **Hours are optional.** RED, run: require three colon-separated parts in `parse_timestamp` and
//!   `00:01.500 --> 00:04.000` — the common form — fails to parse. Because a bad timestamp skips the
//!   cue, the file does not error; it comes out EMPTY, which is the worse failure.
//! - **`NOTE` is a comment.** RED, run: drop the NOTE branch and a translator's private remark
//!   renders on screen as a caption.
//! - **Cue settings are not text.** RED, run: keep the whole line after `-->` and the viewer reads
//!   `align:start position:50%`.
//! - **Half-open interval.** RED, run: use `t <= end` and back-to-back cues both render for one
//!   instant — a flicker of doubled captions at every cue boundary in the file.
//! - **A `.srt` is refused, not mangled.** RED, run: accept `,` as the fraction separator and an SRT
//!   renamed `.vtt` parses with every timestamp silently wrong.

use manuk_media::VttTrack;

/// Deliberately exercises every trap at once: no-hours timestamps, a NOTE, cue settings, an id
/// line, a multi-line payload, out-of-order cues, and two OVERLAPPING speakers.
///
/// **The NOTE body deliberately CONTAINS A TIMESTAMP LINE**, and that detail was found by a RED
/// probe that failed to go red. With an ordinary prose NOTE, disabling the comment branch entirely
/// still produced the right answer — the block fell through to the generic "neither line is a
/// timing line, skip it" path, so the NOTE assertion was **vacuous**. Only a NOTE shaped like a
/// real cue can tell the two code paths apart.
const VTT: &str = "WEBVTT - Example captions

NOTE
00:00.500 --> 00:01.000
A translator's private remark that CONTAINS A TIMESTAMP LINE.

intro
00:00.000 --> 00:02.000 align:start position:50%
Hello, and welcome.

00:02.000 --> 00:05.000
ALICE: I'll take the first one.
It runs across two lines.

00:03.500 --> 00:06.000
BOB: And I'm talking over her.

00:10.000 --> 00:12.000
Out of order in the file.

00:07.000 --> 00:09.000
Comes earlier in time.
";

#[test]
fn captions_parse_and_overlapping_cues_are_all_reported() {
    let t = VttTrack::parse(VTT).expect("a WEBVTT file parses");

    assert_eq!(
        t.len(),
        5,
        "5 cues — the NOTE block is a comment, not a sixth cue: {:#?}",
        t.cues()
    );

    // ── Hours are optional, and the fraction is milliseconds. ────────────────────────────────
    assert_eq!(t.cues()[0].start, 0.0);
    assert_eq!(t.cues()[0].end, 2.0);
    assert_eq!(
        t.cues()[1].end,
        5.0,
        "`00:05.000` is 5 seconds — a parser demanding HH:MM:SS rejects this form and the track \
         comes out EMPTY rather than erroring"
    );

    // ── Cue settings are not caption text. ───────────────────────────────────────────────────
    assert_eq!(
        t.cues()[0].text,
        "Hello, and welcome.",
        "everything after the end timestamp is SETTINGS — keeping it prints \
         `align:start position:50%` to the viewer"
    );
    assert_eq!(t.cues()[0].id.as_deref(), Some("intro"));

    // ── A multi-line payload keeps its newline. ──────────────────────────────────────────────
    assert_eq!(
        t.cues()[1].text,
        "ALICE: I'll take the first one.\nIt runs across two lines."
    );

    // ── Cues are ordered by start time even when the file is not. ────────────────────────────
    let starts: Vec<f64> = t.cues().iter().map(|c| c.start).collect();
    let mut sorted = starts.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(starts, sorted, "cues are indexed in time order: {starts:?}");

    // ── THE CLAIM THE MODULE TURNS ON: two speakers at once. ─────────────────────────────────
    let both = t.active_at(4.0);
    assert_eq!(
        both.len(),
        2,
        "at t=4.0 ALICE (2.0-5.0) and BOB (3.5-6.0) are BOTH on screen. Answering this plural \
         question in the singular drops the second speaker for the whole overlap, and every \
         single-cue assertion in this file still passes. got: {both:#?}"
    );
    assert!(both.iter().any(|c| c.text.starts_with("ALICE")));
    assert!(both.iter().any(|c| c.text.starts_with("BOB")));

    // ── Half-open [start, end): back-to-back cues never both render. ─────────────────────────
    let at_boundary = t.active_at(2.0);
    assert_eq!(
        at_boundary.len(),
        1,
        "cue 0 ends exactly where cue 1 begins; an inclusive end renders both for one instant, \
         which is a flicker of doubled captions at EVERY boundary in the file. got: {at_boundary:#?}"
    );
    assert!(at_boundary[0].text.starts_with("ALICE"));

    // ── Gaps are silent, not sticky. ─────────────────────────────────────────────────────────
    assert!(
        t.active_at(6.5).is_empty(),
        "between cues nothing is on screen — a caption that lingers into the gap is the 'stuck \
         subtitle' bug"
    );
    assert!(t.active_at(100.0).is_empty(), "past the last cue, nothing");
}

#[test]
fn a_file_that_is_not_webvtt_is_refused_rather_than_guessed_at() {
    // An SRT renamed `.vtt` — a real thing people do. It has no signature AND uses `,` for the
    // fraction, so accepting it would mean every timestamp is silently wrong.
    let srt = "1\n00:00:01,500 --> 00:00:04,000\nHello.\n";
    assert!(
        VttTrack::parse(srt).is_err(),
        "no WEBVTT signature means we do not know what this is, and saying so beats parsing it \
         into plausible nonsense"
    );

    // The signature alone is a valid, empty track — not an error.
    let empty = VttTrack::parse("WEBVTT\n").expect("a header-only file is valid");
    assert!(empty.is_empty());

    // A malformed timestamp costs its own cue and nothing else: one bad line in a 900-cue file
    // must not cost the viewer the other 899.
    let partly_bad = "WEBVTT\n\n00:00.000 --> 00:02.000\nGood.\n\nnot:a:timestamp --> 00:04.000\nBad.\n\n00:05.000 --> 00:06.000\nAlso good.\n";
    let t = VttTrack::parse(partly_bad).expect("parses");
    assert_eq!(
        t.len(),
        2,
        "the malformed cue is skipped and its neighbours survive: {:#?}",
        t.cues()
    );
    assert_eq!(t.cues()[1].text, "Also good.");
}
