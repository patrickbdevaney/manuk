//! **G_AGENT_FOLLOWS_SCRIPT_NAVIGATION — a click whose handler ends in `location.href = url` goes there.**
//! (RG.E2.b)
//!
//! `Page::take_script_navigation` records `location.href =`, `location.assign` and `window.location =`;
//! only the GUI shell performed them. `AgentBrowser::activate` ignored it, so staynb's "Reserve stay"
//! (Next's router hard-navigates to /book) reported NO EFFECT / "0 click listeners" under manuk while
//! Chrome opened the booking page.
//!
//! Red by the mutation: delete the `take_script_navigation` block in `AgentBrowser::activate`
//! (the click then returns `Inert` and the URL never changes). The control arm (a handler that only
//! mutates the DOM) must stay on its page.
use manuk_agent::{Activation, AgentBrowser};
use std::io::{Read, Write};
use std::net::TcpListener;

const HOME: &str = r#"<!doctype html><html><body><h1>Home</h1>
<button onclick="location.href='/book?n=1'">Reserve</button>
<button onclick="document.getElementById('o').textContent='changed'">Local</button><p id="o">idle</p>
</body></html>"#;

fn serve() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let line = String::from_utf8_lossy(&buf[..n])
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            let html = if line.starts_with("GET /book") {
                "<html><body><h1>Confirm and pay</h1></body></html>"
            } else {
                HOME
            };
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}", html.len()).as_bytes());
        }
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn a_click_that_assigns_location_navigates() {
    let base = serve();
    let mut b = AgentBrowser::new(800, 600);
    b.navigate(&format!("{base}/")).await.unwrap();

    // CONTROL: a handler that only edits the DOM stays on the page (a navigation on every click would pass the arm below).
    b.click_by_name(&manuk_a11y::Role::Button, "Local")
        .await
        .unwrap();
    assert!(
        b.current_url().unwrap().ends_with('/'),
        "CONTROL: stayed on the page, got {}",
        b.current_url().unwrap()
    );

    let act = b
        .click_by_name(&manuk_a11y::Role::Button, "Reserve")
        .await
        .unwrap();
    assert!(
        matches!(&act, Activation::Navigated(u) if u.ends_with("/book?n=1")),
        "the click must report the navigation, got {act:?}"
    );
    assert!(
        b.current_url().unwrap().ends_with("/book?n=1"),
        "the browser must be on /book, is on {}",
        b.current_url().unwrap()
    );
    assert!(
        b.observe().unwrap().text.contains("Confirm and pay"),
        "the new page must be the one observed"
    );
}
