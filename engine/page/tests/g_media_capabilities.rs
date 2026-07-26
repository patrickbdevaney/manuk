//! # G_MEDIA_CAPABILITIES — `decodingInfo()` answers, and it answers the SAME thing as its two siblings
//!
//! **The failure this gate exists for.** `navigator.mediaCapabilities` was `undefined`, so
//! `navigator.mediaCapabilities.decodingInfo({…})` threw a **TypeError**. Shaka, dash.js, hls.js
//! and YouTube's own player all call it on boot, once per candidate rendition — a throw there
//! happens *while enumerating renditions*, so the player never gets to render any of them. That is
//! the throw-class that blanks a page, not a missing nicety.
//!
//! ## The assertion that matters most is not "it exists"
//!
//! Three spec surfaces now answer *"can this tree decode this contentType"*:
//! `MediaSource.isTypeSupported`, `HTMLMediaElement.canPlayType`, and `decodingInfo().supported`.
//! t634 consolidated the first two after their WebM answers had silently drifted apart. **A third
//! implementation would have restored that defect at full size, one tick after paying to remove
//! it** — and a third implementation is indistinguishable from a shared one on any test that only
//! checks each answer against a hardcoded expectation.
//!
//! So this gate asserts **agreement**, computed at runtime: for each of six contentTypes it
//! compares `decodingInfo().supported` against `isTypeSupported()` **in the page**, and reports a
//! single `agree:` claim. A second implementation that happens to be correct today passes it; a
//! second implementation that has drifted does not, and the drift is what actually happens.
//! `mixed:` then proves the six strings are not all the same answer — agreement across six `false`s
//! would be vacuous.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! | `decodingInfo` grows its own `/mp4/` test instead of calling `__manukCanDecodeType` | RED on `agree:` **and nothing else** — `sup:true`, `pe:false`, `mixedcfg:false` and `webrtc:false` all stay green. This is the probe that justifies the gate's design: a second implementation is invisible to every per-answer assertion. |
//! | delete the `navigator.mediaCapabilities` install | RED — and the record stops DEAD at `mc:undefined`, because the next line throws. **The throw-class this gate is named for, reproduced.** |
//! | `powerEfficient: true` | RED on `pe:` alone |
//!
//! The `badtype:`/`noparts:` validation claims have **not** had a mutation run against them; they
//! are asserted, not probed, and saying so is cheaper than the alternative — t633 shipped a gate
//! whose doc claimed a coverage its RED probe then denied.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <video id="v"></video>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } },
    };

    R.push('mc:' + typeof navigator.mediaCapabilities);
    R.push('di:' + typeof navigator.mediaCapabilities.decodingInfo);
    R.push('ei:' + typeof navigator.mediaCapabilities.encodingInfo);

    // ── The six strings, spanning both containers and both sides of every codec line this tree
    //    draws. `agree` is computed against isTypeSupported IN THE PAGE, so it compares the two
    //    implementations rather than comparing each to a number I wrote down.
    var TYPES = [
      'video/mp4; codecs="avc1.42E01E"',          // Baseline H.264 — decodes
      'video/mp4; codecs="avc1.640028"',          // High profile   — does not
      'video/mp4; codecs="av01.0.00M.08"',        // AV1 in MP4     — decodes
      'video/webm; codecs="av01.0.01M.08"',       // AV1 in WebM    — decodes (t634)
      'video/webm; codecs="vp9"',                 // VP9            — does not
      'audio/mp4; codecs="mp4a.40.2"'             // AAC            — decodes
    ];

    var agree = true, yes = 0, no = 0, pending = TYPES.length;
    TYPES.forEach(function (ct) {
      var its = MediaSource.isTypeSupported(ct);
      if (its) { yes++; } else { no++; }
      navigator.mediaCapabilities
        .decodingInfo({ type: 'media-source',
                        video: { contentType: ct, width: 640, height: 360,
                                 bitrate: 800000, framerate: 24 } })
        .then(function (info) {
          if (info.supported !== its) { agree = false; }
          if (--pending === 0) {
            R.push('agree:' + agree);
            // Not all six the same answer — otherwise `agree` is a comparison of two constants.
            R.push('mixed:' + (yes > 0 && no > 0));
          }
        })
        .catch(function () { agree = false; if (--pending === 0) { R.push('agree:false'); } });
    });

    // ── The shape of a resolved answer, on a rendition we genuinely decode.
    navigator.mediaCapabilities
      .decodingInfo({ type: 'file',
                      video: { contentType: 'video/mp4; codecs="avc1.42E01E"',
                               width: 640, height: 360, bitrate: 800000, framerate: 24 } })
      .then(function (info) {
        R.push('sup:' + info.supported);
        R.push('smooth:' + info.smooth);
        R.push('pe:' + info.powerEfficient);
        R.push('echo:' + (info.configuration && info.configuration.type === 'file'));
      });

    // ── AUDIO-only, and a MIXED config where the audio half is the undecodable one: the answer
    //    must be false for the WHOLE config, or a player pairs a stream it can show with one it
    //    cannot hear and calls that supported.
    navigator.mediaCapabilities
      .decodingInfo({ type: 'media-source',
                      video: { contentType: 'video/webm; codecs="av01.0.01M.08"',
                               width: 480, height: 360, bitrate: 500000, framerate: 30 },
                      audio: { contentType: 'audio/webm; codecs="opus"' } })
      .then(function (info) { R.push('mixedcfg:' + info.supported); });

    // ── WebRTC is a decided non-goal, and an honest no is not the same as a throw.
    navigator.mediaCapabilities
      .decodingInfo({ type: 'webrtc',
                      video: { contentType: 'video/mp4; codecs="avc1.42E01E"',
                               width: 640, height: 360, bitrate: 800000, framerate: 24 } })
      .then(function (info) { R.push('webrtc:' + info.supported); });

    // ── Validation REJECTS; it does not resolve `supported:false`. A player must be able to tell
    //    "you told me no" from "you did not understand the question".
    navigator.mediaCapabilities
      .decodingInfo({ type: 'bogus', video: { contentType: 'video/mp4' } })
      .then(function () { R.push('badtype:resolved'); })
      .catch(function (e) { R.push('badtype:' + (e && e.name ? e.name : 'threw')); });

    navigator.mediaCapabilities
      .decodingInfo({ type: 'file' })
      .then(function () { R.push('noparts:resolved'); })
      .catch(function (e) { R.push('noparts:' + (e && e.name ? e.name : 'threw')); });

    // ── encodingInfo exists and truthfully says no; nothing here encodes.
    navigator.mediaCapabilities
      .encodingInfo({ type: 'record', video: { contentType: 'video/mp4; codecs="avc1.42E01E"',
                                               width: 640, height: 360, bitrate: 800000,
                                               framerate: 24 } })
      .then(function (info) { R.push('enc:' + info.supported); });
  </script>
</body></html>"##;

#[test]
fn decoding_info_answers_and_agrees_with_its_siblings() {
    let tmp = std::env::temp_dir().join("manuk-g-media-caps");
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };

    let fonts = FontContext::new();
    // ONE `Page::load` in this binary — a second stands up a second SpiderMonkey context and
    // SIGSEGVs (the standing rule for every JS gate here). `load` drains the microtask queue, so
    // the `.then` claims are already settled by the time the document is read; if they were not,
    // every promise-delivered claim would be ABSENT and `contains` would report that as a wrong
    // answer rather than as an unpumped queue.
    let page = manuk_page::Page::load(HTML, "https://caps.test/", &fonts, 800.0);

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("MEDIA CAPS PROBE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_MEDIA_CAPABILITIES: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "mc:object",
        "`navigator.mediaCapabilities` was undefined, so every player's boot-time rendition scan \
         threw a TypeError before it could render anything",
    ),
    (
        "di:function",
        "`decodingInfo` is what shaka/dash.js/hls.js actually call; a feature detect reads its \
         typeof",
    ),
    (
        "ei:function",
        "and `encodingInfo` next to it — an absent method is the same TypeError one surface over",
    ),
    (
        "agree:true",
        "THE CLAIM THIS GATE IS FOR. `decodingInfo().supported` and `isTypeSupported()` are \
         compared IN THE PAGE across six contentTypes, so this catches a SECOND implementation of \
         the codec rule rather than checking each answer against a number I wrote down. t634 paid \
         to remove exactly that defect between two surfaces; a third one would restore it",
    ),
    (
        "mixed:true",
        "and the six strings do not all get the same answer — agreement across six identical \
         `false`s would be vacuous, which is how this kind of cross-check usually fails",
    ),
    (
        "sup:true",
        "Baseline H.264 in MP4 genuinely decodes here (openh264), so a rendition naming it is \
         supported",
    ),
    (
        "smooth:true",
        "`smooth` tracks `supported` and NOTHING ELSE, because this tree does not model decode \
         throughput — so it cannot honestly discriminate 4K from 360p and does not pretend to. \
         It is asserted here so the limitation is written down as an executable claim rather than \
         a comment: the day throughput is measured, this line goes red and forces the question",
    ),
    (
        "pe:false",
        "`powerEfficient` is FALSE and that is factually true of this tree: every decoder here is \
         software (openh264, symphonia, re_rav1d) and there is no VA-API/VideoToolbox/DXVA path at \
         all. It is a checkable claim, and a lie the day a hardware path lands",
    ),
    (
        "echo:true",
        "the configuration is echoed back — several players correlate an answer with the rendition \
         they asked about by reading it",
    ),
    (
        "mixedcfg:false",
        "a config whose VIDEO decodes and whose AUDIO does not (AV1-in-WebM + Opus) is NOT \
         supported. Answering true would pair a stream we can show with one we cannot hear and \
         call that playable — the same trade `av1opus:false` refuses in G_MEDIA_WEBM",
    ),
    (
        "webrtc:false",
        "WebRTC is a decided non-goal (STATUS.md), and an honest no about a decided non-goal is \
         not the same thing as an absence nobody has looked at",
    ),
    (
        "badtype:TypeError",
        "an invalid `type` REJECTS rather than resolving `supported:false` — a player must be able \
         to tell `you told me no` from `you did not understand the question`, and collapsing the \
         two hides its own bugs",
    ),
    (
        "noparts:TypeError",
        "and a config with neither audio nor video is the same class of error",
    ),
    (
        "enc:false",
        "`encodingInfo` answers a truthful no: nothing in this tree encodes",
    ),
];
