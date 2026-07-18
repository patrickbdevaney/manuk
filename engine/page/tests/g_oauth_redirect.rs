//! **G_OAUTH_REDIRECT — "Sign in with…" end to end, against two real servers.**
//!
//! The constellation carried `OAuth redirect flow` as **unknown**, with the honest cost written
//! beside it: *you cannot log in to the modern web*. Almost nothing a daily driver needs is reachable
//! without it — GitHub, Google, every SaaS dashboard, every "continue with" button.
//!
//! **Why this had to be an integration gate and not a unit test.** The flow is not one feature; it is
//! six of them agreeing across two origins:
//!
//!   1. a **cross-origin 302** from the provider to the app's callback,
//!   2. with the **query string carried** (`?code=…&state=…`) — the code IS the query,
//!   3. the post-redirect **`final_url`** reaching the page, so
//!   4. **`location.search`** inside the page is the callback's, not the authorize URL's,
//!   5. a **cross-origin `fetch` POST** carrying a body and a `Content-Type` the page chose, and
//!   6. an **`Authorization: Bearer`** header surviving onto the wire for the userinfo call.
//!
//! Any one of those failing silently produces the same user-visible symptom — a login that hangs on
//! the callback screen — and each lives in a different layer (`manuk-net` redirects, `Page::load`'s
//! URL threading, the JS `location` shim, the fetch pump). A unit test on any single one passes while
//! the login stays broken; that is exactly the composition problem `g_websocket_live` exists for, and
//! this gate is built the same way: two real `TcpListener`s, distinct ports, so the cross-origin hop
//! is genuinely cross-origin rather than simulated.
//!
//! **The `state` parameter is asserted deliberately.** It is CSRF defence for the flow, every real
//! provider requires it, and a client that loses it round-tripping through the redirect looks like it
//! works right up until a provider rejects the exchange. It rides the same query string as the code,
//! so a redirect implementation that truncated the query would be caught by `code` alone — the state
//! assertion is what proves the *whole* query survived rather than just the first parameter.
//!
//! RED, run: dropping the `Location` query on the provider's 302 (serving
//! `Location: http://app/callback`) leaves `final_url` without a code, `location.search` empty, and
//! the page stuck on "waiting" — the hung-callback bug precisely.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use manuk_text::FontContext;

/// The app's callback page: the client half of an authorization-code exchange, written the way a
/// real SPA writes it — read the query, POST the code, then call userinfo with the bearer token.
const CALLBACK_HTML: &str = r##"<!doctype html>
<html><body>
  <div id="out">waiting</div>
  <script>
    var q = {};
    String(location.search || '').replace(/^\?/, '').split('&').forEach(function (kv) {
      if (!kv) { return; }
      var i = kv.indexOf('=');
      q[decodeURIComponent(kv.slice(0, i))] = decodeURIComponent(kv.slice(i + 1));
    });
    var out = document.getElementById('out');

    if (!q.code) {
      out.textContent = 'nocode';
    } else if (q.state !== 'st4te') {
      out.textContent = 'badstate';
    } else {
      fetch('TOKEN_URL', {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: 'grant_type=authorization_code&code=' + encodeURIComponent(q.code)
      })
        .then(function (r) { return r.json(); })
        .then(function (tok) {
          return fetch('USERINFO_URL', {
            headers: { 'Authorization': 'Bearer ' + tok.access_token }
          });
        })
        .then(function (r) { return r.json(); })
        .then(function (u) { out.textContent = 'signedin:' + u.name; })
        .catch(function (e) { out.textContent = 'failed:' + e; });
    }
  </script>
</body></html>"##;

/// Minimal HTTP/1.1 server: one thread per connection, routed on the request line.
///
/// `Content-Length` + `Connection: close` on every response, because the client needs the framing
/// and will otherwise hold the socket open waiting for a body that never ends.
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
                // The whole request is logged: the assertions below are about headers the page set
                // (Authorization, Content-Type) and the body it sent, not just the path.
                log.lock().unwrap().push(format!("{label} {req}"));
                let resp = route(&line, &req);
                let _ = sock.write_all(resp.as_bytes());
                let _ = sock.flush();
            });
        }
    });
    addr
}

fn ok_json(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn ok_html(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[test]
fn a_full_authorization_code_login_completes_across_two_origins() {
    // Jar isolation FIRST, before any net call: the cookie jar is process-global and loads from
    // `$MANUK_STATE` on first use, so without this the gate would read and rewrite the developer's
    // real cookie file.
    let tmp = std::env::temp_dir().join(format!("manuk-oauth-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };

    let plog: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let alog: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // The app origin has to exist before the provider can redirect to it, and the provider's address
    // has to be known before the app can serve a page pointing back at it — so the app's routes read
    // the provider address out of a cell filled in immediately after.
    let provider_addr: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let pa = provider_addr.clone();

    let app_addr = serve("APP", alog.clone(), move |line, _req| {
        if line.starts_with("GET /callback") {
            let p = pa.lock().unwrap().clone();
            let html = CALLBACK_HTML
                .replace("TOKEN_URL", &format!("http://{p}/token"))
                .replace("USERINFO_URL", &format!("http://{p}/userinfo"));
            ok_html(&html)
        } else {
            ok_html("<p>app</p>")
        }
    });

    let app_for_provider = app_addr.clone();
    let provider = serve("IDP", plog.clone(), move |line, req| {
        if line.starts_with("GET /authorize") {
            // The cross-origin hop, carrying the authorization code AND the state in the query.
            format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{app_for_provider}/callback?code=auth_c0de&state=st4te\r\n\
                 Set-Cookie: idp_session=abc; Path=/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
        } else if line.starts_with("POST /token") {
            // Only mint a token for the code we actually issued — otherwise the gate would pass
            // against a client that posted nothing at all.
            if req.contains("code=auth_c0de") {
                ok_json(r#"{"access_token":"t0ken","token_type":"Bearer"}"#)
            } else {
                ok_json(r#"{"error":"invalid_grant"}"#)
            }
        } else if line.starts_with("GET /userinfo") {
            if req
                .to_ascii_lowercase()
                .contains("authorization: bearer t0ken")
            {
                ok_json(r#"{"name":"ada"}"#)
            } else {
                ok_json(r#"{"name":"ANONYMOUS"}"#)
            }
        } else {
            ok_json("{}")
        }
    });
    *provider_addr.lock().unwrap() = provider.clone();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    // ── 1. The user clicks "Sign in": a navigation to the provider's authorize endpoint.
    let authorize = format!(
        "http://{provider}/authorize?client_id=demo&state=st4te&redirect_uri=http://{app_addr}/callback"
    );
    let loaded = rt
        .block_on(manuk_page::fetch_document(&authorize))
        .expect("the authorize navigation must complete");
    let (html, final_url) = match loaded {
        manuk_page::Loaded::Document { html, final_url } => (html, final_url),
        manuk_page::Loaded::Download { filename, .. } => {
            panic!("the authorize redirect produced a DOWNLOAD ({filename}), not a document")
        }
        _ => panic!("the authorize redirect did not produce a document"),
    };

    // The redirect must have been followed ACROSS ORIGINS and landed on the app's callback with the
    // query intact. `final_url` is what becomes `location` inside the page.
    assert!(
        final_url.contains(&app_addr) && final_url.contains("code=auth_c0de"),
        "the 302 must be followed cross-origin with the code preserved in the query\n  \
         final_url: {final_url}\n\n  \
         The authorization code IS the query string; a redirect that drops it hands the callback \
         page nothing to exchange and the login hangs on the callback screen."
    );
    assert!(
        final_url.contains("state=st4te"),
        "the WHOLE query must survive the redirect, not just its first parameter\n  \
         final_url: {final_url}\n\n  \
         `state` is the flow's CSRF defence and every real provider requires it back."
    );

    // ── 2. The callback page runs, reads location.search, and drives the exchange.
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(&html, &final_url, &fonts, 800.0);

    // ── 3. The fetch pump, in the shape the shell runs it (`gui.rs::pump_fetches`): drain, resolve
    // against the document URL, put it on the wire, feed the result back — bounded, because each
    // reaction can issue the next request (this flow is two chained fetches).
    let base = url::Url::parse(&final_url).unwrap();
    for _ in 0..8 {
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
                // status 0 is the engine's network-failure signal; the page's .catch runs.
                Err(_) => page.resolve_fetch(id, 0, "", &[], &fonts, 800.0),
            }
        }
    }

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    assert_eq!(
        got, "signedin:ada",
        "the login must complete end to end\n  got: {got}\n\n  \
         'waiting' = the page never ran the exchange; 'nocode'/'badstate' = the redirect lost the \
         query; 'failed:…' = a fetch did not come back; 'signedin:ANONYMOUS' = the Authorization \
         header did not reach the wire, which is the failure that looks most like success."
    );

    // ── 4. What the servers actually received. The DOM assertion above can be satisfied by a page
    // that guessed; these cannot.
    let idp = plog.lock().unwrap().join("\n").to_ascii_lowercase();
    assert!(
        idp.contains("post /token") && idp.contains("code=auth_c0de"),
        "the token exchange must reach the provider as a POST carrying the code in its BODY \
         (a fetch whose body is dropped is a silent-fail class this project has hit before)\n{idp}"
    );
    assert!(
        idp.contains("content-type: application/x-www-form-urlencoded"),
        "the page's chosen Content-Type must survive onto the wire — a token endpoint rejects a \
         form POST sent as JSON\n{idp}"
    );
    assert!(
        idp.contains("authorization: bearer t0ken"),
        "the bearer token must reach the userinfo request; without it the provider answers as \
         anonymous and the app renders a logged-in shell with nobody in it\n{idp}"
    );
}
