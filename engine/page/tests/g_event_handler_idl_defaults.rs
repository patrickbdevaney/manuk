//! **G_EVENT_HANDLER_IDL_DEFAULTS — `'oninput' in document` is true, so React uses the native `input` event.**
//!
//! React probes `'oninput' in document` (then `typeof div.oninput` after `setAttribute`) once, at load. Both
//! used to answer no, so React took its IE9 polyfill path and `onChange` never fired for a text input: the
//! typed text stayed in the DOM until the next render put the old state back (opendining's reservation form
//! rejected every phone number under W3.18.a.a.a).

use manuk_page::Page;
use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><body><div id="log"></div><script>
  var r = [];
  ['input', 'change', 'beforeinput', 'keydown', 'click'].forEach(function (n) {
    var en = 'on' + n;
    var d = document.createElement('div');
    r.push(en + ':' + (en in document) + '/' + (en in d) + '/' + (d[en] === null));
  });
  var b = document.createElement('div');
  b.oninput = function () {};
  r.push('assign:' + (typeof b.oninput));
  document.getElementById('log').textContent = r.join(' ');
</script></body>"#;

#[test]
fn event_handler_idl_attributes_exist_and_default_to_null() {
    let fonts = FontContext::new();
    let p = Page::load(HTML, "https://t.test/", &fonts, 800.0);
    let root = p.dom().root();
    let log = manuk_css::query_selector_all(p.dom(), root, "#log")[0];
    let out = p.dom().text_content(log);
    for n in ["input", "change", "beforeinput", "keydown", "click"] {
        assert!(out.contains(&format!("on{n}:true/true/true")), "{out}");
    }
    assert!(out.contains("assign:function"), "{out}");
}
