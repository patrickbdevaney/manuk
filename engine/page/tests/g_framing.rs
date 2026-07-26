//! **G_FRAMING — a site that says "do not frame me" is not framed.**
//!
//! `X-Frame-Options` and CSP `frame-ancestors` are the only clickjacking defence a page has: they
//! are how a bank says its transfer form may not be loaded invisibly on top of someone's game.
//! Surface audit #33 found this row carried as `unknown`; t599 measured it **absent** — and found
//! that `engine/net/src/csp.rs`'s own header had *already documented* `frame-ancestors` as
//! unimplemented while the map said nobody knew.
//!
//! Ignoring the headers is **silent**: the frame renders, the user interacts with it believing it is
//! the outer site, and nothing anywhere reports that a stated policy was overruled. It matters more
//! in this browser than in an ordinary one, because an agentic browser composes pages it did not
//! author and then *acts inside them*.
//!
//! ## The one thing that is easy to get backwards
//!
//! **`frame-ancestors` OVERRIDES `X-Frame-Options` entirely** (CSP3 §7.4.1) — including overriding a
//! `DENY`. Checking both and taking the stricter answer is the intuitive implementation and it is
//! wrong: a site migrating from the legacy header to the modern directive deliberately leaves a
//! stale `X-Frame-Options: DENY` behind, and honouring it would break framing the site has just
//! decided to allow. Claim `override` is that case, and it is the one an implementation written from
//! intuition fails.
//!
//! ## The claims
//!
//! ```text
//! plain      no headers                                   → frames    (vacuity guard + the default)
//! deny       X-Frame-Options: DENY                         → BLOCKED
//! sameorigin X-Frame-Options: SAMEORIGIN, other origin     → BLOCKED
//! none       CSP frame-ancestors 'none'                    → BLOCKED
//! star       CSP frame-ancestors *                         → frames
//! override   frame-ancestors * + X-Frame-Options: DENY     → frames   (the directive wins)
//! garbage    X-Frame-Options: WHATEVER                      → frames   (unknown value is ignored,
//!                                                                       not treated as DENY)
//! ```
//!
//! `plain` is a **vacuity guard**: without it, an implementation that blocked every frame would pass
//! every "was it blocked?" assertion while making the browser useless — the same safe-looking,
//! wrong shape the SRI gate guards against in its first claim.
//!
//! `garbage` is the mirror: a header value the spec does not define must be **ignored**, so a typo
//! cannot silently un-frame a working embed.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;

/// Serve each path with its own header block. One thread per connection.
fn serve(route: impl Fn(&str) -> Option<(String, String)> + Send + Sync + 'static) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let route = Arc::new(route);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut sock) = stream else { continue };
            let route = route.clone();
            std::thread::spawn(move || {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = req.lines().next().unwrap_or("").to_string();
                let (headers, body) =
                    route(&line).unwrap_or_else(|| (String::new(), "not found".to_string()));
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n{headers}Connection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes());
                let _ = sock.flush();
            });
        }
    });
    addr
}

/// Each framed document carries a unique marker in its body, so the outer page's rendered text names
/// exactly which frames were allowed through.
fn framed_body(marker: &str) -> String {
    format!("<!doctype html><html><body><p>FRAME{marker}</p></body></html>")
}

#[test]
fn a_site_that_refuses_framing_is_not_framed() {
    let addr = serve(|line| {
        let path = line.split_whitespace().nth(1).unwrap_or("");
        let (h, m) = match path {
            "/plain" => (String::new(), "PLAIN"),
            "/deny" => ("X-Frame-Options: DENY\r\n".to_string(), "DENY"),
            "/sameorigin" => ("X-Frame-Options: SAMEORIGIN\r\n".to_string(), "SAME"),
            "/none" => (
                "Content-Security-Policy: frame-ancestors 'none'\r\n".to_string(),
                "NONE",
            ),
            "/star" => (
                "Content-Security-Policy: frame-ancestors *\r\n".to_string(),
                "STAR",
            ),
            // The directive must WIN over the legacy header, even when that header says DENY.
            "/override" => (
                "Content-Security-Policy: frame-ancestors *\r\nX-Frame-Options: DENY\r\n"
                    .to_string(),
                "OVERRIDE",
            ),
            "/garbage" => ("X-Frame-Options: WHATEVER\r\n".to_string(), "GARBAGE"),
            _ => return None,
        };
        Some((h, framed_body(m)))
    });

    // The embedder is served from a DIFFERENT port, so it is a different origin: that is what makes
    // the `SAMEORIGIN` claim meaningful rather than trivially satisfied.
    let frames = [
        "plain",
        "deny",
        "sameorigin",
        "none",
        "star",
        "override",
        "garbage",
    ];
    let iframes: String = frames
        .iter()
        .map(|p| format!(r#"<iframe src="http://{addr}/{p}"></iframe>"#))
        .collect();
    let page = format!("<!doctype html><html><body>{iframes}</body></html>");
    let doc_addr = serve(move |line| {
        let path = line.split_whitespace().nth(1).unwrap_or("");
        if path == "/" {
            Some((String::new(), page.clone()))
        } else {
            None
        }
    });

    let fonts = manuk_text::FontContext::new();
    // `load_async` is the path that actually FETCHES subframes — `from_prefetched` builds the
    // document without them, so a gate written on it would have measured nothing and passed the
    // "was it blocked?" claims for the emptiest possible reason.
    let rt = tokio::runtime::Runtime::new()
        .expect("a runtime is required: the framing check happens during the async subframe pass");
    let doc_url = format!("http://{doc_addr}/");
    let (html, final_url) = rt
        .block_on(manuk_page::fetch_html(&doc_url))
        .expect("the outer document must fetch");
    let mut page = rt.block_on(manuk_page::Page::load_async(
        &html, &final_url, &fonts, 800.0,
    ));
    // **The observable is which frames RENDERED.** A refused frame never reaches `render_iframe`,
    // so it is simply absent. Asserting on the outer document's text would have measured nothing —
    // a frame's content lives in its own document, reachable only through `contentDocument`.
    let text = page.rendered_iframe_urls().join(",");
    println!("FRAMING: rendered frames = {text:?}");

    assert!(
        text.contains("/plain"),
        "a document with NO framing headers must frame normally. Without this claim an \
         implementation that blocked every frame would pass every 'was it blocked?' assertion below \
         while making the browser useless. got {text:?}"
    );
    assert!(
        !text.contains("/deny"),
        "**THE POINT**: `X-Frame-Options: DENY` must not be framed. Rendering it anyway is silent — \
         the user interacts with the frame believing it is the outer site, and nothing reports that \
         a stated policy was overruled. got {text:?}"
    );
    assert!(
        !text.contains("/sameorigin"),
        "`X-Frame-Options: SAMEORIGIN` from a DIFFERENT origin must be blocked (the embedder here is \
         on another port, so it is another origin). got {text:?}"
    );
    assert!(
        !text.contains("/none"),
        "CSP `frame-ancestors 'none'` must be blocked. got {text:?}"
    );
    assert!(
        text.contains("/star"),
        "CSP `frame-ancestors *` explicitly ALLOWS framing and must not be blocked — a security \
         check that ignores the permissive form is just a broken browser. got {text:?}"
    );
    assert!(
        text.contains("/override"),
        "**THE ONE WRITTEN FROM INTUITION GETS WRONG**: `frame-ancestors` OVERRIDES \
         `X-Frame-Options` entirely (CSP3 §7.4.1), including a DENY. Taking the stricter of the two \
         breaks every site that migrated to the directive and left the legacy header behind — which \
         is the normal migration path. got {text:?}"
    );
    assert!(
        text.contains("/garbage"),
        "an UNRECOGNISED `X-Frame-Options` value must be ignored, not treated as DENY — otherwise a \
         typo silently un-frames a working embed. got {text:?}"
    );
}
