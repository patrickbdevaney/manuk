//! **G_DEVICE_IDENTITY — the last two completeness-identity gaps: `navigator.deviceMemory` and a
//! canonical `navigator.platform`.**
//!
//! Two small, honest facts that pushed real logged-in apps (LinkedIn, banks, Cloudflare-gated
//! consoles) onto their degraded/"unknown client" path:
//!
//!   * `navigator.deviceMemory` was ABSENT. Adaptive-loading bundles do `if (navigator.deviceMemory
//!     < 4)` to choose image quality / eager hydration; on `undefined` that comparison is silently
//!     `false` (the wrong branch), and `navigator.deviceMemory.toFixed()` throws. A bot detector also
//!     cross-checks it against the UA-CH surface, so its absence while `userAgentData` is present is
//!     itself an inconsistency tell. The spec QUANTISES it to {0.25,0.5,1,2,4,8} capped at 8.
//!   * `navigator.platform` was the raw lowercase `"linux x86_64"`. Real browsers report a canonical
//!     capitalised token that sniffers exact-match: `"Linux x86_64"` / `"MacIntel"` / `"Win32"`. A
//!     page testing `navigator.platform === 'Linux x86_64'` or `/^Linux/` saw a MISS.
//!
//! Teeth a stub cannot fudge, and each RED-provable:
//!   * `dm-quantised` — deviceMemory is one of the spec's allowed values, not an arbitrary number.
//!   * `plat-canonical` — the platform token's first character is UPPERCASE. This is exactly the old
//!     `"linux…"` bug: revert the canonicalisation and `plat-canonical` fails on the lowercase `l`.
//!   * `plat-consistent` — the legacy platform string starts with the UA-CH platform family
//!     (`"Linux x86_64"` begins with `"Linux"`), on the OS families where that legacy token is the
//!     OS name (Linux/Windows-`Win`); the two identity surfaces cannot disagree on what OS we are.
//!
//! Proven RED: delete `deviceMemory: 8` from the navigator literal → `dm-present`/`dm-quantised`
//! read `undefined`; restore the old `format!("{} {}", OS, ARCH)` platform → `plat-canonical` fails.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }

try {
  // ---- navigator.deviceMemory ----
  var dm = navigator.deviceMemory;
  push('dm-present:' + (typeof dm === 'number'));
  var allowed = [0.25, 0.5, 1, 2, 4, 8];
  push('dm-quantised:' + (allowed.indexOf(dm) >= 0));
  // it must not silently take the "low-memory" branch on a desktop
  push('dm-desktop:' + (dm >= 4));

  // ---- navigator.platform ----
  var p = navigator.platform;
  push('plat-str:' + (typeof p === 'string' && p.length > 0));
  // canonical casing: first char uppercase. The old raw value was "linux x86_64" (lowercase l).
  var c0 = p.charAt(0);
  push('plat-canonical:' + (c0 === c0.toUpperCase() && c0 !== c0.toLowerCase()));

  // ---- the two identity surfaces agree on the OS family ----
  // userAgentData.platform is the OS family token ("Linux"/"macOS"/"Windows"). On Linux and Windows
  // the legacy navigator.platform begins with that same family word ("Linux x86_64" / "Win32" from
  // "Windows"); macOS uses the unrelated legacy "MacIntel", so only assert the prefix where it holds.
  var fam = (navigator.userAgentData && navigator.userAgentData.platform) || '';
  var consistent = true;
  if (fam === 'Linux') { consistent = (p.indexOf('Linux') === 0); }
  else if (fam === 'Windows') { consistent = (p.indexOf('Win') === 0); }
  // macOS / unknown: no legacy-token prefix relationship to assert — pass.
  push('plat-consistent:' + consistent);
} catch (e) {
  push('THREW:' + e);
}

document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn navigator_device_memory_and_canonical_platform_close_the_identity_surface() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ident.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("DEVICE IDENTITY RESULT: {got}");

    for claim in [
        "dm-present:true", // navigator.deviceMemory exists — a call on `undefined` is what we kill
        "dm-quantised:true", // …and is one of the spec's privacy-preserving values, not arbitrary
        "dm-desktop:true", // …and does not silently pick the low-memory branch on a desktop
        "plat-str:true",   // navigator.platform is a non-empty string
        "plat-canonical:true", // …capitalised as a real browser reports it — the lowercase-`l` bug
        "plat-consistent:true", // …and agrees with userAgentData about which OS we are
    ] {
        assert!(
            got.contains(claim),
            "G_DEVICE_IDENTITY: expected `{claim}`\n  got: {got}\n\n  \
             `navigator.deviceMemory` must be present and quantised, and `navigator.platform` must be \
             the canonical capitalised token real browsers report (`Linux x86_64`, not the raw \
             lowercase `linux x86_64`). The gaps pushed LinkedIn / Cloudflare-gated consoles onto \
             their degraded path and read as a headless inconsistency."
        );
    }
}
