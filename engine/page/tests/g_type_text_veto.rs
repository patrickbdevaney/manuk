//! W4.A1 — a `beforeinput` handler that cancels vetoes that character in `Page::type_text`.

use manuk_page::Page;
use manuk_text::FontContext;

const W: f32 = 800.0;

fn node(p: &Page, sel: &str) -> manuk_dom::NodeId {
    let root = p.dom().root();
    manuk_css::query_selector_all(p.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"))
}

const HTML: &str = r#"<!doctype html><body>
<input id="v"><div id="log"></div>
<script>
  var v = document.getElementById('v');
  v.addEventListener('beforeinput', function (e) { if (v.value.length >= 2) e.preventDefault(); });
  window.__report = function () { document.getElementById('log').textContent = 'v=' + v.value; };
</script></body>"#;

#[test]
fn a_cancelled_beforeinput_drops_that_character() {
    let fonts = FontContext::new();
    let mut p = Page::load(HTML, "https://t.test/", &fonts, W);
    let v = node(&p, "#v");
    assert!(p.type_text(v, "abcd", 0.0, &fonts, W));
    p.eval_for_test("window.__report();");
    let out = p.dom().text_content(node(&p, "#log"));
    assert!(out.contains("v=ab"), "{out}");
}
