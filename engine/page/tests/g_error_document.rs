//! **G_ERROR_DOCUMENT — an HTTP error status is a DOCUMENT, and its body is rendered.**
//!
//! A `404`, a `403` bot-wall challenge, a `429` rate-limit notice and a `500` stack trace all arrive
//! with a real HTML body, and that body is *the whole point*: it is what the site chose to say. Every
//! browser renders it. This engine used to `bail!` on `status >= 400` in **both** top-level navigation
//! paths (`manuk_net::fetch_document_or_download` for the shell, `manuk_page::fetch_html_with_headers`
//! for iframes/agent/instrument), turning "the server answered" into "the network broke" and leaving a
//! blank tab where Chrome shows the page.
//!
//! ## Why this is a daily-driver gap and not a conformance detail
//!
//! Measured, on the certification pilot's own corpus (t606, 20 HEAD sites of `corpus-v2.tsv`):
//! **5 of 20 answer `403`** with a ~5.5KB Cloudflare-style challenge page — `tamildhool.tech`,
//! `mangago.me`, `supjav.com`, `fdown.net`, `quora.com`. Those five were the bulk of the pilot's
//! "fetch failed, skipping" column. The certificate could not score them not because the engine
//! renders them badly, but because it **declined to look**. A quarter of the head of the real web
//! was invisible to the Phase-0 exit measurement for a reason that had nothing to do with rendering.
//!
//! ## The repo already knew the rule
//!
//! `page::prefetch_document_post` carries it in a comment — *"a 4xx/5xx still has a body worth
//! showing (the server's 'invalid password' page), so it is rendered rather than turned into an
//! error — matching a real browser"*. So the **POST** navigation rendered error pages while the
//! **GET** navigation refused them: one question, two answers, and the wrong one on the path
//! virtually every navigation takes. That is this project's recurring shape — two implementations of
//! one rule, and the live one goes stale.
//!
//! ## The claims
//!
//! ```text
//! ok          200 + body                → renders   (VACUITY GUARD — see below)
//! notfound    404 + the site's 404 page  → renders
//! blocked     403 + a challenge page     → renders   (the 5-of-20 case)
//! boom        500 + an error page        → renders
//! shell path  403 via fetch_document()   → renders   (the two paths must not disagree)
//! iframe      403 in an <iframe>         → renders   (silent-blank class)
//! refused     nothing listening          → FAILS     (HONESTY FLOOR — see below)
//! ```
//!
//! **The vacuity guard** (`ok`) and **the honesty floor** (`refused`) are not padding, they are what
//! make the other five mean anything. Without `refused`, an engine that simply never reported a
//! network failure would pass every "did it render?" assertion here while being unable to tell a
//! working origin from a dead one — and "renders everything, including nothing" is precisely the
//! shape of the silent failure the rest of this file exists to remove.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;

/// Serve each path with its own **status line** and body. One thread per connection.
fn serve(
    route: impl Fn(&str) -> Option<(u16, &'static str, String)> + Send + Sync + 'static,
) -> String {
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
                let (code, reason, body) = route(&line)
                    .unwrap_or_else(|| (404, "Not Found", "<p>no route</p>".to_string()));
                let resp = format!(
                    "HTTP/1.1 {code} {reason}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes());
                let _ = sock.flush();
            });
        }
    });
    addr
}

/// Each response carries a unique marker, so an assertion names exactly which body came back rather
/// than merely observing that *something* did.
fn body(marker: &str) -> String {
    format!("<!doctype html><html><body><p id=\"m\">BODY{marker}</p></body></html>")
}

#[test]
fn an_http_error_status_is_a_document_and_its_body_renders() {
    let addr = serve(|line| {
        let path = line.split_whitespace().nth(1).unwrap_or("");
        let (code, reason, m) = match path {
            "/ok" => (200u16, "OK", "OK"),
            "/notfound" => (404, "Not Found", "NOTFOUND"),
            // The shape 5 of the pilot's 20 HEAD sites actually return: a challenge interstitial.
            "/blocked" => (403, "Forbidden", "BLOCKED"),
            "/boom" => (500, "Internal Server Error", "BOOM"),
            "/frame" => (403, "Forbidden", "FRAMED"),
            _ => return None,
        };
        Some((code, reason, body(m)))
    });

    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let fonts = manuk_text::FontContext::new();

    // ── The document path used by iframes, the agent and the fidelity instrument.
    let fetch = |p: &str| {
        rt.block_on(manuk_page::fetch_html(&format!("http://{addr}{p}")))
            .map(|(html, _)| html)
    };

    let ok = fetch("/ok").expect("a 200 must fetch — if this fails the whole gate is measuring a broken fixture, not the rule");
    assert!(
        ok.contains("BODYOK"),
        "VACUITY GUARD: an ordinary 200 must still deliver its body. got {ok:?}"
    );

    for (path, marker, what) in [
        (
            "/notfound",
            "BODYNOTFOUND",
            "a 404 — the site's own not-found page, which every browser shows",
        ),
        (
            "/blocked",
            "BODYBLOCKED",
            "a 403 challenge page — the shape 5 of the certification pilot's 20 HEAD sites return",
        ),
        ("/boom", "BODYBOOM", "a 500 — the server's error page"),
    ] {
        let got = fetch(path).unwrap_or_else(|e| {
            panic!(
                "**THE POINT**: {what} must be RENDERED, not turned into a network error. \
                 An error status means the server answered; refusing its body gives the user a blank \
                 tab where every other browser shows the page. fetch_html said: {e:#}"
            )
        });
        assert!(
            got.contains(marker),
            "{what}: the response body must arrive intact. want {marker:?}, got {got:?}"
        );
    }

    // ── The SHELL's navigation path, which is a different function in a different crate. The two
    // must not disagree about what an error response is — a divergence here is exactly how the POST
    // path came to render error pages while the GET path refused them.
    let loaded = rt
        .block_on(manuk_page::fetch_document(&format!("http://{addr}/blocked")))
        .expect(
            "the SHELL navigation path (manuk_net::fetch_document_or_download) must also treat a 403 \
             as a document. If only one of the two paths was fixed, the browser renders the challenge \
             page in an iframe and a blank tab at the top level — which is worse than both being wrong",
        );
    match loaded {
        manuk_page::Loaded::Document { html, .. } => assert!(
            html.contains("BODYBLOCKED"),
            "the shell path must carry the error body through. got {html:?}"
        ),
        _ => panic!("a 403 text/html response is a DOCUMENT, not a download or a prefetch"),
    }

    // ── An error document inside an <iframe>. This is the silent-blank case: a framed OAuth consent
    // screen or 3DS challenge that answers 403 used to render as *nothing at all* inside an
    // otherwise-working page, with no error anywhere.
    let outer = format!(
        "<!doctype html><html><body><iframe src=\"http://{addr}/frame\"></iframe></body></html>"
    );
    let outer_url = format!("http://{addr}/");
    let page = rt.block_on(manuk_page::Page::load_async(
        &outer, &outer_url, &fonts, 800.0,
    ));
    let framed = page.rendered_iframe_urls().join(",");
    assert!(
        framed.contains("/frame"),
        "an <iframe> whose document answers 403 must still render that document — a consent screen \
         or 3DS challenge that 403s currently vanishes with no error anywhere, which is the silent \
         failure shape. rendered frames: {framed:?}"
    );

    // ── THE HONESTY FLOOR. Bind a port, learn it, drop the listener: nothing is listening, so the
    // connection is REFUSED. That is a genuinely different fact from "the server said 404", and the
    // engine must still be able to say so. Without this claim, an engine that never reported a
    // network failure at all would pass every assertion above.
    let dead = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let a = l.local_addr().unwrap().to_string();
        drop(l);
        a
    };
    let refused = rt.block_on(manuk_page::fetch_html(&format!("http://{dead}/")));
    assert!(
        refused.is_err(),
        "a REFUSED connection must still fail loudly. 'an error status is a document' must not \
         degrade into 'nothing ever fails' — a dead origin and a 404 are different facts and the \
         browser has to keep telling them apart"
    );
}
