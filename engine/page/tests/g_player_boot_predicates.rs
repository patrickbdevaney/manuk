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
//! * **THE EME TRIPLE — and its assertion was INVERTED at t641, which is this gate working.** t640
//!   asserted the three interface objects ABSENT, so that the day any of them appeared the gate
//!   would go red and force the shaka question to be answered deliberately rather than discovered.
//!   **It went red one tick later, at t641, and the question was answered on the constitution's own
//!   words** — PART IV makes *Widevine/EME HD streaming* a permanent non-goal and in the same
//!   sentence prescribes *"documented, degraded gracefully"*, which omitting the interfaces is not.
//!   The interfaces now exist and grant nothing; `G_EME_HONEST_REFUSAL` holds the refusal. This
//!   half now asserts PRESENCE, and the pairing is the point: shaka's clause is satisfied **and**
//!   every key system is still refused, which are two claims that must never drift apart.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! | delete `changeType` from the SourceBuffer prototype | RED — `sbChangeType:function` |
//! | (t640) define `window.MediaKeys` while the triple was asserted absent | RED — the tripwire, which then fired for real at t641 |

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
    // ── The EME half — asserted PRESENT since t641 (inverted from t640's tripwire). See the module note.
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
        "MediaKeys:function",
        "INVERTED AT t641 — t640 asserted this ABSENT as a tripwire, it fired one tick later, and \
         the question it was built to force got answered from PART IV's own words (`degraded \
         gracefully`). shaka reads the EME interface objects as a proxy for `is this a real \
         browser` and refuses to run without them even on unencrypted content",
    ),
    (
        "rMKSA:function",
        "the second term of the same clause; asserted separately so a partial definition is caught",
    ),
    (
        "MediaKeySystemAccess:function",
        "and the third. PRESENCE here is only safe because G_EME_HONEST_REFUSAL asserts that every \
         key system — Widevine, PlayReady and Clear Key — is still REFUSED. These two gates must be \
         read together: interfaces without the refusal would be a promise this tree cannot keep",
    ),
];
