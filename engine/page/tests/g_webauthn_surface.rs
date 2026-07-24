//! **G_WEBAUTHN_SURFACE — the passkey feature-detect surface exists and DEGRADES gracefully.**
//!
//! **The failure this gate exists for.** `navigator.credentials` did not exist, and neither did
//! `window.PublicKeyCredential`. A growing wall of sites — banks, GitHub, Microsoft, Apple, every
//! passkey-first login — runs, inside a sign-in click handler:
//!
//! ```js
//! navigator.credentials.get({ publicKey: options }).then(useAssertion).catch(showPasswordForm);
//! ```
//!
//! When `navigator.credentials` is `undefined`, `navigator.credentials.get(...)` is a SYNCHRONOUS
//! `TypeError` — thrown before any promise exists, so the `.catch()` the page attached (to the promise
//! it expected back) never runs. The whole ceremony dies with an uncaught error and, crucially, the
//! page never reaches `showPasswordForm`: the user is stranded on a login screen that does nothing.
//! That is a hard wall, not a degraded experience.
//!
//! **What is asserted here is detection + graceful degradation, NOT WebAuthn.** We have no
//! authenticator — no platform TPM binding, no roaming key — and the gate holds the implementation to
//! that honesty rather than faking an assertion:
//!
//!   * `window.PublicKeyCredential` is a function (sites feature-detect it by truthiness).
//!   * `PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()` resolves to `false` — we
//!     have no platform authenticator, so a site that asks first never offers the passkey button.
//!   * `isConditionalMediationAvailable()` resolves to `false` — no autofill/conditional UI.
//!   * `navigator.credentials.get({publicKey})` REJECTS (returns a rejected promise, does NOT throw
//!     synchronously) with a `NotAllowedError` DOMException — the exact "no credential / cancelled"
//!     signal a real browser yields, which routes the page onto its password fallback.
//!   * `navigator.credentials.create({publicKey})` rejects the same way (registration needs an
//!     authenticator too).
//!   * A `get()`/`create()` with NO `publicKey` member (a password/federated request) resolves to
//!     `null`, which is spec — a missing stored credential is not an error.
//!   * `store()` and `preventSilentAccess()` resolve quietly.
//!
//! **RED direction.** Removing the surface block from the prelude restores the original engine: the
//! `get({publicKey})` call becomes `THREW:TypeError` (`navigator.credentials` is undefined) and the
//! `pkc:` / `uvpaa:` claims flip to `undefined`/absent — precisely the hard wall described above.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var r = [];
    var done = function () { document.getElementById('out').textContent = r.join(' '); };

    try {
        // Feature-detect surface: both are present, PublicKeyCredential is a function.
        r.push('pkc:' + (typeof window.PublicKeyCredential === 'function'));
        r.push('creds:' + (typeof navigator.credentials === 'object' && navigator.credentials !== null));
        r.push('get:' + (typeof navigator.credentials.get === 'function'));
        r.push('create:' + (typeof navigator.credentials.create === 'function'));

        // The publicKey get() must REJECT, not throw synchronously. If it threw, the catch below the
        // try would record THREW and the getrej claim would be absent — the original failure exactly.
        var challenge = new Uint8Array([1, 2, 3, 4]);
        var getP = navigator.credentials.get({ publicKey: { challenge: challenge, timeout: 1000 } });
        r.push('getispromise:' + (getP && typeof getP.then === 'function'));

        var createP = navigator.credentials.create({ publicKey: {
            challenge: challenge, rp: { name: 'x' }, user: { id: challenge, name: 'a', displayName: 'A' },
            pubKeyCredParams: [{ type: 'public-key', alg: -7 }]
        } });

        // Password/federated request (no publicKey) resolves to null — not an error.
        var pwP = navigator.credentials.get({ password: true });

        Promise.all([
            window.PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable(),
            window.PublicKeyCredential.isConditionalMediationAvailable(),
            getP.then(function () { return 'RESOLVED'; }, function (e) { return e && e.name; }),
            createP.then(function () { return 'RESOLVED'; }, function (e) { return e && e.name; }),
            pwP.then(function (v) { return v === null ? 'null' : 'val'; }, function (e) { return 'ERR:' + (e && e.name); }),
            navigator.credentials.store({}).then(function () { return 'ok'; }, function () { return 'threw'; }),
            navigator.credentials.preventSilentAccess().then(function () { return 'ok'; }, function () { return 'threw'; })
        ]).then(function (a) {
            r.push('uvpaa:' + (a[0] === false));
            r.push('condmed:' + (a[1] === false));
            r.push('getrej:' + (a[2] === 'NotAllowedError'));
            r.push('createrej:' + (a[3] === 'NotAllowedError'));
            r.push('pwnull:' + (a[4] === 'null'));
            r.push('store:' + (a[5] === 'ok'));
            r.push('preventsilent:' + (a[6] === 'ok'));
            done();
        }, function (e) { r.push('ALLTHREW:' + (e && e.name ? e.name : e)); done(); });
    } catch (e) {
        r.push('THREW:' + (e && e.name ? e.name : e));
        done();
    }
</script></body></html>"#;

#[test]
fn passkey_feature_detect_surface_exists_and_degrades_gracefully() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://login.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("pkc:true", "window.PublicKeyCredential must be a function — sites feature-detect it by truthiness to decide whether to show the passkey button"),
        ("creds:true", "navigator.credentials must be an object, or navigator.credentials.get(...) is a synchronous TypeError the page's promise .catch never sees"),
        ("get:true", "credentials.get is the assertion entry point every passkey login calls"),
        ("create:true", "credentials.create is the registration entry point"),
        ("getispromise:true", "get({publicKey}) must RETURN a promise, not throw — the whole point is that the rejection is catchable"),
        ("uvpaa:true", "we have no platform authenticator, so isUserVerifyingPlatformAuthenticatorAvailable must honestly resolve false, steering the site off the passkey path (the claim reads true when the resolved value is correctly false)"),
        ("condmed:true", "no conditional/autofill mediation, so isConditionalMediationAvailable resolves false (claim reads true when the resolved value is correctly false)"),
        ("getrej:true", "get({publicKey}) must reject with NotAllowedError — the 'no credential / cancelled' signal that routes the page onto its password fallback"),
        ("createrej:true", "create({publicKey}) must reject with NotAllowedError — registration also needs an authenticator we do not have"),
        ("pwnull:true", "a password/federated request (no publicKey) resolves to null, not an error — a missing stored credential is not a failure"),
        ("store:true", "credentials.store() resolves quietly — a page saving a password credential expects a resolved promise"),
        ("preventsilent:true", "credentials.preventSilentAccess() resolves quietly — logout paths call it and expect no throw"),
    ] {
        assert!(
            got.contains(claim),
            "G_WEBAUTHN_SURFACE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
