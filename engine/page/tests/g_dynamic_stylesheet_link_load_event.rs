//! **G_DYNAMIC_STYLESHEET_LINK_LOAD_EVENT — a `<link rel=stylesheet>` inserted at runtime gets `load`
//! (or `error`) once its fetch settles.** React 19 holds its commit on that event (W6.R1.b.a.a: opendining
//! waited for a 60 s timer). Red when `fire_stylesheet_link_events` is not called.
use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_text::FontContext;

fn origin() -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let (status, body) = if req.contains("/ok.css") {
                    ("200 OK", "body{color:red}")
                } else {
                    ("404 Not Found", "nope")
                };
                let _ = s.write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Type: text/css\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            });
        }
    });
    format!("http://{addr}")
}

#[test]
fn a_runtime_inserted_stylesheet_link_fires_load_or_error() {
    let fonts = FontContext::new();
    let origin = origin();
    let html = format!(
        r#"<!doctype html><html><head></head><body><div id="out">-</div>
           <script>
             window.r = [];
             function mk(name, href) {{
               var l = document.createElement('link');
               l.rel = 'stylesheet'; l.href = href;
               l.onload = function() {{ window.r.push(name + ':load'); document.getElementById('out').textContent = window.r.join(' '); }};
               l.addEventListener('error', function() {{ window.r.push(name + ':error'); document.getElementById('out').textContent = window.r.join(' '); }});
               document.head.appendChild(l);
             }}
             mk('a', "{origin}/ok.css");
             mk('b', "{origin}/missing.css");
           </script></body></html>"#
    );
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut page = rt.block_on(manuk_page::Page::load_async(
        &html,
        &format!("{origin}/index.html"),
        &fonts,
        800.0,
    ));
    rt.block_on(page.fetch_and_run_dynamic_scripts(&fonts, 800.0, 4));
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("DYN-LINK RESULT: {got}");
    assert!(
        got.contains("a:load"),
        "ok sheet must fire load, got {got:?}"
    );
    assert!(
        got.contains("b:error"),
        "404 sheet must fire error, got {got:?}"
    );
    assert_eq!(got.matches("a:load").count(), 1, "exactly once");
}
