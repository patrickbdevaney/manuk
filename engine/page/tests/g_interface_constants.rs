//! **G_INTERFACE_CONSTANTS — DOMException legacy codes + Event phase constants.**
//!
//! `e.code === DOMException.NOT_FOUND_ERR` and `e.eventPhase === Event.AT_TARGET` are canonical checks in
//! real code and WPT. The named constants were absent, so the comparison silently ran `=== undefined`.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
  var R = [];
  R.push('nf:' + DOMException.NOT_FOUND_ERR);                         // 8
  R.push('idx:' + DOMException.INDEX_SIZE_ERR);                       // 1
  var de = new DOMException('x', 'NotFoundError');
  R.push('code:' + (de.code === DOMException.NOT_FOUND_ERR));         // true — instance code matches const
  R.push('at:' + Event.AT_TARGET);                                   // 2
  R.push('bub:' + Event.BUBBLING_PHASE);                             // 3
  R.push('cap:' + (new Event('x').CAPTURING_PHASE));                 // 1 — inherited on instances
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn domexception_codes_and_event_phase_constants() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ic.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "nf:8",
            "DOMException.NOT_FOUND_ERR is the legacy code 8, not undefined",
        ),
        ("idx:1", "DOMException.INDEX_SIZE_ERR is 1"),
        (
            "code:true",
            "a thrown DOMException's .code matches the named constant",
        ),
        ("at:2", "Event.AT_TARGET is 2"),
        ("bub:3", "Event.BUBBLING_PHASE is 3"),
        ("cap:1", "Event instances inherit the phase constants"),
    ] {
        assert!(
            got.contains(claim),
            "G_INTERFACE_CONSTANTS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
