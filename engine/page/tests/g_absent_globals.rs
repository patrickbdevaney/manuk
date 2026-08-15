//! **G_ABSENT_GLOBALS — `isSecureContext` and `HTMLDocument` exist, and they tell the TRUTH.**
//!
//! Both come from this loop's own 200-site CrUX throw histogram (surface audit #68, t1263), at
//! 2 of 200 each: `ReferenceError: isSecureContext is not defined` and
//! `ReferenceError: HTMLDocument is not defined`. They are in the **throw class**, not the
//! missing-feature class, and that difference is why two lines are worth a gate: a page reading a
//! global it expects to exist does not feature-detect first, so absence is a `ReferenceError` that
//! takes the rest of the bundle with it rather than a fallback path.
//!
//! **The teeth are the VALUES, not the existence**, because both have an easy wrong implementation
//! that no feature detect can see:
//!
//! - `isSecureContext = true` (hardcoded) would send a page down the `crypto.subtle` / service-worker
//!   path on an `http:` origin — a *worse* failure than the ReferenceError it replaces. So this gate
//!   asserts **both directions**: `https:` is secure, plain `http:` on a non-loopback host is not.
//! - `HTMLDocument = function(){}` (a fresh object) would make the name exist and make every
//!   `document instanceof HTMLDocument` answer **false**, which is the half-presence wall this
//!   codebase keeps naming. The spec says the alias *is* `Document`, so identity is asserted.
//!
//! Proven RED: without the two definitions both pages report `err:ReferenceError: ... is not
//! defined`. With a hardcoded `true`, the `http:` row fails alone; with a fresh function,
//! `doc-is-html-document` and `alias-is-document` fail while `defined` passes.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
  var r = [];
  try {
    r.push('secure-defined:' + (typeof isSecureContext !== 'undefined'));
    r.push('secure:' + isSecureContext);
    r.push('htmldoc-defined:' + (typeof HTMLDocument !== 'undefined'));
    // The legacy detects, verbatim — these are what the name exists FOR.
    r.push('doc-is-html-document:' + (document instanceof HTMLDocument));
    r.push('alias-is-document:' + (HTMLDocument === Document));
    document.getElementById('out').textContent = r.join(' ');
  } catch (e) {
    document.getElementById('out').textContent = r.join(' ') + ' err:' + e;
  }
</script>
</body></html>"##;

fn run(url: &str) -> String {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, url, &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    page.dom().text_content(out)
}

#[test]
fn absent_globals_exist_and_tell_the_truth() {
    // ── A SECURE ORIGIN.
    let got = run("https://secure.test/");
    println!("ABSENT-GLOBALS (https): {got}");
    for claim in [
        "secure-defined:true",       // the global exists at all (the ReferenceError)
        "secure:true",               // — and https IS a secure context
        "htmldoc-defined:true",      // the legacy interface name exists
        "doc-is-html-document:true", // — and the document is actually an instance of it
        "alias-is-document:true",    // — because it IS Document, per spec, not a lookalike
    ] {
        assert!(
            got.contains(claim),
            "G_ABSENT_GLOBALS(https): expected `{claim}`\n  got: {got}\n\n  \
             `isSecureContext` and `HTMLDocument` are read UNGUARDED by real bundles, so absence is \
             a ReferenceError that takes the rest of the script with it. `HTMLDocument` must BE \
             `Document` (HTML spec) — a fresh function would make the name exist and every \
             `instanceof` answer false, which is half-presence, not presence."
        );
    }

    // ── AN INSECURE ORIGIN. The half of this that a hardcoded `true` gets wrong, and the reason
    //    the value is asserted rather than the existence: answering `true` on plain http sends a
    //    page down the crypto.subtle / service-worker path, which fails WORSE than not answering.
    let got = run("http://plain.test/");
    println!("ABSENT-GLOBALS (http): {got}");
    assert!(
        got.contains("secure:false"),
        "G_ABSENT_GLOBALS(http): expected `secure:false`\n  got: {got}\n\n  \
         A plain-http, non-loopback origin is NOT a secure context. Hardcoding `true` would make \
         this global a wrong answer of the right type — no feature detect can see it, and it routes \
         the page into APIs that are gated on it."
    );

    // ── LOCALHOST over plain http IS a secure context (W3C Secure Contexts §3.1), which is the row
    //    that separates "reads the scheme" from "reads the scheme correctly".
    let got = run("http://localhost:8080/");
    println!("ABSENT-GLOBALS (localhost): {got}");
    assert!(
        got.contains("secure:true"),
        "G_ABSENT_GLOBALS(localhost): expected `secure:true`\n  got: {got}\n\n  \
         `http://localhost` IS a secure context (W3C Secure Contexts §3.1 — a potentially \
         trustworthy origin). Every local dev server on the web depends on this being true."
    );
}
