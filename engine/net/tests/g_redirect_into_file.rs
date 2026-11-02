//! **G_REDIRECT_INTO_FILE (SEC.DID.2) — a network response cannot redirect the browser into the disk.**
//!
//! A server answers a POSTed form with `302 Location: file:///.../secret`. `post_document` follows
//! redirects as a GET, and without the `scheme == "file"` check at the redirect hop it would read the
//! local file and hand its bytes back as the page. Claims: the POST errors with "blocked" and the
//! canary never appears (A); a GET whose server redirects to the file does not return the canary (B);
//! control: a 302 to a second http URL is followed (vacuity guard).
//!
//! Mutation (chrome_ver=145.0.7632.116): deleting the `if next.scheme() == "file" { bail!(..) }`
//! block in `post_document` turns claim A RED.

use std::io::{Read, Write};
use std::net::TcpListener;

fn redirector(location: String) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { break };
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let first = String::from_utf8_lossy(&buf[..n])
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            let resp = if first.contains("/final") {
                let b = "FINAL-OK";
                format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len())
            } else {
                format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            };
            let _ = s.write_all(resp.as_bytes());
        }
    });
    port
}

#[test]
fn a_network_redirect_into_a_local_file_reads_nothing() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let dir = std::env::temp_dir().join(format!("manuk-redir-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let secret = dir.join("secret.txt");
    std::fs::write(&secret, "REDIRECT-CANARY").unwrap();
    let file_url = format!("file://{}", secret.display());

    // A: POST -> 302 -> file:
    let port = redirector(file_url.clone());
    let r = rt.block_on(manuk_net::post_document(
        &format!("http://127.0.0.1:{port}/login"),
        "application/x-www-form-urlencoded",
        bytes::Bytes::from_static(b"a=1"),
        None,
    ));
    match &r {
        Err(e) => assert!(format!("{e:#}").contains("blocked"), "wrong error: {e:#}"),
        Ok(resp) => panic!(
            "POST redirect into file: was followed; body = {:?}",
            String::from_utf8_lossy(&resp.body)
        ),
    }

    // B: GET -> 302 -> file: must not yield the canary.
    let port = redirector(file_url);
    let g = rt.block_on(manuk_net::fetch(&format!("http://127.0.0.1:{port}/x")));
    if let Ok(resp) = g {
        assert!(
            !String::from_utf8_lossy(&resp.body).contains("REDIRECT-CANARY"),
            "GET redirect into file: returned the file"
        );
    }

    // Control: POST -> 302 -> http /final is followed.
    let port = redirector(String::new());
    let port2 = redirector(format!("http://127.0.0.1:{port}/final"));
    let ok = rt
        .block_on(manuk_net::post_document(
            &format!("http://127.0.0.1:{port2}/login"),
            "application/x-www-form-urlencoded",
            bytes::Bytes::from_static(b"a=1"),
            None,
        ))
        .expect("control redirect to http should be followed");
    assert_eq!(&ok.body[..], b"FINAL-OK");
}
