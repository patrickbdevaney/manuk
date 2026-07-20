//! **G_USERAGENTDATA — the User-Agent Client Hints surface (`navigator.userAgentData`).**
//!
//! Modern sites no longer parse the UA string; they read `navigator.userAgentData` and call
//! `getHighEntropyValues([...])`. When the object is `undefined` two things break at once: the call
//! throws on `undefined` and takes the surrounding feature-detection with it, and a headless
//! detector reads the absence as the single loudest "this is not a real browser" tell there is.
//!
//! This gate is BEHAVIOURAL and has teeth a stub cannot pass:
//!   * `getHighEntropyValues` must return ONLY the hints that were ASKED for (plus the always-present
//!     low-entropy set) — a shim that dumps every field fails `unasked-absent`.
//!   * the `uaFullVersion` it reports must actually appear in `navigator.userAgent` — the CH surface
//!     and the UA string are the same honest fact (Axis F: what we are, never a competitor's brand),
//!     so an inconsistent stub fails `consistent`.
//!
//! Proven RED: delete the `userAgentData` block in the window prelude and `present` reads
//! `undefined` while the first `getHighEntropyValues` call throws; return every hint unconditionally
//! and `unasked-absent` fails; hard-code a Chrome version string and `consistent` fails.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function finish() { document.getElementById('out').textContent = r.join(' '); }

try {
  var uad = navigator.userAgentData;
  push('present:' + (typeof uad === 'object' && uad !== null));

  // brands: a non-empty array of { brand:string, version:string }, and one of them is us.
  var b = uad.brands;
  var shapeOk = Array.isArray(b) && b.length >= 1;
  var mine = false;
  for (var i = 0; i < b.length; i++) {
    if (typeof b[i].brand !== 'string' || typeof b[i].version !== 'string') { shapeOk = false; }
    if (b[i].brand === 'Manuk') { mine = true; }
  }
  push('brands-shape:' + shapeOk);
  push('brand-manuk:' + mine);

  push('mobile-false:' + (uad.mobile === false));
  push('platform-str:' + (typeof uad.platform === 'string' && uad.platform.length > 0));

  // toJSON is the low-entropy set and NOT the method surface.
  var j = uad.toJSON();
  push('json:' + (typeof j.platform === 'string' && Array.isArray(j.brands) &&
                  j.mobile === false && typeof j.getHighEntropyValues === 'undefined'));

  // getHighEntropyValues: ask for TWO hints, prove BOTH resolve, prove an UNASKED hint is absent,
  // and prove the low-entropy set is folded in. Then prove the version is the same honest fact the
  // UA string carries.
  uad.getHighEntropyValues(['architecture', 'uaFullVersion']).then(function (v) {
    push('hev-arch:' + (typeof v.architecture === 'string' && v.architecture.length > 0));
    push('hev-fullver:' + (typeof v.uaFullVersion === 'string' && v.uaFullVersion.length > 0));
    // 'bitness' and 'model' were NOT requested — a selective implementation omits them.
    push('unasked-absent:' + (!('bitness' in v) && !('model' in v)));
    // the always-present low-entropy keys ride along, per the spec's resolution.
    push('hev-low:' + (Array.isArray(v.brands) && v.mobile === false &&
                       typeof v.platform === 'string'));
    // the CH version is the SAME string the UA advertises — not a competitor's number.
    push('consistent:' + (navigator.userAgent.indexOf(v.uaFullVersion) >= 0));
    finish();
  }, function (e) { push('hev-threw:' + e); finish(); });
} catch (e) {
  push('THREW:' + e);
  finish();
}
</script></body></html>"##;

#[test]
fn navigator_user_agent_data_is_a_real_consistent_client_hints_surface() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://uad.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("UAD RESULT: {got}");

    for claim in [
        "present:true", // the object exists — a call on `undefined` is what we are killing
        "brands-shape:true", // brands is [{brand,version}], not a bare string
        "brand-manuk:true", // and it honestly names us
        "mobile-false:true", // desktop
        "platform-str:true", // OS family present
        "json:true",    // toJSON is the low-entropy dict, not the method surface
        "hev-arch:true", // getHighEntropyValues resolves the asked-for hints
        "hev-fullver:true",
        "unasked-absent:true", // and returns ONLY what was asked — the anti-stub tooth
        "hev-low:true",        // with the low-entropy set folded in
        "consistent:true",     // the version matches the honest UA string — the anti-mimicry tooth
    ] {
        assert!(
            got.contains(claim),
            "G_USERAGENTDATA: expected `{claim}`\n  got: {got}\n\n  \
             `navigator.userAgentData` must be a real, self-consistent Client Hints surface. Its \
             absence throws in feature-detection code and reads as headless; a stub that dumps every \
             hint or reports a foreign version fails the `unasked-absent`/`consistent` teeth."
        );
    }
}
