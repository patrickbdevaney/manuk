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
fn a_message_handler_reads_the_width_of_what_it_just_built() {
    let fonts = FontContext::new();
    let html = format!(
        r#"<!doctype html><html><body><div id=out>-</div><script>
        window.addEventListener('message', function(){{
            {BOX_JS}
            document.getElementById('out').textContent = document.body.lastChild.offsetWidth;
        }});
        </script></body></html>"#
    );
    let mut p = manuk_page::Page::load(&html, "http://x/", &fonts, 800.0);
    p.deliver_message("{}", "https://peer.test", 1, &fonts, 800.0);
    let got = out_text(&p);
    assert_eq!(
        got, "123",
        "message handler read offsetWidth {got:?} of a node it had just appended (the box is 123px wide)"
    );
}

/// Serves one JavaScript body to every connection, the way a CDN serves a chunk.
fn chunk_origin(body: &'static str) -> String {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = s.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            });
        }
    });
    format!("http://{addr}")
}
