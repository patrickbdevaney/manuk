//! **G_IFRAME_READS_FRESH_GEOMETRY — the iframe `load`, element `scroll` and `pageswap` rounds read
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
fn an_iframe_load_handler_reads_the_width_of_what_it_just_built() {
    let fonts = FontContext::new();
    let html = format!(
        r#"<!doctype html><html><body><div id=out>-</div>
        <iframe srcdoc="<p>hi</p>" onload="{BOX_JS}
            document.getElementById('out').textContent = document.body.lastChild.offsetWidth;"></iframe>
        </body></html>"#
    )
    .replace(
        "document.getElementById('out')",
        "document.getElementById(&quot;out&quot;)",
    );
    let p = manuk_page::Page::load(&html, "http://x/", &fonts, 800.0);
    let got = out_text(&p);
    assert_eq!(got, "123", "iframe load handler read offsetWidth {got:?}");
}
