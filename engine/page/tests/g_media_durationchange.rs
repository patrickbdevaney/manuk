//! **G_MEDIA_DURATIONCHANGE — the element fires `durationchange` when the timeline length is known.**
//!
//! A MediaSource-fed `<video>` learns its duration either from an explicit `mediaSource.duration = N`
//! (the API a live/DVR player sets) or from a demuxed `moov` (VOD). Either way the element's
//! `duration` getter started reflecting the new value — but silently: no `durationchange` fired, so a
//! player bound to that event (to size its scrub bar, to compute "% watched", to enable seeking once
//! the length is known) never woke up. The scrub bar stayed zero-width over a video whose length the
//! element already knew.
//!
//! Asserted through the public surface: setting `mediaSource.duration` fires `durationchange` on the
//! element and moves `video.duration`; setting it to the SAME value fires nothing (no event on a
//! no-op); setting it to a new value fires again.
//!
//! **RED, run:** drop the `__fireDurationChange()` call from the `duration` setter in `mse_js.rs` —
//! `dc:1` becomes `dc:0` and the element never hears the length it now reports.

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

    var dc = 0;
    v.addEventListener('durationchange', function () { dc++; });

    ms.addEventListener('sourceopen', function () {
     try {
      R.push('nan:' + (v.duration !== v.duration));   // NaN before any length is known
      ms.duration = 12;
      R.push('set:' + v.duration + ',dc:' + dc);      // 12, dc:1
      ms.duration = 12;                                // same value: no event
      R.push('same:dc:' + dc);                         // dc:1
      ms.duration = 15;                                // new value: fires again
      R.push('again:' + v.duration + ',dc:' + dc);     // 15, dc:2
      R.push('done:true');
     } catch (e) { R.push('threw:' + (e && e.name ? e.name : e)); }
    });
  </script>
</body></html>"##;

#[test]
fn setting_media_source_duration_fires_durationchange_on_the_element() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://watch.test/", &fonts, 800.0);

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("MEDIA DURATIONCHANGE PROBE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_MEDIA_DURATIONCHANGE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "nan:true",
        "before any length is known the duration is NaN — the honest starting state a player \
         feature-tests before showing a scrub bar",
    ),
    (
        "set:12,dc:1",
        "setting mediaSource.duration moves video.duration AND fires durationchange exactly once — \
         the event a player binds to size its timeline",
    ),
    (
        "same:dc:1",
        "setting the duration to the value it already has fires nothing; durationchange announces a \
         CHANGE, not every write",
    ),
    (
        "again:15,dc:2",
        "a new duration fires again — a live/DVR stream whose window grows must re-announce",
    ),
    (
        "done:true",
        "the whole sequence ran inside the sourceopen handler; a throw or hang drops this token",
    ),
];
