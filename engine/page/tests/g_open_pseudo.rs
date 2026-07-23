//! **G_OPEN_PSEUDO — `:open` matches an open `<details>`/`<dialog>` in querySelector/`matches`.**
//!
//! `:open` (Baseline 2026) styles and selects a disclosure widget in its open state. The shipping CSS
//! cascade already handles it (Stylo's `NonTSPseudoClass::Open`), so `details:open { … }` renders — but
//! the querySelector/`matches` selector engine is SEPARATE (it backs `querySelectorAll`, `matches`,
//! `closest`) and did not know `:open`, so `document.querySelectorAll('details:open')` returned nothing
//! and `el.matches(':open')` was false. A disclosure-widget or accessibility helper that enumerates the
//! open panels (`querySelectorAll('details:open')`) found none, silently.
//!
//! The claims check the JS selector engine, each a way the missing pseudo-class goes RED:
//!
//!   * **`matches(':open')`** is true for an open `<details>`/`<dialog>`, false for a closed one.
//!   * **`querySelectorAll('details:open')`** returns only the open ones.
//!   * **`:open` composes** with a tag/compound and works through `querySelector`.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
    <details id="d1" open><summary>a</summary>1</details>
    <details id="d2"><summary>b</summary>2</details>
    <details id="d3" open><summary>c</summary>3</details>
    <dialog id="dlg" open>hi</dialog>
    <div id="out">-</div><script>
    var r = [];
    r.push('d1:' + document.getElementById('d1').matches(':open'));      // open → true
    r.push('d2:' + document.getElementById('d2').matches(':open'));      // closed → false
    r.push('dlg:' + document.getElementById('dlg').matches(':open'));    // open dialog → true
    var open = document.querySelectorAll('details:open');
    r.push('qsa:' + Array.prototype.map.call(open, function (e) { return e.id; }).join(','));
    r.push('qs:' + (document.querySelector('details:open') || {}).id);   // first open details
    document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn open_pseudo_matches_open_disclosure_widgets() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://open-pseudo.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "d1:true",   // an open <details> matches :open
        "d2:false",  // a closed <details> does not
        "dlg:true",  // an open <dialog> matches too
        "qsa:d1,d3", // querySelectorAll('details:open') returns only the open ones, in order
        "qs:d1",     // querySelector finds the first open details
    ] {
        assert!(
            got.contains(claim),
            "G_OPEN_PSEUDO: expected {claim} in {got:?}\n  \
             `:open` must match an open <details>/<dialog> in the querySelector/matches engine, not \
             only in the style cascade — else `querySelectorAll('details:open')` silently returns none."
        );
    }
}
