//! **G_ONSUBMIT_CANCEL — W4.D3.a.a.a: an inline `onsubmit="...;return false"` runs and cancels.**
//!
//! The agent's `submit` (and MCP `type(submit)`) went straight to the native navigation without
//! firing the `submit` event, so a page whose submit script does `fetch('/ping'); return false`
//! never ran its script and the page reloaded as a GET instead (found on the EIA page: manuk
//! utility 0/120).
//!
//! Mutants that must turn this red:
//!   1. skip the `submit` event in `submit_form_at`          → no /ping, URL changes
//!   2. ignore the dispatch result (always proceed)          → URL changes to /?q=...

use manuk_agent::AgentBrowser;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

const PAGE: &str = r#"<!doctype html><html><body>
<form action="/landing" onsubmit="fetch('/ping?v='+document.getElementById('q').value);return false">
<input id="q" name="q" type="text" aria-label="Query"><button type="submit">Go</button></form>
</body></html>"#;

fn serve(log: Arc<Mutex<Vec<String>>>) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let mut tmp = [0u8; 8192];
            let n = s.read(&mut tmp).unwrap_or(0);
            let line = String::from_utf8_lossy(&tmp[..n])
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            log.lock().unwrap().push(line.clone());
            let html = if line.contains("/landing") {
                "<html><body><h1>Landed</h1></body></html>"
            } else if line.contains("/ping") {
                "ok"
            } else {
                PAGE
            };
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}", html.len()).as_bytes());
        }
    });
    format!("http://{addr}")
}

fn run(click: bool) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let base = serve(log.clone());
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut b = AgentBrowser::new(1024, 768);
        b.navigate(&format!("{base}/")).await.expect("navigate");
        b.type_into("Query", "eia").unwrap();
        if click {
            b.click_by_name(&manuk_a11y::Role::Button, "Go")
                .await
                .expect("click");
        } else {
            b.submit(None).await.expect("submit");
        }
        b.settle().await;
        let url = b.current_url().unwrap().to_string();
        assert!(
            !url.contains("landing") && !url.contains("q="),
            "page navigated: {url}"
        );
    });
    let log = log.lock().unwrap();
    assert!(
        log.iter().any(|l| l.contains("/ping?v=eia")),
        "the onsubmit script's fetch never reached the server: {log:?}"
    );
    assert!(
        log.iter().all(|l| !l.contains("/landing")),
        "native submission happened: {log:?}"
    );
}

// One test: SpiderMonkey runtimes must not be created concurrently in one process.
#[test]
fn submit_and_click_run_onsubmit_and_honour_return_false() {
    run(false);
    run(true);
}
