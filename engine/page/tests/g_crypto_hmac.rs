//! **G_CRYPTO_HMAC — `crypto.subtle` HMAC (`importKey` / `sign` / `verify`).**
//!
//! The signature path every webhook-signature check and HS256 JWT verifier uses. `crypto.subtle.digest`
//! was already real (RustCrypto); `importKey`/`sign`/`verify` were absent, so HMAC threw `is not a
//! function`. HMAC is a standard composition of the existing correct SHA (RFC 2104), so it is verifiable
//! against KNOWN-ANSWER vectors — this gate uses RFC 4231 Test Case 2, which pins the exact bytes.
//!
//! Proven RED: remove `sign` and `present`/`sign-vector` fail while the call throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function finish() { document.getElementById('out').textContent = r.join(' '); }
function hex(buf) { var v = new Uint8Array(buf), s = ''; for (var i = 0; i < v.length; i++) { s += ('0' + v[i].toString(16)).slice(-2); } return s; }

try {
  push('present:' + (typeof crypto.subtle.importKey === 'function' &&
                     typeof crypto.subtle.sign === 'function' &&
                     typeof crypto.subtle.verify === 'function'));

  var enc = new TextEncoder();
  // RFC 4231 Test Case 2: key "Jefe", data "what do ya want for nothing?".
  var expected = '5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843';

  crypto.subtle.importKey('raw', enc.encode('Jefe'), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign', 'verify'])
    .then(function (key) {
      var msg = enc.encode('what do ya want for nothing?');
      return crypto.subtle.sign('HMAC', key, msg).then(function (sig) {
        push('sign-vector:' + (hex(sig) === expected));
        // verify accepts the correct signature and rejects a tampered one.
        return crypto.subtle.verify('HMAC', key, sig, msg).then(function (ok) {
          push('verify-good:' + (ok === true));
          var bad = new Uint8Array(sig); bad[0] ^= 0xff;
          return crypto.subtle.verify('HMAC', key, bad, msg).then(function (ok2) {
            push('verify-bad:' + (ok2 === false));
            finish();
          });
        });
      });
    }).then(null, function (e) { push('CHAIN-THREW:' + e); finish(); });
} catch (e) {
  push('THREW:' + e);
  finish();
}
</script></body></html>"##;

#[test]
fn crypto_subtle_hmac_matches_rfc4231() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://hmac.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CRYPTO-HMAC RESULT: {got}");

    for claim in [
        "present:true",
        "sign-vector:true", // matches the RFC 4231 known-answer HMAC-SHA256
        "verify-good:true",
        "verify-bad:true", // a tampered signature is rejected
    ] {
        assert!(
            got.contains(claim),
            "G_CRYPTO_HMAC: expected `{claim}`\n  got: {got}\n\n  \
             `crypto.subtle` HMAC importKey/sign/verify must match the RFC 4231 test vector and \
             reject a tampered signature. A wrong construction fails `sign-vector`."
        );
    }
}
