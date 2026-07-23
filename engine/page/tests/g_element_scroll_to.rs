//! **G_ELEMENT_SCROLL_TO — `element.scrollTo`/`scrollBy` programmatically scroll a container.**
//!
//! Programmatic scrolling is everywhere: a "scroll to top" button (`el.scrollTo(0, 0)`), a chat pane
//! pinning to the bottom (`el.scrollTo(0, el.scrollHeight)`), a virtualised list jumping to an index, a
//! carousel's prev/next, `el.scrollTo({ top, behavior: 'smooth' })`. `element.scrollTop = n` worked, but
//! `scrollTo`/`scrollBy` were absent — so `el.scrollTo is not a function` threw and the control did
//! nothing, silently. They reuse the native `scrollLeft`/`scrollTop` setters, so they inherit CLAMPING
//! (to the scrollable range) and scroll-snap.
//!
//! The claims read the scroll position back, each a way the missing methods go RED:
//!
//!   * **`scrollTo(x, y)`** sets both axes; a same-line read agrees.
//!   * **`scrollTo({ left, top })`** — the options-object form, including a partial (one-axis) object.
//!   * **`scrollBy(dx, dy)`** is relative to the current position.
//!   * **`behavior: 'smooth'`** is accepted (the final position is correct; we jump).
//!   * **A huge target CLAMPS** to the scrollable maximum (inherited from the setter).

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
    <div id="s" style="overflow:auto;height:40px;width:60px"><div style="height:400px;width:600px">tall wide</div></div>
    <div id="out">-</div><script>
    var r = [];
    var s = document.getElementById('s');
    s.scrollTo(30, 100); r.push('xy:' + s.scrollLeft + ',' + s.scrollTop);
    s.scrollTo({ left: 10, top: 50 }); r.push('opts:' + s.scrollLeft + ',' + s.scrollTop);
    s.scrollTo(0, 0); s.scrollTo({ top: 80 }); r.push('partial:' + s.scrollLeft + ',' + s.scrollTop);
    s.scrollTo(0, 20); s.scrollBy(5, 30); r.push('by:' + s.scrollLeft + ',' + s.scrollTop);
    s.scrollTo({ top: 60, behavior: 'smooth' }); r.push('smooth:' + s.scrollTop);
    s.scrollTo(0, 99999); r.push('clamp:' + (s.scrollTop > 0 && s.scrollTop <= 360 ? 'ok' : s.scrollTop));
    document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn element_scroll_to_and_by_move_the_container() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://element-scroll-to.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "xy:30,100",    // scrollTo(x, y) sets both axes
        "opts:10,50",   // the { left, top } options form
        "partial:0,80", // a partial options object touches only its axis (left stays 0)
        "by:5,50",      // scrollBy is relative: (0,20) + (5,30) = (5,50)
        "smooth:60",    // behavior:'smooth' accepted, final position correct
        "clamp:ok",     // a huge target clamps to the scrollable maximum
    ] {
        assert!(
            got.contains(claim),
            "G_ELEMENT_SCROLL_TO: expected {claim} in {got:?}\n  \
             element.scrollTo/scrollBy must scroll the container — their absence throws \
             `not a function` and a scroll-to-top / chat-pin / carousel control silently does nothing."
        );
    }
}
