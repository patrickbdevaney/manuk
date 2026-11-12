//! **G_POINTER_EVENTS_CLICK — a trigger that opens on `pointerdown` never opened.**
//!
//! Radix Select (the opendining time and guest dropdowns) opens its listbox in an `onPointerDown`
//! handler and has no `click` listener, so a click that fired only `mousedown`/`mouseup`/`click`
//! left the `Select time` combobox shut: `+0 ~1 -0`, no options, where Chrome reports `+35` options.
//!
//! RED, run: `MANUK_POINTER_MUTATE=off` drops the pointer pair and the order and `menuOpen`
//! assertions fail. The mouse mask, `pointerType` and the primary-button check Radix makes
//! (`e.button === 0 && e.ctrlKey === false`) are asserted too.

use manuk_page::Page;
use manuk_text::FontContext;

const W: f32 = 800.0;

#[test]
fn a_click_fires_pointerdown_before_mousedown_and_pointerup_before_mouseup() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<button id="t" role="combobox">Select time</button><div id="log"></div>
<script>
  var seq = [], menuOpen = false, info = '';
  var t = document.getElementById('t');
  t.addEventListener('pointerdown', function (e) {
    seq.push('pd');
    info = e.pointerType + ':' + e.button + ':' + e.ctrlKey + ':' + e.isPrimary;
    if (e.button === 0 && e.ctrlKey === false && e.pointerType === 'mouse') menuOpen = true;
  });
  t.addEventListener('mousedown', function () { seq.push('md'); });
  t.addEventListener('pointerup',  function () { seq.push('pu'); });
  t.addEventListener('mouseup',   function () { seq.push('mu'); });
  t.addEventListener('click',     function () { seq.push('click'); });
  window.__report = function () {
    document.getElementById('log').textContent =
      'seq=' + seq.join('>') + ' menuOpen=' + menuOpen + ' info=' + info;
  };
</script></body>"#,
        "https://pointer.test/",
        &fonts,
        W,
    );
    let root = p.dom().root();
    let t = manuk_css::query_selector_all(p.dom(), root, "#t")[0];
    let lg = manuk_css::query_selector_all(p.dom(), root, "#log")[0];
    p.dispatch_click(t, &fonts, W);
    p.eval_for_test("window.__report();");
    let out = p.dom().text_content(lg);
    assert!(
        out.contains("seq=pd>md>pu>mu>click"),
        "pointerdown>mousedown>pointerup>mouseup>click; got: {out}"
    );
    assert!(
        out.contains("menuOpen=true") && out.contains("info=mouse:0:false:true"),
        "a pointerdown-only menu must open for a primary mouse press; got: {out}"
    );
}
