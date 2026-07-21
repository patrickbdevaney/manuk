//! **G_CRYPTO_HKDF — `crypto.subtle.deriveBits` with HKDF.**
//!
//! HKDF (RFC 5869) is the key-derivation modern protocols and token schemes use to expand one secret
//! into keying material. It was absent (`deriveBits is not a function`). It is Extract-then-Expand, both
//! built on the tick-306 HMAC — a pure composition of the existing hash, so it is verifiable against the
//! RFC 5869 KNOWN-ANSWER vectors. This gate uses RFC 5869 Test Case 1.
//!
//! Proven RED: remove `deriveBits` and `present`/`okm-vector` fail while the call throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function finish() { document.getElementById('out').textContent = r.join(' '); }
function hex(buf) { var v = new Uint8Array(buf), s = ''; for (var i = 0; i < v.length; i++) { s += ('0' + v[i].toString(16)).slice(-2); } return s; }

try {
  push('present:' + (typeof crypto.subtle.deriveBits === 'function'));

  // RFC 5869 Test Case 1 (SHA-256).
  var ikm = new Uint8Array(22); for (var i = 0; i < 22; i++) { ikm[i] = 0x0b; }
  var salt = new Uint8Array([0,1,2,3,4,5,6,7,8,9,10,11,12]);
  var info = new Uint8Array([0xf0,0xf1,0xf2,0xf3,0xf4,0xf5,0xf6,0xf7,0xf8,0xf9]);
  var expected = '3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865';

  crypto.subtle.importKey('raw', ikm, { name: 'HKDF' }, false, ['deriveBits'])
    .then(function (key) {
      return crypto.subtle.deriveBits({ name: 'HKDF', hash: 'SHA-256', salt: salt, info: info }, key, 42 * 8);
    })
    .then(function (bits) {
      push('okm-vector:' + (hex(bits) === expected));
      push('length:' + (new Uint8Array(bits).length === 42));
      finish();
    }, function (e) { push('CHAIN-THREW:' + e); finish(); });
} catch (e) {
  push('THREW:' + e);
  finish();
}
</script></body></html>"##;

#[test]
fn crypto_subtle_hkdf_matches_rfc5869() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://hkdf.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CRYPTO-HKDF RESULT: {got}");

    for claim in [
        "present:true",
        "okm-vector:true", // matches the RFC 5869 Test Case 1 output keying material
        "length:true",     // 42 bytes derived
    ] {
        assert!(
            got.contains(claim),
            "G_CRYPTO_HKDF: expected `{claim}`\n  got: {got}\n\n  \
             `crypto.subtle.deriveBits` with HKDF must match the RFC 5869 known-answer output. A wrong \
             Extract/Expand fails `okm-vector`."
        );
    }
}
