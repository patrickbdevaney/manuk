//! **G_CREATED_INLINE_STYLE_REFLOW — a node a handler creates and styles inline measures as styled.**
//!
//! PERF.1.2.a.b reported that `getComputedStyle(d).height` read `0px` after a forced reflow for a div
//! made with `style.height='25px'` in a DOMContentLoaded handler. It does not reproduce: every shape
//! below (rect/computed/offset read first, style before or after append, a second forced reflow in the
//! same round, and the adopted box tree the page paints) reads 25. The original observation was most
//! likely a fixed-height ancestor in the fixture. Kept as a guard for the shape.
//! One page per process (SpiderMonkey's once-only `load` guard is shared by a thread's pages), so all
//! variants run in ONE handler.
//!
//! **How to break it:** make `forced_reflow` return early (`if true || ...` on its idempotence check).
use manuk_text::FontContext;
use std::collections::HashMap;

#[test]
fn created_inline_height_is_measured_every_way() {
    let fonts = FontContext::new();
    let html = r##"<!doctype html><html><body><style>.tall{height:50px}</style>
<div id="host"></div><div id="out"></div>
<script>document.addEventListener('DOMContentLoaded', function () {
  var host = document.getElementById('host'), r = [];
  var a = document.createElement('div'); a.style.height = '25px'; host.appendChild(a);
  r.push(a.getBoundingClientRect().height);
  r.push(getComputedStyle(a).height);
  var b = document.createElement('div'); host.appendChild(b); b.getBoundingClientRect();
  b.style.height = '25px';
  r.push(b.offsetHeight);
  host.className = 'tall'; host.getBoundingClientRect();
  var c = document.createElement('div'); c.id = 'mk'; c.style.height = '25px'; document.body.appendChild(c);
  r.push(c.getBoundingClientRect().height + '/' + getComputedStyle(c).height);
  document.getElementById('out').textContent = r.join(',');
});</script></body></html>"##;
    let mut p = manuk_page::Page::load(html, "https://inl.test/", &fonts, 800.0);
    let out = manuk_css::query_selector_all(p.dom(), p.dom().root(), "#out")[0];
    assert_eq!(p.dom().text_content(out), "25,25px,25,25/25px");
    p.apply_stylesheets(&HashMap::new(), &fonts, 800.0);
    let mk = manuk_css::query_selector_all(p.dom(), p.dom().root(), "#mk")[0];
    let h = p
        .root_box
        .node_rects(p.dom())
        .into_iter()
        .find(|(n, _)| *n == mk)
        .unwrap()
        .1
        .height;
    assert_eq!(h, 25.0, "adopted tree: created div height {h}");
}
