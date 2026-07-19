//! **G_CUE_CHANGE — the caption timeline fires, which is the only way a caption reaches a screen.**
//!
//! Tick 255 built the WebVTT parser, tick 256 built the `TextTrack` that holds cues and answers
//! `activeCues`. Both are correct and both are POLL-ONLY, and nothing polls. Every caption renderer
//! there is — the players' own overlays, and the `<track>` UI — is
//! `track.addEventListener('cuechange', render)`. So a track that computes the right active cues
//! and never fires is a track whose captions are computed and never shown: the same failure shape
//! as the inert object tick 256 replaced, one layer further along.
//!
//! ## How each assertion here can go RED
//!
//! - **`cuechange` fires at all.** RED, run: it did not exist before this tick — a page that renders
//!   captions the way every page renders captions saw nothing, forever, on a track holding cues.
//! - **It fires on CHANGE, not on every clock write.** A player writes `currentTime` on every frame.
//!   RED, run: fire unconditionally from the setter and a listener that redraws its caption node
//!   does a DOM write per frame for a line of text that did not change.
//! - **The active set is compared by IDENTITY, position by position — not by LENGTH.** RED, run:
//!   compare `a.length !== b.length` only, and seeking from one single-cue line straight to another
//!   (a click on the transcript — the common case) reports "no change" while the viewer sits on the
//!   previous caption.
//! - **A `disabled` track does not fire.** `disabled` is the spec default and is how "captions off"
//!   is represented. RED, run: drop the mode gate and a track no player has enabled fires at every
//!   boundary, so the listener renders subtitles the user turned off.
//! - **`mode = 'showing'` with a cue already under the playhead fires.** Turning captions ON is a
//!   state change the renderer has no other moment to learn about. RED, run: make `mode` a plain
//!   data property again and the caption stays blank until the clock happens to cross the NEXT
//!   boundary — on a long cue, that is many seconds of nothing.
//! - **Turning a showing track OFF fires.** The set went non-empty → empty, and the listener is what
//!   CLEARS the caption node. RED, run: skip it and the last caption stays burned on screen after
//!   the user turns captions off.
//! - **A cue appended over the current time fires immediately.** Live/segmented streams deliver cues
//!   while the clock runs. RED, run: skip the sync in `addCue` and a caption that is already due
//!   waits for the next clock write to appear.

use manuk_page::Page;
use manuk_text::FontContext;

const W: f32 = 800.0;

/// One test — a `PageContext` is per-process, see `g_mouse_actuation.rs`.
#[test]
fn the_caption_timeline_tells_the_renderer_exactly_when_the_caption_changed() {
    let fonts = FontContext::new();
    let p = Page::load(
        r#"<!doctype html><body>
<video id="v"></video><div id="log"></div>
<script>
  var v = document.getElementById('v');
  var t = v.addTextTrack('captions', 'English', 'en');
  t.addCue(new VTTCue(0.0, 2.0, 'A'));
  t.addCue(new VTTCue(2.0, 4.0, 'B'));
  t.addCue(new VTTCue(4.0, 6.0, 'C'));

  // Exactly what a caption renderer is: count the redraws, and record what it would have drawn.
  var fires = 0, drawn = '';
  t.addEventListener('cuechange', function () {
    fires++;
    var a = this.activeCues, s = [];
    for (var i = 0; i < a.length; i++) { s.push(a[i].text); }
    drawn = s.join('+');
  });

  var out = [];
  var mark = function (name) { out.push(name + '=' + fires + ':' + (drawn || '-')); };

  // 1. DISABLED (the default). The clock crosses every boundary; captions are OFF, so a renderer
  //    that drew anything here would be drawing subtitles the user did not ask for.
  v.currentTime = 1; v.currentTime = 3; v.currentTime = 5;
  mark('off');

  // 2. Turning captions ON, with cue C already under the playhead. There is no later moment for the
  //    renderer to learn this.
  t.mode = 'showing';
  mark('on');

  // 3. Repeated writes WITHIN the same cue. A player does this every frame; nothing changed.
  v.currentTime = 5.1; v.currentTime = 5.2; v.currentTime = 5.9;
  mark('same');

  // 4. A seek from one single-cue line straight to another — both active sets are length 1. This is
  //    the assertion a length comparison gets wrong.
  v.currentTime = 1.0;
  mark('seek');

  // 5. A cue appended over the CURRENT time, the live-stream case.
  t.addCue(new VTTCue(0.5, 1.5, 'LIVE'));
  mark('live');

  // 6. Captions turned back off: the set goes non-empty -> empty, and this fire is what CLEARS the
  //    caption node.
  t.mode = 'disabled';
  mark('offagain');

  document.getElementById('log').textContent = out.join(' ');
</script></body>"#,
        "https://cuechange.test/",
        &fonts,
        W,
    );

    let root = p.dom().root();
    let lg = manuk_css::query_selector_all(p.dom(), root, "#log")[0];
    let got = p.dom().text_content(lg);
    assert!(
        !got.is_empty(),
        "the script never reached the end — the caption path threw"
    );

    // 1. `off`  — three boundary crossings on a disabled track: ZERO fires, nothing drawn.
    // 2. `on`   — +1, and it draws C, the cue already under the playhead at t=5.
    // 3. `same` — still 1. Three more clock writes inside C changed nothing.
    // 4. `seek` — +1 -> 2, and it draws A. Length-only comparison scores this as no-change.
    // 5. `live` — +1 -> 3, LIVE lands over t=1.0 alongside A, in start order.
    // 6. `offagain` — +1 -> 4, and the active set is empty, which is the signal to CLEAR.
    let want = "off=0:- on=1:C same=1:C seek=2:A live=3:A+LIVE offagain=4:-";
    assert_eq!(got.trim(), want, "G_CUE_CHANGE");
}
