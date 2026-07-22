//! **G_COOKIE_ATTRIBUTES — cookie flags are enforced across the JS ↔ wire boundary.**
//!
//! The constellation carried `cookies (SameSite/Secure/HttpOnly)` as **partial**, with the honest
//! note *"cookie jar exists; flags unmeasured."* The jar in fact enforces every flag
//! (`engine/net/src/cookies.rs` + `storage.rs`, with unit coverage), but no gate proved the property
//! that actually protects a login: that the flags hold **across layers** — the JS `document.cookie`
//! shim, the network Cookie header, and the jar all agreeing. A unit test on the jar passes while a
//! wiring bug leaks an `HttpOnly` session cookie to a `<script>` an ad network injected; that is the
//! composition failure this gate exists to catch.
//!
//! Three cross-layer properties, each daily-driver-critical and each RED-provable:
//!
//!   1. **`HttpOnly` hides from `document.cookie`.** A session cookie the server marks `HttpOnly`
//!      must be invisible to script — this is the single most important XSS mitigation for session
//!      theft. Set via a real `Set-Cookie` header, read back through the JS shim.
//!   2. **…but `HttpOnly` still travels on the wire.** The same cookie MUST ride a subsequent
//!      same-origin request's `Cookie:` header — it is hidden from *script*, not from the *server*.
//!      A jar that dropped it entirely would log the user out on the next request while looking
//!      "more secure."
//!   3. **`__Host-` prefix rejection.** A `document.cookie` write of a `__Host-`-prefixed cookie
//!      that does not meet the prefix's contract (Secure + host-only + `Path=/`) must be REJECTED,
//!      not silently stored — the prefix is a promise the browser enforces so a subdomain cannot
//!      forge a host cookie.
//!
//! Built as an integration gate against a real `TcpListener` (like `g_oauth_redirect`) so the
//! `Set-Cookie` genuinely crosses the network boundary and the follow-up `fetch` genuinely puts a
//! `Cookie:` header on the wire.
//!
//! RED, run: flipping the `document.cookie` read predicate in `engine/net/src/lib.rs`
//! (`|c| !c.http_only` → `|_| true`) leaks `ho=secret` into `document.cookie` and fails property 1;
//! disabling the `__Host-` check in `cookies.rs` stores `__Host-bad` and fails property 3.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use manuk_text::FontContext;

/// The page served at `/page`. Its inline script writes cookies through the JS shim, then reports
/// what `document.cookie` returns, then issues a same-origin `fetch` so the server can observe which
/// cookies rode the wire.
const PAGE_HTML: &str = r##"<!doctype html>
<html><body>
  <div id="out">none</div>
  <script>
    // A plain cookie the script is allowed to set and read.
    document.cookie = 'jsset=1; Path=/';
    // A __Host- cookie with NONE of the required attributes (no Secure, http origin): must be rejected.
    document.cookie = '__Host-bad=nope; Path=/';
    document.getElementById('out').textContent = document.cookie;
    // Now let the server see what actually ships on a request.
    fetch('/whoami');
  </script>
</body></html>"##;

/// One-connection-per-request test server, identical in shape to `g_oauth_redirect::serve`: it logs
/// every raw request (so the `Cookie:` header the page sent is inspectable) and answers via `route`.
fn serve(
    label: &'static str,
    log: Arc<Mutex<Vec<String>>>,
    route: impl Fn(&str, &str) -> String + Send + Sync + 'static,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let route = Arc::new(route);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut sock) = stream else { continue };
            let (log, route) = (log.clone(), route.clone());
            std::thread::spawn(move || {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = req.lines().next().unwrap_or("").to_string();
                log.lock().unwrap().push(format!("{label} {req}"));
                let resp = route(&line, &req);
                let _ = sock.write_all(resp.as_bytes());
                let _ = sock.flush();
            });
        }
    });
    addr
}

#[test]
fn cookie_flags_hold_across_the_js_and_wire_boundaries() {
    // Jar isolation FIRST, before any net call: the cookie jar is process-global and loads from
    // `$MANUK_STATE` on first use, so without this the gate would read and rewrite the developer's
    // real cookie file.
    let tmp = std::env::temp_dir().join(format!("manuk-cookieattr-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let addr = serve("SRV", log.clone(), move |line, _req| {
        if line.starts_with("GET /page") {
            // The document load carries the cookies: one HttpOnly (session-shaped), one plain.
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\
                 Set-Cookie: ho=secret; HttpOnly; Path=/\r\n\
                 Set-Cookie: plain=visible; Path=/\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{PAGE_HTML}",
                PAGE_HTML.len()
            )
        } else {
            // /whoami and anything else: an empty 200 — the point is the REQUEST the page sent, which
            // `serve` has already logged, not the response.
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
                .to_string()
        }
    });

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    // ── 1. Load the page over the network, so the `Set-Cookie` headers genuinely cross the boundary
    //       and land in the jar.
    let url = format!("http://{addr}/page");
    let loaded = rt
        .block_on(manuk_page::fetch_document(&url))
        .expect("the page load must complete");
    let (html, final_url) = match loaded {
        manuk_page::Loaded::Document {
            html, final_url, ..
        } => (html, final_url),
        _ => panic!("the page load did not produce a document"),
    };

    // ── 2. Run the page. The inline script writes cookies through the JS shim and reports
    //       `document.cookie`.
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(&html, &final_url, &fonts, 800.0);

    // ── 3. Pump the page's fetches (the `/whoami` probe) onto the wire, in the shape the shell runs
    //       it — so the server observes the real `Cookie:` header.
    let base = url::Url::parse(&final_url).unwrap();
    for _ in 0..4 {
        let reqs = page.take_fetches();
        if reqs.is_empty() {
            break;
        }
        for (id, raw_url, method, headers, body) in reqs {
            let abs = base.join(&raw_url).expect("resolvable request URL");
            let hdrs: Vec<(&str, &str)> = headers
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let resp = rt.block_on(manuk_net::request_from(
                &method,
                abs.as_str(),
                &hdrs,
                body.into(),
                Some(&final_url),
            ));
            match resp {
                Ok(r) => {
                    let text = r.decoded_text();
                    page.resolve_fetch(id, r.status, &text, &r.headers, &fonts, 800.0);
                }
                Err(_) => page.resolve_fetch(id, 0, "", &[], &fonts, 800.0),
            }
        }
    }

    // ── 4. What `document.cookie` returned — the JS-visible view of the jar.
    let root = page.dom().root();
    let out_node = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let seen_by_js = page.dom().text_content(out_node);

    // Property 1: the HttpOnly cookie is INVISIBLE to script, the plain one is visible.
    assert!(
        seen_by_js.contains("plain=visible"),
        "a non-HttpOnly cookie set by the server must be readable via document.cookie\n  \
         document.cookie = {seen_by_js:?}"
    );
    assert!(
        !seen_by_js.contains("ho=") && !seen_by_js.contains("secret"),
        "an HttpOnly cookie must be HIDDEN from document.cookie — this is THE session-theft XSS \
         mitigation; leaking it here means any injected <script> can exfiltrate the session\n  \
         document.cookie = {seen_by_js:?}"
    );
    // The script's own plain write is visible (proves the write path reached the same jar).
    assert!(
        seen_by_js.contains("jsset=1"),
        "a plain cookie written via document.cookie must be stored and read back\n  \
         document.cookie = {seen_by_js:?}"
    );
    // Property 3: the __Host- write with none of the required attributes was rejected.
    assert!(
        !seen_by_js.contains("__Host-bad"),
        "a __Host--prefixed cookie written without Secure/host-only/Path=/ must be REJECTED, not \
         silently stored — the prefix is a browser-enforced promise\n  document.cookie = {seen_by_js:?}"
    );

    // Property 2: the HttpOnly cookie STILL travels to the server on the follow-up request. It is
    // hidden from script, not from the origin.
    let wire = log.lock().unwrap().join("\n").to_ascii_lowercase();
    assert!(
        wire.contains("get /whoami"),
        "the page's fetch('/whoami') must reach the server\n{wire}"
    );
    let whoami = wire
        .lines()
        .skip_while(|l| !l.contains("get /whoami"))
        .take_while(|l| !l.trim().is_empty() || l.contains("get /whoami"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        whoami.contains("cookie:") && whoami.contains("ho=secret"),
        "the HttpOnly cookie MUST ride the Cookie: header of a same-origin request — it hides from \
         script, not from the wire; a jar that dropped it would silently log the user out\n\
         the /whoami request was:\n{whoami}"
    );
}
