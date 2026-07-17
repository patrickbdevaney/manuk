//! **G_CRYPTO — `crypto.getRandomValues` / `crypto.randomUUID` must be a real CSPRNG, and must fill
//! and shape their output correctly.**
//!
//! This gate was born from a live security + correctness bug: the boot shim implemented both from
//! `Math.random()`. That is wrong on two independent axes, and both are the kind of failure that is
//! invisible until it is exploited or a value is inspected:
//!
//!   1. **Security.** `Math.random()` is a NON-cryptographic PRNG. Every session token, CSRF nonce,
//!      OAuth `state`, password-reset id and `crypto.randomUUID()` a page minted through this API was
//!      therefore *predictable* — the exact threat `crypto.getRandomValues` exists to remove. A
//!      browser that answers this call with a guessable stream is worse than one that throws, because
//!      the page believes it has entropy it does not have.
//!   2. **Correctness.** The old filler did `a[i] = (Math.random()*256)|0`, so a `Uint32Array` got
//!      values in `0..255` — 24 of every 32 bits always zero. And `randomUUID` never set the RFC 4122
//!      variant nibble, so it emitted strings that are not valid v4 UUIDs (a `!== 8|9|a|b` at index 19
//!      fails strict validators and collides across the reserved space).
//!
//! The fix routes entropy through the OS CSPRNG (`__cryptoRandomHex` → `getrandom`), fills through a
//! byte view so every element byte is random regardless of element width, and stamps the version (4)
//! and variant (10xx) bits. This gate asserts the observable consequences — and every one of them
//! goes RED against the `Math.random()` shim, which is what makes it a ratchet tooth and not a hope.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var r = [];

    // 1. Returns the SAME array object it was handed (spec: getRandomValues returns its argument).
    var a8 = new Uint8Array(16);
    r.push('same:' + (crypto.getRandomValues(a8) === a8));

    // 2. A Uint32Array must be filled with FULL 32-bit values, not 0..255. The old Math.random()
    //    filler wrote one byte per element, so every value was <= 255. 64 real u32 draws being ALL
    //    <= 255 has probability (256/2^32)^64 — a number with ~450 leading zeros. Effectively never.
    var u32 = new Uint32Array(64);
    crypto.getRandomValues(u32);
    var anyWide = false;
    for (var i = 0; i < u32.length; i++) { if (u32[i] > 255) { anyWide = true; break; } }
    r.push('u32wide:' + anyWide);

    // 3. randomUUID must be a valid RFC 4122 v4 UUID: version nibble '4', variant nibble in [89ab].
    var uuid = crypto.randomUUID();
    var V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
    r.push('uuidv4:' + V4.test(uuid));

    // 4. Two UUIDs must differ (a constant UUID would pass the shape test but be catastrophic).
    r.push('uuidUnique:' + (crypto.randomUUID() !== crypto.randomUUID()));

    // 5. Two independent 32-byte draws must differ (equal = 2^-256; a stuck/zero source shows here).
    var x = new Uint8Array(32), y = new Uint8Array(32);
    crypto.getRandomValues(x); crypto.getRandomValues(y);
    var equal = true;
    for (var i = 0; i < 32; i++) { if (x[i] !== y[i]) { equal = false; break; } }
    r.push('entropy:' + (!equal));

    // 6. Over-quota (> 65536 bytes) must THROW (QuotaExceededError), not silently truncate.
    var threw = false;
    try { crypto.getRandomValues(new Uint8Array(65537)); } catch (e) { threw = true; }
    r.push('quota:' + threw);

    // 7. A non-integer view (Float64Array) must THROW, not be filled with garbage.
    var threw2 = false;
    try { crypto.getRandomValues(new Float64Array(4)); } catch (e) { threw2 = true; }
    r.push('typeThrow:' + threw2);

    document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn crypto_random_is_a_real_csprng_and_correctly_shaped() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://crypto.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "same:true",       // returns its argument
        "u32wide:true",    // full 32-bit fill — RED against the Math.random() 0..255 filler
        "uuidv4:true", // valid RFC 4122 v4 (version + variant bits) — RED without the variant fix
        "uuidUnique:true", // not a constant
        "entropy:true", // independent draws differ (source is not stuck/zero)
        "quota:true",  // > 65536 bytes throws QuotaExceededError
        "typeThrow:true", // a Float64Array throws TypeMismatchError
    ] {
        assert!(
            got.contains(claim),
            "G_CRYPTO: expected {claim} in {got:?}\n  \
             crypto.getRandomValues/randomUUID must be a CRYPTOGRAPHICALLY SECURE source, correctly \
             filled and shaped. A predictable or mis-filled value is a silent security bug: the page \
             believes it has entropy it does not have."
        );
    }
}
