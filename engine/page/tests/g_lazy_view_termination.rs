//! Gate for lazy_view termination: a script cut off by the script deadline while reading
//! `el.style` must END, not see `style === null` and carry on. Red if `lazy_view` maps a failed
//! maker call back to `NullValue()`.
use manuk_text::FontContext;

const PAGE: &str = r##"<!doctype html><html><body>
<div id="t">x</div><div id="out">DID-NOT-FINISH</div>
<script>
  var el = document.getElementById('t'), seen = 'none';
  var t = Date.now();
  while (Date.now() - t < 6000) {
    var s = document.createElement('div').style;
    if (s === null) { seen = 'NULL-STYLE'; break; }
    document.createElement('p').classList;
  }
  document.getElementById('out').textContent = seen === 'NULL-STYLE' ? 'NULL-STYLE' : 'LOOP-ENDED';
</script></body></html>"##;

#[test]
fn a_deadline_cut_inside_style_access_terminates_rather_than_yielding_null() {
    let fonts = FontContext::new();
    std::env::set_var("MANUK_MAX_DRAIN_MS", "400");
    let page = manuk_page::Page::load(PAGE, "https://lv.test/", &fonts, 800.0);
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    let out = page.dom().text_content(hits[0]);
    assert_eq!(
        out, "DID-NOT-FINISH",
        "terminated script must not run its tail (got {out}); NULL-STYLE means lazy_view returned null"
    );
}
