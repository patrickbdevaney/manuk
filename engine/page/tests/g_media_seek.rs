//! **G_MEDIA_SEEK — writing `currentTime` is a real seek: `seeking`/`seeked` fire, the position
//! clamps, and the new position crosses to the host.**
//!
//! Tick 521 gave `<video>` a running clock; the clock could only move FORWARD, driven by the host.
//! A `video.currentTime = 30` — the write behind every scrub-bar drag, chapter jump and
//! "resume where you left off" — stored the number, synced captions, and told no one: no `seeking`,
//! no `seeked`, no reposition of the decoder, and no clamp, so a scrub to 9999 on a 30s clip left
//! the clock in a place the media has no frames for. A player bound to `seeked` (to hide its
//! buffering spinner, to fetch the segment around the new point) waited forever.
//!
//! What is asserted, all through the public surface a player touches:
//!   * `seekable` reports the `[0, duration]` span a scrub bar jumps within.
//!   * a `currentTime` write to a NEW position fires `seeking` then `seeked` and moves the clock.
//!   * a write to the SAME position is not a seek (no event storm over a still clock).
//!   * the position CLAMPS: past the end → the end, negative → 0.
//!   * `fastSeek(t)` seeks (approximate-seek shares the path; the difference is decoder-internal).
//!   * a backward seek after `ended` clears `ended` — the element is playable again from there.
//!   * **the new position crosses to the HOST** on the same live-write channel as volume/rate:
//!     asserted Rust-side via `take_media_props`, so the seek is not a JS-only illusion.
//!
//! **RED, run:** drop the `seeking`/`seeked` dispatch from the `currentTime` setter and
//! `seek:5,5` / `endclear:true,false,4` flip; remove the clamp and `clampHi:10.00` becomes
//! `100.00`; remove `"currentTime"` from the `media_prop` allow-list in `dom_bindings.rs` and the
//! host-seam assertion fails.

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
      ms.duration = 10;
      R.push('dur:' + v.duration);

      var sk = v.seekable;
      R.push('seekable:' + sk.length + ',' + sk.start(0) + ',' + sk.end(0));

      var seeking = 0, seeked = 0;
      v.addEventListener('seeking', function () { seeking++; });
      v.addEventListener('seeked',  function () { seeked++; });

      v.currentTime = 6;                 // (1) real seek from 0
      R.push('ct6:' + v.currentTime);
      v.currentTime = 6;                 // same position: NOT a seek
      R.push('noop:' + seeking);         // still 1

      v.currentTime = 100;               // (2) clamp to the end
      R.push('clampHi:' + v.currentTime.toFixed(2));
      v.currentTime = -5;                // (3) clamp to 0
      R.push('clampLo:' + v.currentTime.toFixed(2));
      v.fastSeek(3);                     // (4) approximate seek shares the path
      R.push('fast:' + v.currentTime.toFixed(2));
      R.push('seek:' + seeking + ',' + seeked);

      // A backward seek after `ended` makes the element playable again.
      var id = v.__nodeId;
      v.play();
      globalThis.__mediaAdvance(id, 12); // run past the 10s end -> ended
      var endedBefore = v.ended;
      v.currentTime = 4;                 // (5) backseek clears ended
      R.push('endclear:' + endedBefore + ',' + v.ended + ',' + v.currentTime.toFixed(2));
      R.push('seekf:' + seeking + ',' + seeked);

      R.push('done:true');
     } catch (e) { R.push('threw:' + (e && e.name ? e.name : e)); }
    });
  </script>
</body></html>"##;

#[test]
fn writing_current_time_seeks_fires_events_clamps_and_reaches_the_host() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://watch.test/", &fonts, 800.0);

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("MEDIA SEEK PROBE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_MEDIA_SEEK: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }

    // The host seam: every seek pushes `currentTime` onto the live-write channel, exactly as a
    // volume/rate change does. If it never arrives, the decoder can never be repositioned and the
    // seek is a JS-only illusion. The final backseek to 4 is the last write, so it must be present.
    let props = page.take_media_props();
    println!("MEDIA SEEK HOST PROPS: {props:?}");
    let seeked_to: Vec<f64> = props
        .iter()
        .filter(|(_, name, _)| name == "currentTime")
        .map(|(_, _, v)| *v)
        .collect();
    assert!(
        !seeked_to.is_empty(),
        "G_MEDIA_SEEK: no `currentTime` reached the host via take_media_props — the seek never \
         crossed the live-write channel; got props: {props:?}"
    );
    assert!(
        seeked_to.iter().any(|v| (v - 4.0).abs() < 0.001),
        "G_MEDIA_SEEK: the final backseek to 4.0 did not reach the host; currentTime props were \
         {seeked_to:?}"
    );
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "dur:10",
        "the MSE handshake must give a finite duration, or clamp/seekable below are vacuous",
    ),
    (
        "seekable:1,0,10",
        "a fully-known video reports one seekable range spanning [0, duration] — the extent a \
         scrub bar lets the user jump within",
    ),
    (
        "ct6:6",
        "writing currentTime moves the clock to the written position",
    ),
    (
        "noop:1",
        "a write to the SAME position is not a seek — players re-assign currentTime every frame, \
         and firing seeking/seeked on each would be an event storm over a still clock",
    ),
    (
        "clampHi:10.00",
        "seeking past the end clamps to the duration; a scrub to 9999 must not leave the clock \
         where the media has no frames",
    ),
    ("clampLo:0.00", "seeking to a negative time clamps to 0"),
    (
        "fast:3.00",
        "fastSeek(t) performs the seek (approximate-seek shares the path — the difference is a \
         decoder optimisation, not JS-visible); it must not throw",
    ),
    (
        "seek:4,4",
        "four distinct seeks fired seeking->seeked exactly once each; the same-position write in \
         between fired nothing",
    ),
    (
        "endclear:true,false,4.00",
        "a backward seek after `ended` clears `ended` and repositions — the element is playable \
         again from the new point, which is how a viewer scrubs back from the end to rewatch",
    ),
    (
        "seekf:5,5",
        "the backseek was the fifth seek; seeking and seeked stay paired",
    ),
    (
        "done:true",
        "the whole sequence ran inside the sourceopen handler; a throw or hang drops this token",
    ),
];
