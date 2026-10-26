//! **G_AGENT_FORM_POST — R2.1.1: the agent submits `method=post` forms.**
//!
//! `submit_form` used to fail every POST form with "form method=post is not implemented", which
//! sank 15 of 33 failed rows of the R1.7 table (every login, checkout and wizard step).
//!
//! Mutations that must turn this red:
//!   1. make `submit_form` return `PostUnsupported` again   → the submit errors, state stays logged out
//!   2. serialise the fields into the URL instead of a body → the "no body in URL" assertion fails

use manuk_agent::AgentBrowser;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

const LOGIN: &str = r#"<!doctype html><html><body><form method="post" action="/login">
<input name="user" type="text" aria-label="User"><input name="pass" type="text" aria-label="Pass">
<button type="submit">Sign in</button></form></body></html>"#;

fn serve(log: Arc<Mutex<Vec<(String, String)>>>) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            let (head_end, len) = loop {
                let n = s.read(&mut tmp).unwrap();
                buf.extend_from_slice(&tmp[..n]);
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..i]).to_ascii_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    break (i + 4, len);
                }
                if n == 0 {
                    break (buf.len(), 0);
                }
            };
            while buf.len() < head_end + len {
                let n = s.read(&mut tmp).unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            let line = String::from_utf8_lossy(&buf)
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
            log.lock().unwrap().push((line.clone(), body.clone()));
            let resp = if line.starts_with("POST /login") {
                let loc = if body.contains("user=alice") && body.contains("pass=wonderland") {
                    "/home"
                } else {
                    "/denied"
                };
                format!("HTTP/1.1 302 Found\r\nLocation: {loc}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            } else {
                let html = if line.starts_with("GET /home") {
                    "<html><body><h1>Welcome alice</h1></body></html>"
                } else if line.starts_with("GET /denied") {
                    "<html><body><h1>Denied</h1></body></html>"
                } else {
                    LOGIN
                };
                format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}", html.len())
            };
            let _ = s.write_all(resp.as_bytes());
        }
    });
    format!("http://{addr}")
}

#[test]
fn the_agent_posts_a_login_form_and_follows_the_redirect() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let base = serve(log.clone());
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut b = AgentBrowser::new(1024, 768);
        b.navigate(&format!("{base}/login"))
            .await
            .expect("navigate");
        b.type_into("User", "alice").unwrap();
        b.type_into("Pass", "wonderland").unwrap();
        b.submit(None).await.expect("POST submit");
        let url = b.current_url().unwrap().to_string();
        assert!(
            url.ends_with("/home"),
            "did not land on /home after the 302: {url}"
        );
        let tree = b.a11y_tree().unwrap();
        let (bag, _) = manuk_agent::a11y_score::manuk_bag(&tree);
        assert!(
            bag.iter().any(|(_, n)| n.contains("Welcome alice")),
            "logged-in page not shown: {bag:?}"
        );
    });
    let log = log.lock().unwrap();
    let post = log
        .iter()
        .find(|(l, _)| l.starts_with("POST"))
        .expect("a POST was sent");
    assert_eq!(post.1, "user=alice&pass=wonderland", "body: {post:?}");
    assert!(
        log.iter()
            .all(|(l, _)| !l.contains("wonderland") && !l.contains("user=")),
        "a field value leaked into a request URL: {log:?}"
    );
}
