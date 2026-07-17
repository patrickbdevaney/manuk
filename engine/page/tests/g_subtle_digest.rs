//! **G_SUBTLE_DIGEST — `crypto.subtle.digest` computes correct SHA digests and returns a Promise.**
//!
//! `crypto.subtle.digest(algo, data)` is what Subresource-Integrity checks, content-addressed caches
//! and many auth/signing libraries call. It was absent: `crypto.subtle` was `undefined`, so
//! `crypto.subtle.digest(...)` threw a `TypeError` and took whatever was running with it. This gate
//! asserts the observable contract against **known test vectors** (so it is deterministic, not
//! statistical): `digest` returns a Promise, and it resolves to the correct SHA-1/256/512 bytes for
//! `"abc"` and the empty message, accepts both the string and `{name}` algorithm forms, and *rejects*
//! (rather than mis-hashing) an unknown algorithm. Every claim is RED against the absent API.
//!
//! The result is read after load, which requires the microtask queue to drain during `Page::load` —
//! the same delivery path `MutationObserver` (queueMicrotask) already relies on. All work funnels
//! through one `Promise.all().then`, so a single full drain writes the output.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    function hexOf(buf){ var b=new Uint8Array(buf), s=''; for(var i=0;i<b.length;i++){ s += ('0'+b[i].toString(16)).slice(-2); } return s; }
    var enc = new TextEncoder();
    var out = document.getElementById('out');
    var r = [];

    var p = crypto.subtle.digest('SHA-256', enc.encode('abc'));
    r.push('isPromise:' + (!!p && typeof p.then === 'function'));

    Promise.all([
      p,                                                        // SHA-256("abc")
      crypto.subtle.digest('SHA-1', enc.encode('abc')),         // SHA-1("abc")
      crypto.subtle.digest('SHA-512', enc.encode('abc')),       // SHA-512("abc")
      crypto.subtle.digest({ name: 'SHA-256' }, new Uint8Array(0)), // object-form algo + empty message
      crypto.subtle.digest('MD5', enc.encode('x')).then(function(){ return 'NOREJECT'; }, function(e){ return e.name; })
    ]).then(function(a){
      r.push('sha256:' + (hexOf(a[0]) === 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'));
      r.push('sha1:' + (hexOf(a[1]) === 'a9993e364706816aba3e25717850c26c9cd0d89d'));
      r.push('sha512:' + (hexOf(a[2]) === 'ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f'));
      r.push('emptyObjAlgo:' + (hexOf(a[3]) === 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'));
      r.push('rejectBad:' + (a[4] === 'NotSupportedError'));
      out.textContent = r.join(' ');
    }, function(err){ out.textContent = 'PROMISE_REJECTED:' + err; });
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn subtle_digest_computes_known_sha_vectors() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://digest.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "isPromise:true",    // digest returns a thenable
        "sha256:true",       // SHA-256("abc") vector
        "sha1:true",         // SHA-1("abc") vector
        "sha512:true",       // SHA-512("abc") vector
        "emptyObjAlgo:true", // {name:'SHA-256'} + empty message → the empty-string SHA-256
        "rejectBad:true", // an unknown algorithm rejects with NotSupportedError, not a wrong hash
    ] {
        assert!(
            got.contains(claim),
            "G_SUBTLE_DIGEST: expected {claim} in {got:?}\n  \
             crypto.subtle.digest must compute the correct digest for a known vector and return a \
             Promise. A missing subtle is a TypeError; a wrong digest silently fails SRI/auth checks."
        );
    }
}
