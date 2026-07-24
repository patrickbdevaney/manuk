//! **G_MEDIA_PLAYED — `played` is the union of the spans actually watched.**
//!
//! Ticks 521-522 gave `<video>` a running clock and a real seek. `played` — distinct from
//! `buffered` (what was fetched) and `seekable` (what may be jumped to) — is the union of the time
//! spans the clock has ACTUALLY advanced through. It was a frozen empty TimeRanges. That zero is the
//! ground truth behind watch-progress analytics ("you've watched 80%"), the "continue watching"
//! resume marker, and per-segment engagement heatmaps: a `played` that never grows is a progress bar
//! that never fills and a resume point stuck at 0:00.
//!
//! What is asserted, through the public TimeRanges surface:
//!   * playing forward grows the current span, and an adjacent/overlapping span MERGES (playing
//!     0→5 is one range, not five one-second ranges).
//!   * seeking across a gap and playing there creates a SECOND range — `played` is a union, not an
//!     envelope, so a viewer who skipped the middle leaves a hole.
//!   * seeking back into a gap and playing MERGES the two spans back down.
//!
//! **RED, run:** drop the `__addPlayed` calls from `__advance` and every `pl*` claim collapses to
//! the empty `length 0`; break the merge (always `push` a new range) and `merge:2,0.00,6.00` shows
//! more than two ranges.

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
    function ranges(tr) {
      var s = tr.length + ':';
      for (var i = 0; i < tr.length; i++) {
        s += '[' + tr.start(i).toFixed(2) + ',' + tr.end(i).toFixed(2) + ']';
      }
      return s;
    }

    var v = document.getElementById('v');
    var ms = new MediaSource();
    v.src = URL.createObjectURL(ms);

    ms.addEventListener('sourceopen', function () {
     try {
      ms.duration = 20;
      var id = v.__nodeId;
      R.push('empty:' + v.played.length);   // nothing watched yet

      v.play();
      globalThis.__mediaAdvance(id, 3);      // play 0 -> 3
      R.push('p1:' + ranges(v.played));      // 1:[0.00,3.00]
      globalThis.__mediaAdvance(id, 2);      // play 3 -> 5, MERGES with [0,3]
      R.push('p2:' + ranges(v.played));      // 1:[0.00,5.00]

      v.currentTime = 8;                     // skip the middle (a seek does not play)
      globalThis.__mediaAdvance(id, 1);      // play 8 -> 9, a SECOND range
      R.push('gap:' + ranges(v.played));     // 2:[0.00,5.00][8.00,9.00]

      v.currentTime = 3;                     // seek back into the first span
      globalThis.__mediaAdvance(id, 3);      // play 3 -> 6, MERGES with [0,5] -> [0,6]
      R.push('merge:' + ranges(v.played));   // 2:[0.00,6.00][8.00,9.00]

      R.push('done:true');
     } catch (e) { R.push('threw:' + (e && e.name ? e.name : e)); }
    });
  </script>
</body></html>"##;

#[test]
fn played_accumulates_watched_spans_as_a_merged_union() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://watch.test/", &fonts, 800.0);

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("MEDIA PLAYED PROBE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_MEDIA_PLAYED: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "empty:0",
        "a video that has not played reports no played ranges — the honest starting state",
    ),
    (
        "p1:1:[0.00,3.00]",
        "playing 0->3 records exactly that span as one played range",
    ),
    (
        "p2:1:[0.00,5.00]",
        "playing on to 5 EXTENDS the same range (adjacency merges) — not five one-second ranges",
    ),
    (
        "gap:2:[0.00,5.00][8.00,9.00]",
        "seeking past the middle and playing there leaves a hole: `played` is a union of watched \
         spans, not an envelope — this is what makes an engagement heatmap show the skipped section",
    ),
    (
        "merge:2:[0.00,6.00][8.00,9.00]",
        "seeking back into the first span and playing merges it down — [0,5] + [3,6] = [0,6], while \
         the disjoint [8,9] stays separate",
    ),
    (
        "done:true",
        "the whole sequence ran inside the sourceopen handler; a throw or hang drops this token",
    ),
];
