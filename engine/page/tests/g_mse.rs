//! **G_MSE — a player can construct a MediaSource, attach it to a `<video>`, and stream bytes into it.**
//!
//! **The failure this gate exists for.** `MediaSource` did not exist. Not a stub, not an inert
//! constructor — the name was absent, so `new MediaSource()` was a `ReferenceError`. Every adaptive
//! player (hls.js, dash.js, shaka, video.js, and YouTube's own) runs that construction inside a
//! capability probe at *module-evaluation* time, so the throw did not degrade playback; it killed
//! the player script before it rendered a control, and took the rest of the bundle's evaluation with
//! it. Adaptive streaming is not `<video src="file.mp4">` — the element's `src` is a `blob:` URL and
//! every byte arrives through `appendBuffer`, so with no MediaSource the entire watch-the-web class
//! is not "degraded", it is absent.
//!
//! **What is asserted here is the pipe, not playback.** There is no decoder yet, and the gate holds
//! the implementation to that honesty rather than papering over it:
//!
//!   * `isTypeSupported` answers from `__mseCodecs`, the registry of what can genuinely be decoded.
//!     It is empty in the shipped engine, so the gate asserts VP9/AAC is `false` **first** — a
//!     player must be told no and take its fallback, never told yes and then hang on a `buffered`
//!     range that can never grow. The gate then registers a codec (exactly as M3/M4/M5 will) and
//!     asserts the answer flips, which is what proves the honesty is a *seam* and not a hardcoded
//!     `false`.
//!   * `buffered` stays empty even after bytes are appended, because nothing has been demuxed.
//!
//! **The sequence asserted is the real append loop**: `sourceopen` → `addSourceBuffer` →
//! `appendBuffer` → `updatestart` → `update` → `updateend` → `endOfStream` → `sourceended`, with
//! `updating` true across the append and the re-entrant `appendBuffer` correctly refused. That is
//! byte-for-byte the control flow a player executes, and it must survive unchanged when a real
//! demuxer takes over the middle of it.
//!
//! **Both RED directions were run, not assumed.** Removing the `mse.js` eval reproduces the
//! original engine exactly — `THREW:ReferenceError: MediaSource is not defined`, the whole script
//! dead at its first line, which is the failure mode described above. Removing only the
//! `__mseAttach` call from the `src` setter — a plausible half-implementation, where every object
//! exists and only the handshake is missing — passes the first seven claims and fails at
//! `syncopen:false`: the source never leaves `closed`, so `sourceopen` never fires and nothing
//! after it runs. That is precisely the silent hang this gate exists to prevent, and it is caught
//! at the exact line where the wiring is missing.

use manuk_text::FontContext;

/// A codec string of the shape YouTube actually requests.
const HTML: &str = r##"<!doctype html>
<html><body>
  <video id="v"></video>
  <div id="out">-</div>
  <script>
    var $ = function (id) { return document.getElementById(id); };

    // The record flushes to the DOM on EVERY push, not once at the end.
    //
    // This is a diagnostic property, and it is the difference between a gate that tells you what
    // broke and one that does not. The sequence asserted here is asynchronous and only completes on
    // `sourceended`; a single end-of-run write means ANY earlier break — the source never opening,
    // an append that throws, a listener that never fires — reports the identical empty `-`, which
    // names no claim and points at no mechanism. Flushing per push makes the last recorded claim the
    // failure's location.
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = $('out'); if (o) { o.textContent = this.a.join(' '); } },
      join: function (sep) { return this.a.join(sep); }
    };
    var TYPE = 'video/webm; codecs="vp9"';

    try {
      // ── Interfaces. A player touches all four by name.
      R.push('ctors:' + (typeof MediaSource === 'function' && typeof SourceBuffer === 'function' &&
                         typeof SourceBufferList === 'function' && typeof TimeRanges === 'function'));

      // ── The honest NO. Nothing can decode VP9 yet, so nothing may claim it.
      R.push('unsupported:' + (MediaSource.isTypeSupported(TYPE) === false));

      var ms = new MediaSource();
      R.push('closed:' + (ms.readyState === 'closed'));
      R.push('isms:' + (ms instanceof MediaSource));
      R.push('durnan:' + (ms.duration !== ms.duration));

      // A closed source refuses buffers — and refuses them with InvalidStateError, which is the
      // error a player branches on to decide it attached too early.
      try { ms.addSourceBuffer(TYPE); R.push('closedadd:NOTHROW'); }
      catch (e) { R.push('closedadd:' + (e.name === 'NotSupportedError' || e.name === 'InvalidStateError')); }

      // ── The attachment handshake.
      var url = URL.createObjectURL(ms);
      R.push('bloburl:' + (typeof url === 'string' && url.indexOf('blob:') === 0));

      var v = $('v');
      ms.addEventListener('sourceopen', function () {
        try {
          R.push('sourceopen:true');
          R.push('open:' + (ms.readyState === 'open'));
          R.push('elnet:' + (v.networkState === 2));   // NETWORK_LOADING — it really is being fed

          // ── The seam. This is the single line M3/M4/M5 replace with a real decoder registration.
          __mseCodecs.push(TYPE);
          R.push('supported:' + (MediaSource.isTypeSupported(TYPE) === true));

          var sb = ms.addSourceBuffer(TYPE);
          R.push('sb:' + (sb instanceof SourceBuffer));
          R.push('list:' + (ms.sourceBuffers.length === 1 && ms.sourceBuffers[0] === sb));
          R.push('idle:' + (sb.updating === false));

          var seq = [];
          ['updatestart', 'update', 'updateend'].forEach(function (t) {
            sb.addEventListener(t, function () { seq.push(t); });
          });

          // ── The append. Real bytes, held in order.
          sb.appendBuffer(new Uint8Array([0x1a, 0x45, 0xdf, 0xa3, 0x9f, 0x42, 0x86]).buffer);
          R.push('updating:' + (sb.updating === true));
          R.push('queued:' + (sb.__chunks.length === 1 && sb.__bytes === 7));

          // Appending again mid-update is the single most common player bug, and the spec makes it
          // an InvalidStateError precisely so the player can queue instead.
          try { sb.appendBuffer(new Uint8Array([1]).buffer); R.push('reentrant:NOTHROW'); }
          catch (e2) { R.push('reentrant:' + (e2.name === 'InvalidStateError')); }

          // ...and so is calling endOfStream() while a buffer is still updating.
          try { ms.endOfStream(); R.push('eosbusy:NOTHROW'); }
          catch (e3) { R.push('eosbusy:' + (e3.name === 'InvalidStateError')); }

          // ONE `updateend` listener, dispatching on a step counter — deliberately NOT a listener
          // that registers another listener. A handler that appends is re-entered by the very
          // append it just made, so a per-step listener is an unbounded append/timer chain that
          // never lets the event loop drain: the page hangs rather than fails. That is the same
          // runaway a real player's `updateend`-driven fill loop is, which is why the step machine
          // below (and not nesting) is the shape this must be asserted in.
          var step = 0;
          sb.addEventListener('updateend', function () {
            step++;
            try {
              if (step === 1) {
                R.push('order:' + (seq.join(',') === 'updatestart,update,updateend'));
                R.push('settled:' + (sb.updating === false));

                // A second segment appends onto the same buffer — the loop's steady state.
                sb.appendBuffer(new Uint8Array([9, 9, 9]).buffer);
                R.push('grew:' + (sb.__chunks.length === 2 && sb.__bytes === 10));

                // Honest: bytes are held, nothing is demuxed, so there is no media timeline.
                R.push('nobuffered:' + (sb.buffered.length === 0 && v.buffered.length === 0));
              } else if (step === 2) {
                // duration is live through the element, which is what a progress bar reads.
                ms.duration = 12.5;
                R.push('duration:' + (ms.duration === 12.5 && v.duration === 12.5));

                ms.endOfStream();
                R.push('ended:' + (ms.readyState === 'ended'));
              }
            } catch (e4) { R.push('THREW-step' + step + ':' + e4); $('out').textContent = R.join(' '); }
          });
        } catch (e1) { R.push('THREW-open:' + e1); $('out').textContent = R.join(' '); }
      });

      ms.addEventListener('sourceended', function () {
        R.push('sourceended:true');
        $('out').textContent = R.join(' ');
      });

      // Revoking right after assignment is what every player does — it must NOT tear the stream down.
      v.src = url;
      URL.revokeObjectURL(url);
      R.push('syncopen:' + (ms.readyState === 'open'));
    } catch (e) {
      $('out').textContent = 'THREW:' + e;
    }
  </script>
</body></html>"##;

/// One test in the binary — a second `Page::load` stands up a second SpiderMonkey context in the
/// same process and does not survive teardown (see `g_overflow_cssom`, `g_globals`).
#[test]
fn a_media_source_attaches_to_a_video_and_accepts_appended_segments() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://watch.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("ctors:true", "a player names MediaSource, SourceBuffer, SourceBufferList and TimeRanges directly; a missing one is a ReferenceError inside its capability probe"),
        ("unsupported:true", "nothing can decode VP9 yet, and claiming otherwise steers the player onto a path that hangs instead of onto its fallback"),
        ("closed:true", "a fresh MediaSource is 'closed' until an element attaches it"),
        ("isms:true", "instanceof must work — players type-check the object they were handed"),
        ("durnan:true", "duration is NaN while closed"),
        ("closedadd:true", "addSourceBuffer before attachment must throw, and throw the error the player branches on"),
        ("bloburl:true", "URL.createObjectURL(ms) is the only channel by which the element learns its source"),
        ("syncopen:true", "assigning the object URL to video.src must open the source immediately, not on some later turn"),
        ("sourceopen:true", "sourceopen is the event every player waits for before it will append anything; if it never fires the player waits forever with nothing in the DOM to see"),
        ("open:true", "readyState must read 'open' inside the handler"),
        ("elnet:true", "the element reports NETWORK_LOADING once it has a source — it genuinely is being fed"),
        ("supported:true", "the honest 'no' must be a registry seam, not a hardcoded false: registering a decoder flips the answer"),
        ("sb:true", "addSourceBuffer returns a real SourceBuffer"),
        ("list:true", "ms.sourceBuffers[0] is indexable — players write exactly that"),
        ("idle:true", "a fresh buffer is not updating"),
        ("updating:true", "updating must be true across an append, or a player's guard lets it append twice"),
        ("queued:true", "the bytes must actually be held — this is the queue the demuxer will read"),
        ("reentrant:true", "appending mid-update is an InvalidStateError so the player queues instead of corrupting the stream"),
        ("eosbusy:true", "endOfStream while a buffer updates is an InvalidStateError"),
        ("order:true", "updatestart -> update -> updateend, in that order, is the loop's clock"),
        ("settled:true", "updating must be false by the time updateend runs, or the loop's next append throws"),
        ("grew:true", "the second segment appends onto the same buffer — the steady state of every player"),
        ("nobuffered:true", "nothing is demuxed, so buffered is honestly empty rather than fabricated"),
        ("duration:true", "duration set on the source must read back through the element — that is what a progress bar reads"),
        ("ended:true", "endOfStream moves the source to 'ended'"),
        ("sourceended:true", "sourceended must fire, or a player never learns the stream finished"),
    ] {
        assert!(
            got.contains(claim),
            "G_MSE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
