//! **G_SCRIPT_ROUNDS_READ_FRESH_GEOMETRY — every script round reads layout of the nodes it just built.**
//!
//! A geometry read (`offsetWidth`) is answered from the snapshot published before the round unless the
//! round installs a `ReflowScope`, which arms the forced-reflow hook. The fetch-response round got
//! that in R0.4.a.c.a.b.a.a.a.a.a.a.b.b.a.a.aa (`g_live_now_ticker_has_width`), and the other
//! script-round entry points had none: a `keydown`/`input` handler, a `message` event and a
//! runtime-injected `<script src>` all read 0 for an element they had just appended.
//!
//! Each case below appends a `<div>` with `width:123px` from inside the round and reads its
//! `offsetWidth`. Each case reads a box that is `width:123px`, so the right answer is 123; a snapshot read gives 0.
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
fn a_composition_handler_reads_the_width_of_what_it_just_built() {
    let fonts = FontContext::new();
    let html = format!(
        r#"<!doctype html><html><body><input id=i><div id=out>-</div><script>
        document.getElementById('i').addEventListener('input', function(){{
            {BOX_JS}
            document.getElementById('out').textContent = document.body.lastChild.offsetWidth;
        }});
        </script></body></html>"#
    );
    let mut p = manuk_page::Page::load(&html, "http://x/", &fonts, 800.0);
    let root = p.dom().root();
    let i = manuk_css::query_selector_all(p.dom(), root, "#i")[0];
    p.dispatch_composition(i, "x", &fonts, 800.0);
    let got = out_text(&p);
    assert_eq!(
        got, "123",
        "composition-driven input handler read offsetWidth {got:?} of a node it had just appended \
         (the box is 123px wide)"
    );
}
