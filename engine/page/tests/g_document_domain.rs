//! **G_DOCUMENT_DOMAIN — `document.domain` was ABSENT, and its absence is a THROW rather than a gap.**
//!
//! Found by the t1406 corpus sweep, not by a spec list. `neutypechic.com`'s bundle does
//! `document.domain.replace(...)`, and the engine answered:
//!
//! ```text
//!   TypeError: can't access property "replace", document.domain is undefined
//! ```
//!
//! ⭐⭐ **A MISSING PROPERTY THAT PAGES READ AS A STRING IS A THROW-CLASS KILLER, NOT A MISSING
//! FEATURE.** `undefined.replace` ends the script, and everything that script was going to render
//! never happens — which is why the board ranks throw-killers first for scorability: a site that
//! does not boot scores zero out of zero, and a zero is the M1 ceiling rather than a point on it.
//!
//! Headless Chrome 145.0.7632.116, `https://danluu.com/`, via CDP `Runtime.evaluate`:
//!
//! ```text
//!   {"type":"string","value":"danluu.com","inDoc":true,"replace":"danluu.com"}
//! ```
//!
//! ⚠ **The SETTER is a deliberate no-op that keeps the getter honest.** The legacy
//! `document.domain = 'example.com'` widening is a same-origin-policy RELAXATION this engine does not
//! implement. Pretending to honour it would be worse than ignoring it, so the value is remembered —
//! which is what the compatibility idiom reads back — and no security consequence follows. Named
//! here rather than half-built in silence.
//!
//! Priced before building (t1367's rule): `document.domain` appears on **2 of 52** freshly-fetched
//! CrUX corpus pages (3.8%). Small, and the cost is nine lines of prelude.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><title>dd</title></head><body>
<div id="out">-</div>
<script>
var r = [];
r.push('type:' + (typeof document.domain));
r.push('value:' + JSON.stringify(document.domain));
r.push('inDoc:' + ('domain' in document));
try { r.push('replace:' + document.domain.replace('dd', 'DD')); }
catch (e) { r.push('replace:THREW ' + e.message); }
document.domain = 'example.test';
r.push('afterSet:' + JSON.stringify(document.domain));
document.getElementById('out').textContent = r.join(' | ');
</script></body></html>"##;

#[test]
fn document_domain_is_a_string_that_a_page_can_call_replace_on() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://dd.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("DOCUMENT.DOMAIN: {got}");

    for claim in [
        // Chrome: {"type":"string", ...} — the type is what decides whether `.replace` throws.
        "type:string",
        // the document's origin HOST, which is what `location.hostname` already knows
        "value:\"dd.test\"",
        "inDoc:true",
        // the exact call that killed neutypechic.com's bundle
        "replace:DD.test",
        // the legacy setter round-trips; the SOP relaxation it asks for is not granted (see above)
        "afterSet:\"example.test\"",
    ] {
        assert!(
            got.contains(claim),
            "G_DOCUMENT_DOMAIN: expected `{claim}`\n  got: {got}\n\n  `document.domain` is the \
             document's origin host, a STRING. It was `undefined`, so a page doing \
             `document.domain.replace(...)` — which is ordinary legacy analytics and ad code — threw \
             and took its whole module down. Chrome-measured on a live https origin via CDP."
        );
    }
}
