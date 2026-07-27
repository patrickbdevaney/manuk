//! # G_EME_HONEST_REFUSAL — the EME interfaces exist, and nothing is ever granted
//!
//! **This gate guards a scope decision, so it asserts the refusal harder than the presence.**
//!
//! `CONSTITUTION.MD` PART IV makes *"Widevine/EME HD streaming"* a permanent non-goal — a licensing
//! wall, never chased — and in the same sentence prescribes how to hold it: *"Documented, **degraded
//! gracefully**, never chased."* t640 measured what omitting the interface objects actually cost:
//! shaka-player 4.11.2 answered `isBrowserSupported(): false` on this clause of its own source —
//!
//! ```js
//! !(window.MediaKeys && window.navigator && window.navigator.requestMediaKeySystemAccess &&
//!   window.MediaKeySystemAccess && window.MediaKeySystemAccess.prototype.getConfiguration) ? !1 : …
//! ```
//!
//! — reading EME's presence as a proxy for *"is this a real browser"* and refusing to run **even for
//! unencrypted content**. The absence turned *"encrypted video will not play"* into *"shaka-player
//! will not run at all"*, which is a hard failure on precisely the case PART IV says to degrade
//! gracefully into.
//!
//! ## The honesty guard IS the design
//!
//! There is no CDM in this tree. **`requestMediaKeySystemAccess` must never resolve** — a resolved
//! access object sends a site down a decryption path that ends worse than the refusal did, which is
//! the "advertise before it works" failure `MEDIA.md` names, wearing DRM's clothes. So the claims
//! below are weighted accordingly: four assert that *nothing is granted*, including for
//! `org.w3.clearkey`, which needs a decryptor we do not have and would be the same lie in a smaller
//! font.
//!
//! Netflix, Spotify and Disney+ remain unreachable. They were unreachable before. What changed is
//! that a **clear-content** player boots.
//!
//! ## Measured against real shipped code, not only this fixture
//!
//! With this in place, shaka-player 4.11.2 (660KB from jsDelivr) reports
//! `isBrowserSupported(): true`, constructs, and completes `probeSupport()` — 44 media types and 8
//! DRM key systems probed, every key system refused. hls.js 1.5.17 and dash.js 4.7.4 are unmoved.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! | `requestMediaKeySystemAccess` resolves an access object instead of rejecting | RED — `wv:NotSupportedError`, the single most important claim here |
//! | `org.w3.clearkey` special-cased to resolve | RED — `ck:NotSupportedError` catches the plausible "but Clear Key is just AES" concession |
//! | drop `getConfiguration` from the prototype | RED — `shakaClause:true`, the exact predicate that made this tick necessary |

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <video id="v"></video><div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    var CFG = [{ initDataTypes: ['cenc'],
                 videoCapabilities: [{ contentType: 'video/mp4; codecs="avc1.42E01E"' }] }];

    // ── Presence. These are what a support check READS; none of them grants anything.
    p('MediaKeys:' + typeof window.MediaKeys);
    p('MKSA:' + typeof window.MediaKeySystemAccess);
    p('MKSession:' + typeof window.MediaKeySession);
    p('rMKSA:' + typeof navigator.requestMediaKeySystemAccess);
    p('getConfig:' + (window.MediaKeySystemAccess && window.MediaKeySystemAccess.prototype
                       ? typeof window.MediaKeySystemAccess.prototype.getConfiguration : 'n/a'));
    // shaka-player's ACTUAL clause, transcribed from its minified source.
    p('shakaClause:' + !!(window.MediaKeys && window.navigator &&
        window.navigator.requestMediaKeySystemAccess && window.MediaKeySystemAccess &&
        window.MediaKeySystemAccess.prototype.getConfiguration));
    // Interface objects are not constructible from script.
    try { new window.MediaKeys(); p('ctor:RESOLVED-BAD'); }
    catch (e) { p('ctor:' + (e && e.name ? e.name : 'threw')); }

    // ── REFUSAL. The load-bearing half.
    navigator.requestMediaKeySystemAccess('com.widevine.alpha', CFG)
      .then(function () { p('wv:GRANTED-BAD'); }, function (e) { p('wv:' + e.name); });
    navigator.requestMediaKeySystemAccess('com.microsoft.playready', CFG)
      .then(function () { p('pr:GRANTED-BAD'); }, function (e) { p('pr:' + e.name); });
    navigator.requestMediaKeySystemAccess('org.w3.clearkey', CFG)
      .then(function () { p('ck:GRANTED-BAD'); }, function (e) { p('ck:' + e.name); });

    // ── Argument validation still runs, and is DISTINGUISHABLE from the refusal: a player that
    //    passes garbage should learn that from us rather than have it masked.
    navigator.requestMediaKeySystemAccess('', CFG)
      .then(function () { p('empty:RESOLVED-BAD'); }, function (e) { p('empty:' + e.name); });
    navigator.requestMediaKeySystemAccess('com.widevine.alpha', [])
      .then(function () { p('nocfg:RESOLVED-BAD'); }, function (e) { p('nocfg:' + e.name); });
  </script>
</body></html>"##;

#[test]
fn the_eme_interfaces_are_present_and_grant_nothing() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://eme.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("EME PROBE: {got}");

    // Belt and braces on the thing that must never happen: no phrasing of "granted" may appear.
    assert!(
        !got.contains("GRANTED-BAD") && !got.contains("RESOLVED-BAD"),
        "EME must grant NOTHING — there is no CDM in this tree, and a resolved access object sends \
         a site down a decryption path that ends worse than the refusal did.\n  got: {got}"
    );

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_EME_HONEST_REFUSAL: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "shakaClause:true",
        "THE CLAIM THIS TICK EXISTS FOR — shaka-player's own predicate, transcribed from its \
         minified source. False here means shaka refuses to run at all, including on unencrypted \
         content, which is the hard failure PART IV's `degraded gracefully` forbids",
    ),
    ("MediaKeys:function", "the first term of that clause"),
    ("MKSA:function", "the third"),
    (
        "getConfig:function",
        "the fourth, and it is read off the PROTOTYPE — no instance is ever created, which is \
         exactly why a per-instance definition would not have helped",
    ),
    ("rMKSA:function", "the second, on navigator"),
    (
        "MKSession:function",
        "`MediaKeySession` too: a player that feature-detects the session interface must not find a \
         hole where the others are filled",
    ),
    (
        "ctor:TypeError",
        "these are interface objects, not constructors — `new MediaKeys()` is an Illegal \
         constructor, as in every real engine",
    ),
    (
        "wv:NotSupportedError",
        "WIDEVINE IS REFUSED. There is no CDM and no licensing relationship, and this is the claim \
         that keeps the interfaces from becoming a promise. `NotSupportedError` is the spec's own \
         answer for `no supported configuration` and what Chrome without a CDM returns",
    ),
    ("pr:NotSupportedError", "and PlayReady, for the same reason"),
    (
        "ck:NotSupportedError",
        "AND CLEAR KEY — the concession that looks harmless because Clear Key is `just AES` and \
         needs no licence. It still needs a decryptor this tree does not have, so granting it would \
         be the same lie in a smaller font",
    ),
    (
        "empty:TypeError",
        "argument validation still runs and is DISTINGUISHABLE from the refusal — a player passing \
         garbage should learn that from us, not have it masked by a blanket NotSupportedError",
    ),
    ("nocfg:TypeError", "an empty configuration list is the same class of caller error"),
];
