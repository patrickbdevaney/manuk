//! **G_SCROLL_READS_FRESH_GEOMETRY — the iframe `load`, element `scroll` and `pageswap` rounds read
//! layout of the nodes they just built.**
//!
//! These three host entries ran script with no `ReflowScope`, so a handler that appended a
//! `width:123px` box and read its `offsetWidth` got the pre-handler snapshot (0). Each case below
//! must read 123.
//! (One test per file: SpiderMonkey allows one runtime per process.)

use manuk_text::FontContext;

const BOX_JS: &str =
    "var b=document.createElement('div');b.setAttribute('style','width:123px;height:10px');\
     document.body.appendChild(b);";

fn out_text(page: &manuk_page::Page) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "fixture is missing #out");
    page.dom().text_content(hits[0])
}

#[test]
fn a_scroll_handler_reads_the_width_of_what_it_just_built() {
    let fonts = FontContext::new();
    let html = format!(
        r#"<!doctype html><html><body><div id=s style="height:50px;overflow:auto"><div style="height:500px"></div></div>
        <div id=out>-</div><script>
        document.getElementById('s').addEventListener('scroll', function(){{
            {BOX_JS}
            document.getElementById('out').textContent = document.body.lastChild.offsetWidth;
        }});
        </script></body></html>"#
    );
    let mut p = manuk_page::Page::load(&html, "http://x/", &fonts, 800.0);
    p.eval_for_test("document.getElementById('s').scrollTop = 40;");
    let got = out_text(&p);
    assert_eq!(got, "123", "scroll handler read offsetWidth {got:?}");
}
