feat(loop tick 261): six ticks of correct captions, finally drawn

TICK SHAPE: the residue ticks 258/259/260 each ended on, and the only thing left in the caption arc.

Ticks 255-260 parse cues, hold them in a TextTrack, time them, fire cuechange, fetch <track src>,
and preserve placement. Every one of those hands cues to A PAGE'S OWN RENDERER. A plain <video> with
<track default> has none, because a document with no player library never draws a caption itself.
THE BROWSER IS SUPPOSED TO. Six green gates, and the viewer saw nothing — the failure class where
every layer you inspect looks correct because each one correctly hands off to the next, and the last
handoff is to nobody.

THE JOIN IS ACROSS THREE CRATES AND NEITHER END CAN SEE THE OTHER. The painter is in Rust and never
sees the DOM (LayoutBox.node is its only link; no builder signature takes a &Dom). The caption state
is in JavaScript, because that is where the TextTrack API is. So the wire runs:

  JS el.__publishCues() -> __setActiveCues(nodeId, json) -> thread-local ACTIVE_CUES
     -> manuk_js::active_cues() -> Page::caption_map() -> manuk_paint::CaptionMap
     -> caption_items(video_rect, cues) -> Rect + Text items, emitted after the element's own blit

The last hop is the channel `images` and `z_index` already use: a NODEID-KEYED SIDE MAP, resolved by
the page layer, which is the one layer that can see both sides. CueSettings' doc comment (tick 260)
deferred pixel resolution to "a renderer that knows the video box"; caption_items IS that renderer,
and it is where `auto` finally has to mean something.

THREE CLAIMS THAT ARE NOT DETAILS:
  · ACTIVE_CUES IS STATE, NOT A QUEUE. Every other host bridge here (PENDING_HISTORY, PENDING_OPENS)
    is drained by the host. A caption is on screen until it isn't; a paint that CONSUMED the cue set
    would show each caption for exactly one frame and then blank the picture.
  · AN EMPTY ARRAY MUST BE SENT. It is how a cue leaves the screen. A bridge that only ever added
    cues would burn the last caption of every video permanently into the frame.
  · `hidden` IS NOT `disabled`, and this is where it finally bites. activeCues answers for a hidden
    track — cues live, cuechange still firing, so a page's own renderer keeps working — but `hidden`
    means exactly "do not display this" and is the mode a player sets WHEN IT DRAWS CAPTIONS ITSELF.
    So publishing filters on mode === 'showing', and the mode setter publishes UNCONDITIONALLY:
    showing->hidden leaves activeCues identical, the cue-diff correctly sees no change and fires
    nothing, and the overlay would keep painting a caption the user just turned off.

VTTCue.line carries THREE things in one property — 'auto', a bare number (a LINE COUNT, possibly
negative), or a '%'-suffixed string. Number('10%') is NaN; parseFloat on a line count loses the
distinction. Neither alone will do, and line:0 reads correctly under both, which is what lets the
bug survive.

RED PROBES: FOUR, AND ONE CAME BACK GREEN.
  · drop the caption emit from push_group -> RED "the active auto-line cue was not painted at all"
  · collapse `auto` to line 0            -> RED auto (21.6) not below line:0 (21.6)
  · publish `hidden` tracks too          -> RED "a `hidden` track still painted"
  · paint cues BEFORE the video          -> GREEN. THE ASSERTION COULD NOT FAIL.

The paint-order check was written against DisplayItem::Image, and a <video> WITH NO POSTER DECODES
NO BITMAP — so there was no Image item, `if let Some(img_idx)` skipped the assertion entirely, and it
passed under a probe that painted every caption behind the frame. Third distinct disguise of the
vacuous-assertion class in three ticks (tick 260's was a substring that also matched the bug it
hunted). THE FIX IS THE SAME EVERY TIME: MAKE THE THING YOU ASSERT ABOUT EXIST UNCONDITIONALLY. The
video now carries background:#123456 and the item is found by that exact colour — "the first Rect"
would be wrong too, now that captions emit Rects. The reversal probe then goes RED at "the caption
paints at 1, BEHIND the video at 4".

PROCESS FAILURE, RECORDED BECAUSE IT COST REAL WORK: the probe harness undid each probe with
`git checkout -- <file>`, which reverts to HEAD and DESTROYED THE TICK'S OWN UNCOMMITTED WORK. Two
files were wiped mid-tick and rebuilt from context. The rule "never git checkout a file holding
unlanded work" was already written down; the trap is that a PROBE HARNESS is exactly where you write
it without thinking, because undoing a probe feels like undoing a scratch edit. Snapshot with cp,
restore from the snapshot. The edit-applied assertion is what surfaced the wipe — keep it.

REGRESSION CHECK: g_track_src, g_cue_change, g_text_tracks, g_dialog_render, g_popover_render,
manuk-media vtt_captions and all 17 manuk-paint tests green. Workspace check clean under both
feature configurations (the dom_bindings cfg hazard: ActiveCue lives in lib.rs, not behind _sm).
manuk-shell 58+2 green standalone — its verify failure was the known load flake (load 4-5).

Gate: engine/page/tests/g_caption_paint.rs (G_CAPTION_PAINT), asserting the DISPLAY LIST rather than
pixels — DisplayItem::Text carries a plain String (shaping is deferred to CpuPainter::draw_text), so
the cue's own words are legible at that layer and a pixel diff would only obscure which cue landed
where. ONE #[test] per file: PageContext is per-process.

RESIDUE, NAMED RATHER THAN IMPLIED: the arc is closed end-to-end, and what is left is FIDELITY, not
absence. `vertical: rl/lr` paints horizontally (recorded on CaptionCue rather than dropped, so the
gap stays visible). Text width is ESTIMATED (chars x size x 0.5) rather than shaped, so align:end and
size: clipping are approximate — the same stub-metrics problem as the ch/ex lever. No cue-box overlap
avoidance for two simultaneous cues at equal explicit `line`. DisplayList::damage_since over-damages
Text (a hardcoded 4096px box), so a caption change repaints the row rather than the cue.

WIKI: docs/wiki/media-pipeline.md
