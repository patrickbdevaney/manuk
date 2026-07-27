//! # G_PLAYER_BOOT_PREDICATES — the exact globals hls.js / dash.js / shaka-player read to decide whether to run
//!
//! **These are not predicates I invented.** Tick 640 fetched the three real shipped libraries from
//! jsDelivr (hls.js 1.5.17, dash.js 4.7.4, shaka-player 4.11.2 — 1.8MB of production code) and ran
//! their real boot paths in this engine. hls.js reported `isSupported(): true`, constructed, and
//! attached to a `<video>`. dash.js constructed and initialised. **shaka-player reported
//! `isBrowserSupported(): false`**, and its own minified source names why:
//!
//! ```js
//! !(window.MediaKeys && window.navigator && window.navigator.requestMediaKeySystemAccess &&
//!   window.MediaKeySystemAccess && window.MediaKeySystemAccess.prototype.getConfiguration) ? !1 : …
//! ```
//!
//! It reads the **EME interface objects as a proxy for "is this a real browser"** and refuses to run
//! at all without them — *including for unencrypted content*. Every MSE predicate it checks was
//! already green.
//!
//! This gate pins that measured state so it cannot drift silently, without vendoring 1.8MB of
//! third-party code into the tree. Two halves, and the second is the unusual one:
//!
//! * **PRESENT** — the MSE surface real players gate on. `SourceBuffer.prototype.changeType` in
//!   particular is read by shaka and is easy to lose, because nothing in this repo calls it.
//! * **ABSENT, ON PURPOSE** — the EME triple. Asserted as absent so that **the day any of it
//!   appears, this gate goes red and forces the shaka question to be answered deliberately** rather
//!   than discovered. EME *playback* is a permanent non-goal (`STATUS.md`); whether the interfaces
//!   should exist and honestly reject every key system — which is what Chrome without a CDM does —
//!   is open, and is not a question a stray commit should get to settle by accident.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! | delete `changeType` from the SourceBuffer prototype | RED — `sbChangeType:function` |
//! | define `window.MediaKeys = function(){}` | RED — `MediaKeys:undefined`, which is the tripwire firing exactly as designed |

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <video id="v"></video><div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    var SB = window.SourceBuffer, MS = window.MediaSource;
    // ── The MSE half. Every one of these is read by a real player's support check.
    p('MediaSource:' + typeof MS);
    p('isTypeSupported:' + (MS ? typeof MS.isTypeSupported : 'n/a'));
    p('SourceBuffer:' + typeof SB);
    p('sbAppend:' + (SB && SB.prototype ? typeof SB.prototype.appendBuffer : 'n/a'));
    p('sbChangeType:' + (SB && SB.prototype ? typeof SB.prototype.changeType : 'n/a'));
    p('sbRemove:' + (SB && SB.prototype ? typeof SB.prototype.remove : 'n/a'));
    // ── The EME half — asserted ABSENT, deliberately. See the module note.
    p('MediaKeys:' + typeof window.MediaKeys);
    p('rMKSA:' + typeof (navigator && navigator.requestMediaKeySystemAccess));
    p('MediaKeySystemAccess:' + typeof window.MediaKeySystemAccess);
  </script>
</body></html>"##;

#[test]
fn the_globals_real_players_gate_on_are_where_tick_640_measured_them() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://player.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("PLAYER BOOT PREDICATES: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_PLAYER_BOOT_PREDICATES: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "MediaSource:function",
        "hls.js, dash.js and shaka all gate on the MediaSource constructor existing",
    ),
    (
        "isTypeSupported:function",
        "and on the static support probe — it is how every one of them filters its rendition list",
    ),
    (
        "SourceBuffer:function",
        "shaka reads `window.SourceBuffer` directly, not via a MediaSource instance",
    ),
    (
        "sbAppend:function",
        "`SourceBuffer.prototype.appendBuffer` must be ON THE PROTOTYPE — a player checks the \
         prototype, never an instance, so a per-instance closure method reads as absent however \
         well it works",
    ),
    (
        "sbChangeType:function",
        "`changeType` is read by shaka's MSE support check and is the easiest of these to lose, \
         because nothing in this repo calls it. Measured present at t640",
    ),
    (
        "sbRemove:function",
        "`remove` too — shaka monkey-patches it on some platforms, which requires it to be there",
    ),
    (
        "MediaKeys:undefined",
        "ABSENT ON PURPOSE, and asserted so it cannot change by accident. shaka-player's \
         isBrowserSupported() reads the EME interface objects as a proxy for `is this a real \
         browser` and returns FALSE without them, even for unencrypted content — so the day this \
         becomes defined, shaka's verdict changes and this gate goes red to force that question to \
         be answered deliberately rather than discovered",
    ),
    (
        "rMKSA:undefined",
        "the second term of the same clause; asserted separately so a partial definition is caught",
    ),
    (
        "MediaKeySystemAccess:undefined",
        "and the third. EME PLAYBACK is a permanent non-goal (STATUS.md); whether the INTERFACES \
         should exist and honestly reject every key system is a separate open question, and not one \
         a stray commit should settle by accident",
    ),
];
