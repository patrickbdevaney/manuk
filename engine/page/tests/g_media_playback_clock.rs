//! **G_MEDIA_PLAYBACK_CLOCK — `<video>` has a running clock: `timeupdate`/`ended` fire and
//! `currentTime` advances.**
//!
//! The media element surface was mature — `play()` resolved, `paused` flipped, `canPlayType`
//! answered honestly, captions rendered — and the *clock was dead*. `play()` moved `paused` and
//! nothing else: `currentTime` sat at 0, so `timeupdate` never fired and `ended` never arrived. A
//! `<video autoplay>` looked like it was playing and reported a frozen timeline.
//!
//! That is not cosmetic. `timeupdate` is the single most-bound media event — progress bars, the
//! "% watched" analytics beacon every video site sends, synchronized transcripts, ad-insertion
//! cue points and chapter markers ALL listen on it. `ended` is how a playlist advances to the next
//! track, how autoplay-next fires, how a non-`loop` clip stops. A player bound to a clock that
//! never ticks shows a scrubber pinned at 0:00 over a video that (to the host's decoder) is
//! playing, and a queue that never reaches song two.
//!
//! **The clock is host-driven** (see `__mediaAdvance` in `engine/js/src/event_loop.rs`): the shell
//! owns the real audio/wall clock and a bounded render budget, and calls `__mediaAdvance(nodeId,
//! elapsedSeconds)` each frame. A self-pumping `setTimeout` would spin forever on the commonest web
//! video of all, the muted `autoplay loop` background clip, whose `loop` has no natural stop. In
//! this headless gate the test IS the host — it drives that exact entry point, so what is proven is
//! the production path, not a test-only shim.
//!
//! What is asserted, all through the public surface a player touches:
//!   * `play()` fires `play` then `playing` on the paused→playing edge, and flips `paused`.
//!   * driving the clock past known moments advances `currentTime` and fires `timeupdate` each step.
//!   * reaching `duration` clamps `currentTime`, fires a final `timeupdate` then `ended`, and pauses
//!     — WITHOUT firing `pause` (the spec routes end-of-media to `ended`, not `pause`).
//!   * `playbackRate` fires `ratechange` and genuinely scales the clock (2× rate ⇒ 2× advance).
//!   * `play()` after `ended` restarts at 0 and clears `ended` (replay).
//!
//! **RED, run:** make `el.__advance` return immediately (a no-op) — `currentTime` stays 0,
//! `timeupdate`/`ended` never fire, and `t:9.00 / ended:true,1 / tu:true` all flip. Or drop the
//! `play`/`playing` dispatch from `el.play` and `afterplay:false,1,1` fails.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <video id="v"></video>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } },
    };

    var v = document.getElementById('v');
    var ms = new MediaSource();
    v.src = URL.createObjectURL(ms);

    ms.addEventListener('sourceopen', function () {
     try {
      // A known, finite duration is what turns "advance forever" into "ends at 9". MSE is the
      // real path that gives a media element a duration, so the clock is exercised the way a
      // streaming player would give it one.
      ms.duration = 9;
      R.push('dur:' + v.duration);
      R.push('paused0:' + v.paused);

      var log = { play: 0, playing: 0, pause: 0, ended: 0, tu: 0, rate: 0 };
      v.addEventListener('play',       function () { log.play++; });
      v.addEventListener('playing',    function () { log.playing++; });
      v.addEventListener('pause',      function () { log.pause++; });
      v.addEventListener('ended',      function () { log.ended++; });
      v.addEventListener('timeupdate', function () { log.tu++; });
      v.addEventListener('ratechange', function () { log.rate++; });

      v.play();
      R.push('afterplay:' + v.paused + ',' + log.play + ',' + log.playing);

      // Drive the clock through the exact host entry point, by node id.
      var id = v.__nodeId;
      globalThis.__mediaAdvance(id, 4);   // t -> 4
      R.push('t4:' + v.currentTime);
      globalThis.__mediaAdvance(id, 4);   // t -> 8
      globalThis.__mediaAdvance(id, 4);   // t -> 12, clamps to 9 and ends

      R.push('t:' + v.currentTime.toFixed(2));
      R.push('ended:' + v.ended + ',' + log.ended);
      R.push('pausedEnd:' + v.paused);
      R.push('nopause:' + (log.pause === 0));      // end-of-media fires `ended`, never `pause`
      R.push('tu:' + (log.tu >= 3));               // 4, 8, and the final tick at 9

      globalThis.__mediaAdvance(id, 4);            // past the end: a no-op
      R.push('afterEnd:' + v.currentTime.toFixed(2));

      // Replay restarts at 0 and clears `ended`.
      v.play();
      R.push('replay:' + v.currentTime.toFixed(2) + ',' + v.ended);

      // Rate scales the clock, and fires ratechange.
      v.playbackRate = 2;
      globalThis.__mediaAdvance(id, 2);            // 2s * 2x = 4s
      R.push('rate:' + log.rate + ',' + v.currentTime.toFixed(2));

      R.push('done:true');
     } catch (e) { R.push('threw:' + (e && e.name ? e.name : e)); }
    });
  </script>
</body></html>"##;

#[test]
fn a_video_element_has_a_running_clock_that_fires_timeupdate_and_ended() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://watch.test/", &fonts, 800.0);

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("MEDIA PLAYBACK CLOCK PROBE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_MEDIA_PLAYBACK_CLOCK: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "dur:9",
        "the MSE handshake must give the element a finite duration, or every clamp/end assertion \
         below is vacuous",
    ),
    (
        "paused0:true",
        "a fresh element is paused; the play->playing edge that follows depends on this being the \
         starting state",
    ),
    (
        "afterplay:false,1,1",
        "play() flips `paused` to false and fires `play` then `playing` exactly once each on the \
         paused->playing edge; a player reads `paused` back to paint its button and binds `playing` \
         to hide its spinner",
    ),
    (
        "t4:4",
        "one host frame of 4s must move `currentTime` to 4 — the clock is dead if it does not",
    ),
    (
        "t:9.00",
        "advancing past `duration` clamps `currentTime` to exactly the duration, never overshoots",
    ),
    (
        "ended:true,1",
        "reaching the end sets `ended` true and fires the `ended` event once — this is how a \
         playlist advances and autoplay-next fires; a frozen `false` is a queue stuck on track one",
    ),
    (
        "pausedEnd:true",
        "a non-loop clip that reaches its end is paused",
    ),
    (
        "nopause:true",
        "end-of-media fires `ended`, NOT `pause` — a player that treats a `pause` at the end as a \
         user action would suppress its own autoplay-next",
    ),
    (
        "tu:true",
        "`timeupdate` fires on each advance (at 4, 8, and the final tick at 9) — the single event \
         progress bars, transcripts and %-watched analytics all bind",
    ),
    (
        "afterEnd:9.00",
        "advancing an already-ended element is a no-op; the host must be able to keep calling \
         __mediaAdvance on a stale element without moving a stopped clock",
    ),
    (
        "replay:0.00,false",
        "play() after `ended` restarts at 0 and clears `ended` (HTML: seek to 0 if playback has \
         ended)",
    ),
    (
        "rate:1,4.00",
        "setting `playbackRate` fires `ratechange` once, and the clock genuinely scales — 2s of \
         host time at 2x is 4s of media time",
    ),
    (
        "done:true",
        "the whole sequence ran to the end inside the sourceopen handler; a throw or a hang would \
         drop this token",
    ),
];
