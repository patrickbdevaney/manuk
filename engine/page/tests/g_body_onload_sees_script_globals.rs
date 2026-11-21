//! **G_BODY_ONLOAD_SEES_SCRIPT_GLOBALS — `<body onload="init()">` runs ONCE, AFTER the page's scripts,
//! even when the page has `<script src>` elements.** (W6.J3.a.b: the WPT encoding decode/encode files
//! bootstrap this way; a window `load` handler that ran at each external script's own `load` event
//! saw no page globals and zeroed the whole area.)
use manuk_text::FontContext;
use std::io::{Read, Write};
use std::net::TcpListener;

fn origin() -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let _ = s.read(&mut buf);
                let body = "window.EXT=1;\n";
                let _ = s.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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
fn body_onload_runs_once_after_scripts_despite_external_scripts() {
    let fonts = FontContext::new();
    let o = origin();
    let html = format!(
        r##"<!doctype html><html><head><script>window.L=[]; function init(){{ L.push('init:'+typeof EXT); document.getElementById('out').textContent=L.join(','); }}</script></head>
            <body onload="init()"><div id="out">-</div><script src="{o}/a.js"></script><script src="{o}/b.js"></script></body></html>"##
    );
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(async {
        let mut p =
            manuk_page::Page::load_async(&html, &format!("{o}/index.html"), &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    assert_eq!(
        page.dom().text_content(n),
        "init:number",
        "the handler must run exactly once, after the external scripts ran and the inline script defined init"
    );
}
