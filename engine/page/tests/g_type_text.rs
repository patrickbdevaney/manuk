//! W4.A1 — `Page::type_text` types like a user: focus, per-character keydown/beforeinput/input/keyup,
//! `change` on commit. A React-style controlled field (state updated only from `input`, written back
//! on render) keeps the text; a `beforeinput` veto drops that character.

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
<input id="p"><input id="v"><div id="log"></div>
<script>
  var state = '', evs = [];
  var el = document.getElementById('p');
  function render() { if (el.value !== state) el.value = state; }
  ['keydown','beforeinput','input','keyup','change'].forEach(function (t) {
    el.addEventListener(t, function () { evs.push({keydown:'kd',beforeinput:'bi',input:'in',keyup:'ku',change:'ch'}[t]); });
  });
  el.addEventListener('input', function (e) { state = e.target.value; render(); });
  var v = document.getElementById('v');
  v.addEventListener('beforeinput', function (e) { if (v.value.length >= 2) e.preventDefault(); });
  window.__report = function () {
    render();
    document.getElementById('log').textContent =
      'state=' + state + ' val=' + el.value + ' focus=' + (document.activeElement === el) +
      ' evs=' + evs.join(',') + ' v=' + v.value;
  };
</script></body>"#;

#[test]
fn typing_reaches_a_controlled_field_with_the_full_event_sequence() {
    let fonts = FontContext::new();
    let mut p = Page::load(HTML, "https://t.test/", &fonts, W);
    let el = node(&p, "#p");
    assert!(p.type_text(el, "ab", 0.0, &fonts, W));
    p.eval_for_test("window.__report();");
    let out = p.dom().text_content(node(&p, "#log"));
    println!("TYPE RESULT: {out}");
    assert!(out.contains("state=ab val=ab focus=true"), "{out}");
    assert!(
        out.contains("evs=kd,bi,in,ku,kd,bi,in,ku,ch"),
        "event order: {out}"
    );
    // Retype replaces, announcing the clear so the state follows.
    assert!(p.type_text(el, "z", 0.0, &fonts, W));
    p.eval_for_test("window.__report();");
    let out = p.dom().text_content(node(&p, "#log"));
    assert!(out.contains("state=z val=z"), "{out}");
}
