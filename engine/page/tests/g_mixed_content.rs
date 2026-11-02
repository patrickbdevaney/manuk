//! SEC.DID.12.a mutation record (chrome_ver=145.0.7632.116): `mixed_content_blocks` returning `false`
//! turns this gate RED (the http script runs and is requested); returning `true` turns the http-page
//! control RED.
//!
//! **G_MIXED_CONTENT — SECURITY.md P15: an https page does not load an http `<script src>`.**
//!
//! The script is blocked BEFORE the request (the server log stays empty), for parse-time and
//! runtime-inserted scripts alike. Controls: the same script on an http page runs, and an https
//! page's inline script runs, so a constant answer cannot satisfy the gate.
#![cfg(feature = "spidermonkey")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

fn serve(log: Arc<Mutex<Vec<String>>>) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let log = log.clone();
            std::thread::spawn(move || {
                let mut b = vec![0u8; 4096];
                let n = s.read(&mut b).unwrap_or(0);
                let req = String::from_utf8_lossy(&b[..n]).to_string();
                log.lock()
                    .unwrap()
                    .push(req.lines().next().unwrap_or("").to_string());
                let body = "document.getElementById('out').textContent += 'X';";
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/javascript\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    addr
}

fn run(doc_url: &str, html: &str) -> String {
    let fonts = manuk_text::FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut page = manuk_page::Page::load_async(html, doc_url, &fonts, 800.0).await;
        page.finish_loading(&fonts, 800.0).await;
        let root = page.dom().root();
        let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
        assert!(!hits.is_empty());
        page.dom().text_content(hits[0])
    })
}

#[test]
fn https_page_does_not_load_http_script() {
    let tmp = std::env::temp_dir().join(format!("manuk-mixed-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    std::env::set_var("MANUK_STATE", &tmp);

    let log = Arc::new(Mutex::new(Vec::new()));
    let srv = serve(log.clone());
    let tag = format!(r#"<script src="http://{srv}/a.js"></script>"#);
    let dynamic = format!(
        r#"<script>var s=document.createElement('script');s.src='http://{srv}/b.js';document.body.appendChild(s);</script>"#
    );
    let page = |extra: &str| {
        format!(
            r#"<!doctype html><body><div id="out"></div><script>document.getElementById('out').textContent+='I';</script>{extra}</body>"#
        )
    };

    // https page: static + dynamic http scripts are refused, inline still runs, nothing requested.
    assert_eq!(
        run("https://site.test/", &page(&tag)),
        "I",
        "http <script src> ran on an https page"
    );
    assert_eq!(
        run("https://site.test/", &page(&dynamic)),
        "I",
        "dynamic http script ran on an https page"
    );
    let seen = log.lock().unwrap().clone();
    assert!(
        seen.is_empty(),
        "blocked script must not be requested: {seen:?}"
    );

    // controls: the same markup on an http page runs the script.
    assert_eq!(
        run("http://site.test/", &page(&tag)),
        "IX",
        "control: http page must still load http script"
    );
    assert_eq!(
        run("http://site.test/", &page(&dynamic)),
        "IX",
        "control: dynamic script on http page"
    );
}
