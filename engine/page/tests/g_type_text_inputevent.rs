//! RG.2.S1.a.a.b — typed characters reach a masked/sectioned field (MUI date field) the way Chrome
//! delivers them: `beforeinput`/`input` are real `InputEvent`s with `inputType: insertText` and the
//! typed `data`, and the caret sits at the end of the typed text. A field that keys off those (and
//! rewrites to its placeholder otherwise) keeps `11/16/2026`.

use manuk_page::Page;
use manuk_text::FontContext;

const W: f32 = 800.0;

const HTML: &str = r#"<!doctype html><body>
<input id="d" value=""><div id="log"></div>
<script>
  var el = document.getElementById('d'), rows = [];
  ['beforeinput','input'].forEach(function (t) {
    el.addEventListener(t, function (e) {
      rows.push(t + ':' + (e instanceof InputEvent ? 'InputEvent' : 'Object') + ':' + e.inputType + ':' + e.data + ':' + el.selectionStart);
    });
  });
  // masked field: only accepts an insertText with data; anything else wipes to the placeholder.
  el.addEventListener('input', function (e) {
    if (e.inputType !== 'insertText' || e.data == null) el.value = 'MM/DD/YYYY';
  });
  window.__report = function () {
    document.getElementById('log').textContent = 'val=' + el.value + ' rows=' + rows.join('|');
  };
</script></body>"#;

#[test]
fn typed_chars_are_input_events_with_data_and_caret_at_end() {
    let fonts = FontContext::new();
    let mut p = Page::load(HTML, "https://t.test/", &fonts, W);
    let root = p.dom().root();
    let el = manuk_css::query_selector_all(p.dom(), root, "#d")[0];
    assert!(p.type_text(el, "11/", 0.0, &fonts, W));
    p.eval_for_test("window.__report();");
    let log = manuk_css::query_selector_all(p.dom(), root, "#log")[0];
    let out = p.dom().text_content(log);
    println!("TYPE RESULT: {out}");
    assert!(out.starts_with("val=11/ rows="), "{out}");
    assert!(
        out.contains("beforeinput:InputEvent:insertText:1:0|input:InputEvent:insertText:1:1|beforeinput:InputEvent:insertText:1:1|input:InputEvent:insertText:1:2|beforeinput:InputEvent:insertText:/:2|input:InputEvent:insertText:/:3"),
        "{out}"
    );
}
