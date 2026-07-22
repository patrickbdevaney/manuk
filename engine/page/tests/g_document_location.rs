//! **G_DOCUMENT_LOCATION — the document's URL surface: `document.location` / `document.URL` /
//! `document.documentURI`.**
//!
//! Found by the tick-401 re-keyed oracle run, as a NAMED console error on okta.com:
//! `TypeError: can't access property "search", window.document.location is undefined` — the
//! Identity components read `document.location.search` inside their async mount, the read threw,
//! and whole subtrees never came into existence. Under positional keying that damage was booked
//! as 316 phantom "display diffs"; under selector-path keying it books as honest tree drift with
//! this error naming the organ. `window.location` has been a full Location shim for hundreds of
//! ticks — `document.location`, which the spec defines as the SAME object, was never aliased.
//!
//! Teeth, and the shape that matters: `g.location` is REPLACED WHOLESALE by `__applyUrl` on every
//! SPA navigation (pushState, hash change, assign) — so a copied reference goes stale on the
//! first route change. The alias must be an ACCESSOR. `same-after-push` is the claim that proves
//! it: pushState swaps the underlying object, and `document.location` must track the swap.
//! Assigning `document.location = url` must navigate (the legacy idiom every redirect-after-login
//! page uses). `document.URL` and `document.documentURI` are the read-only spellings of the same
//! fact and are asserted against the LIVE location, post-navigation.
//!
//! Proven RED: with the prelude alias absent, `has-docloc` reads `false` and every downstream
//! claim reads `THREW:` — the okta failure, reproduced in one line.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
try {
  r.push('has-docloc:' + (typeof document.location === 'object' && document.location !== null));
  r.push('same-object:' + (document.location === window.location));
  r.push('search:' + document.location.search);
  r.push('origin:' + document.location.origin);
  r.push('doc-url:' + (document.URL === window.location.href));
  r.push('doc-uri:' + (document.documentURI === window.location.href));

  // The accessor must survive the object swap: pushState REPLACES g.location wholesale.
  history.pushState(null, '', '/routed?tab=2');
  r.push('same-after-push:' + (document.location === window.location));
  r.push('search-after-push:' + document.location.search);
  r.push('url-after-push:' + (document.URL === window.location.href));

  // The legacy navigation idiom: assigning document.location navigates.
  document.location = '/next?step=3';
  r.push('assign-navigates:' + window.location.pathname + window.location.search);
} catch (e) {
  r.push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn document_location_is_the_window_location() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(
        HTML,
        "https://app.test/cb?code=abc&state=xyz",
        &fonts,
        800.0,
    );
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("DOCUMENT-LOCATION RESULT: {got}");

    for claim in [
        "has-docloc:true",
        "same-object:true", // the spec: document.location IS window.location
        "search:?code=abc&state=xyz", // the exact read okta died on
        "origin:https://app.test",
        "doc-url:true", // document.URL mirrors the live href
        "doc-uri:true",
        "same-after-push:true", // accessor, not a stale copied reference
        "search-after-push:?tab=2",
        "url-after-push:true",
        "assign-navigates:/next?step=3", // assignment is navigation
    ] {
        assert!(
            got.contains(claim),
            "G_DOCUMENT_LOCATION: expected `{claim}`\n  got: {got}\n\n  \
             `document.location` must be the SAME live Location as `window.location` (an accessor \
             that tracks __applyUrl's wholesale object swap), `document.URL`/`documentURI` must \
             read the live href, and assigning document.location must navigate."
        );
    }
}
