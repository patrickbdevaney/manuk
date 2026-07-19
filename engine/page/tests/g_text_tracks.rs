//! **G_TEXT_TRACKS — the caption API the adaptive players actually call.**
//!
//! Tick 255 built the WebVTT parser; this is the surface a page reaches captions through.
//! `video.textTracks` was `[]` and `addTextTrack()` returned
//! `{cues: [], activeCues: [], mode: 'disabled'}` — an object that accepts every call, reports
//! success and holds nothing. A player added 900 cues to it and rendered none.
//!
//! **Why this surface and not `<track src>`:** hls.js and dash.js ship their own WebVTT parsers and
//! call `addTextTrack` + `addCue`, because segmented streams carry captions inside the media
//! segments rather than as a separate file. This is the path captions take on the streaming sites
//! the media track is aimed at.
//!
//! ## How each assertion here can go RED
//!
//! - **`mode` gates `activeCues`.** A `TextTrack` defaults to `'disabled'`, and a disabled track has
//!   NO active cues — that is how "captions off" is represented. RED, run: drop the mode check and
//!   subtitles render for a user who turned them off, on a track no player has enabled yet.
//! - **`activeCues` is a LIST.** RED, run: return the first match — the second speaker vanishes for
//!   the whole overlap (the same claim as tick 255's `active_at`, one layer up).
//! - **Half-open `[start, end)`.** RED, run: `t <= endTime` and back-to-back cues both render for
//!   an instant, flickering doubled captions at every boundary.
//! - **`VTTCue` exists.** RED, run: delete it and the player's caption path dies on a
//!   `ReferenceError` — a throw, not a missing feature, so whatever it had not yet done stays undone.

use manuk_page::Page;
use manuk_text::FontContext;

const W: f32 = 800.0;

/// One test — a `PageContext` is per-process, see `g_mouse_actuation.rs`.
#[test]
fn a_player_can_add_cues_and_read_back_only_the_ones_that_are_showing() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<video id="v"></video><div id="log"></div>
<script>
  var v = document.getElementById('v');

  // Exactly what hls.js/dash.js do: construct cues themselves, hand them over.
  var t = v.addTextTrack('captions', 'English', 'en');
  t.addCue(new VTTCue(0.0, 2.0, 'Hello, and welcome.'));
  t.addCue(new VTTCue(2.0, 5.0, "ALICE: I'll take the first one."));
  t.addCue(new VTTCue(3.5, 6.0, "BOB: And I'm talking over her."));

  window.__report = function (stage) {
    var active = t.activeCues;
    var texts = [];
    for (var i = 0; i < active.length; i++) { texts.push(active[i].text); }
    document.getElementById('log').textContent =
      stage +
      ' tracks=' + v.textTracks.length +
      ' kind=' + v.textTracks[0].kind + ' lang=' + v.textTracks[0].language +
      ' cues=' + t.cues.length +
      ' mode=' + t.mode +
      ' active=' + active.length +
      ' texts=' + texts.join('|') +
      ' cueTrack=' + (t.cues[0].track === t) +
      ' hasTextTrackCue=' + (typeof TextTrackCue !== 'undefined');
  };
</script></body>"#,
        "https://tracks.test/",
        &fonts,
        W,
    );

    let root = p.dom().root();
    let lg = manuk_css::query_selector_all(p.dom(), root, "#log")[0];

    // ── 1. DISABLED BY DEFAULT: cues are held, none are active. ─────────────────────────────
    p.eval_for_test("v.currentTime = 4.0; window.__report('off');");
    let out = p.dom().text_content(lg);
    assert!(
        !out.is_empty(),
        "the report never ran — the caption path threw, which is what a missing VTTCue does"
    );
    assert!(
        out.contains("tracks=1"),
        "the track joins textTracks; got: {out}"
    );
    assert!(
        out.contains("kind=captions") && out.contains("lang=en"),
        "addTextTrack's arguments are kept — a player enumerates textTracks to find the user's \
         language. got: {out}"
    );
    assert!(
        out.contains("cues=3"),
        "all three cues are held; got: {out}"
    );
    assert!(
        out.contains("mode=disabled"),
        "the spec default is off; got: {out}"
    );
    assert!(
        out.contains("active=0"),
        "A DISABLED TRACK HAS NO ACTIVE CUES — that IS 'captions off'. Serving cues regardless of \
         mode renders subtitles for a user who turned them off, on a track no player has enabled \
         yet. got: {out}"
    );
    assert!(
        out.contains("cueTrack=true"),
        "addCue backlinks the cue to its track; got: {out}"
    );
    assert!(
        out.contains("hasTextTrackCue=true"),
        "players feature-detect TextTrackCue before VTTCue; got: {out}"
    );

    // ── 2. SHOWING: overlapping cues are BOTH active. ────────────────────────────────────────
    p.eval_for_test("t.mode = 'showing'; window.__report('on');");
    let out = p.dom().text_content(lg);
    assert!(
        out.contains("active=2"),
        "at t=4.0 ALICE (2.0-5.0) and BOB (3.5-6.0) are BOTH showing. Answering this plural \
         question in the singular drops the second speaker for the whole overlap. got: {out}"
    );
    assert!(
        out.contains("ALICE") && out.contains("BOB"),
        "both speakers' text is present; got: {out}"
    );

    // ── 3. Half-open [start, end): back-to-back cues never both render. ──────────────────────
    p.eval_for_test("v.currentTime = 2.0; window.__report('boundary');");
    let out = p.dom().text_content(lg);
    assert!(
        out.contains("active=1") && out.contains("ALICE"),
        "cue 0 ends exactly where cue 1 begins; an inclusive end flickers doubled captions at \
         EVERY boundary in the file. got: {out}"
    );

    // ── 4. Gaps are silent, and the timeline is driven by currentTime. ───────────────────────
    p.eval_for_test("v.currentTime = 9.0; window.__report('gap');");
    let out = p.dom().text_content(lg);
    assert!(
        out.contains("active=0"),
        "past the last cue nothing shows — a caption that lingers is the 'stuck subtitle' bug, and \
         this also proves activeCues tracks currentTime rather than being computed once. got: {out}"
    );
}
