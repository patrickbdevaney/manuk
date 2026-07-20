//! **G_CSP** — Content-Security-Policy `script-src` is enforced, and enforced in both directions.
//!
//! CSP is the one security mechanism where *receiving* the header and *honouring* it are completely
//! indistinguishable from the page's side until the day an injection lands. A browser that parses
//! the policy and runs the script anyway behaves identically to one that ignores the header
//! entirely — so this cannot be gated by "we have a CSP module". It has to be gated by a script
//! that would have run, not running, and by a request that would have been made, not being made.
//!
//! Why an integration gate and not a unit test. The policy evaluator has its own unit tests in
//! `manuk-net` (19 of them) and they prove the *matching rules*. They cannot prove that anything
//! consults the evaluator. Four separate layers have to agree for enforcement to be real:
//!
//!   1. `manuk-net` must surface the response headers instead of dropping them at the document
//!      boundary (they were dropped, before this gate existed);
//!   2. `manuk-page` must carry the parsed policy across the off-thread prefetch and seed it before
//!      the page is constructed — the policy must be in force *before* the first script runs;
//!   3. the external-script fetch must consult it **before issuing the request**, not after the
//!      response lands;
//!   4. `manuk-js` must consult it when collecting inline scripts, reading each element's `nonce`.
//!
//! Any one of those four silently failing gives a browser that "supports CSP" and enforces nothing.
//!
//! **RED, run:** delete the `csp.allows_script_url` filter in `fetch_external_scripts` → the
//! cross-origin script runs and `evil` server's log is non-empty. Delete the `csp_allows_inline`
//! check in `collect_inline_scripts` → the un-nonced inline scripts run. Make either check return a
//! constant `false` → the nonced script and the same-origin script stop running, and the no-policy
//! control page goes blank. **No constant satisfies this gate**, which is the property that makes it
//! a ratchet tooth: the assertions on what MUST run are exact complements of those on what MUST NOT.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// Minimal HTTP/1.1 server: one thread per connection, routed on the request line. The whole
/// request is logged, because the load-bearing assertion here is about a request that must NEVER
/// APPEAR — which is only observable from the server side.
fn serve(
    label: &'static str,
    log: Arc<Mutex<Vec<String>>>,
    route: impl Fn(&str) -> String + Send + Sync + 'static,
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
                log.lock().unwrap().push(format!("{label} {line}"));
                let resp = route(&line);
                let _ = sock.write_all(resp.as_bytes());
                let _ = sock.flush();
            });
        }
    });
    addr
}

fn resp(headers: &str, ctype: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n{headers}Connection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Every script on the page appends one letter to `#out`. The final string names exactly which
/// scripts ran, in order — so a failure says *which* rule broke rather than just "not equal".
fn mark(letter: &str) -> String {
    format!("document.getElementById('out').textContent += '{letter}';")
}

fn load(url: &str, fonts: &manuk_text::FontContext) -> manuk_page::Page {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let loaded = rt
        .block_on(manuk_page::prefetch_document(url))
        .expect("navigation must complete");
    match loaded {
        manuk_page::Loaded::Prefetched(pre) => {
            manuk_page::Page::from_prefetched(*pre, fonts, 800.0)
        }
        _ => panic!("expected a prefetched document"),
    }
}

fn out_of(page: &manuk_page::Page) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "the gate page must contain #out");
    page.dom().text_content(hits[0])
}

#[test]
fn csp_script_src_blocks_what_it_forbids_and_only_what_it_forbids() {
    // Jar isolation first: the cookie jar is process-global and would otherwise touch the
    // developer's real state file.
    let tmp = std::env::temp_dir().join(format!("manuk-csp-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    std::env::set_var("MANUK_STATE", &tmp);

    let fonts = manuk_text::FontContext::new();

    // ── The third-party origin. It serves a script that, if it ever runs, marks the page — and it
    // logs every request, so we can prove the request was never even issued.
    let evil_log = Arc::new(Mutex::new(Vec::new()));
    let evil = serve("evil", evil_log.clone(), |_line| {
        resp("", "application/javascript", &mark("X"))
    });

    // ── The document origin.
    let site_log = Arc::new(Mutex::new(Vec::new()));
    let evil_for_site = evil.clone();
    let site = serve("site", site_log.clone(), move |line| {
        let path = line.split_whitespace().nth(1).unwrap_or("/");
        match path {
            // Same-origin external script — MUST be fetched and MUST run under `script-src 'self'`.
            "/same.js" => resp("", "application/javascript", &mark("S")),

            // The policy page. `'self'` plus a single nonce.
            "/policy" => resp(
                "Content-Security-Policy: script-src 'self' 'nonce-goodnonce'\r\n",
                "text/html",
                &format!(
                    r#"<!doctype html><html><body><div id="out"></div>
<script>{no_nonce}</script>
<script nonce="goodnonce">{good}</script>
<script nonce="wrongnonce">{wrong}</script>
<script src="/same.js"></script>
<script src="http://{evil_for_site}/evil.js"></script>
</body></html>"#,
                    no_nonce = mark("N"),
                    good = mark("G"),
                    wrong = mark("W"),
                ),
            ),

            // A policy delivered in the MARKUP rather than a header, with no header at all.
            "/meta" => resp(
                "",
                "text/html",
                &format!(
                    r#"<!doctype html><html><head>
<meta http-equiv="Content-Security-Policy" content="script-src 'none'">
</head><body><div id="out"></div>
<script>{}</script>
</body></html>"#,
                    mark("M")
                ),
            ),

            // **The control.** No CSP at all — everything must run. Without this, "block
            // everything" would pass the gate, and a browser that runs no scripts is not a
            // browser. It is served LAST in the test order on purpose: it also proves the policy
            // from the previous navigation did not leak into this one.
            "/nopolicy" => resp(
                "",
                "text/html",
                &format!(
                    r#"<!doctype html><html><body><div id="out"></div>
<script>{inline}</script>
<script src="http://{evil_for_site}/evil.js"></script>
</body></html>"#,
                    inline = mark("C"),
                ),
            ),
            _ => resp("", "text/html", "<html><body>?</body></html>"),
        }
    });

    // ── 1. The header policy.
    let page = load(&format!("http://{site}/policy"), &fonts);
    let got = out_of(&page);
    assert_eq!(
        got, "GS",
        "under `script-src 'self' 'nonce-goodnonce'`, exactly the nonced inline script (G) and the \
         same-origin external script (S) may run.\n  got: {got:?}\n\n  \
         'N' present = an un-nonced inline script ran: inline is not being checked at all.\n  \
         'W' present = a WRONG nonce was accepted: the comparison is not exact.\n  \
         'X' present = the cross-origin script ran: the URL check is missing.\n  \
         'G' missing = the nonce did not match though it should have (over-blocking).\n  \
         'S' missing = 'self' did not authorize the document's own origin (over-blocking).\n  \
         empty = nothing ran at all, which is the failure mode that looks most like security."
    );

    // ── 2. What the servers actually received. This is the half the DOM cannot show: a blocked
    // script that is fetched anyway still tells the third party that this user loaded this page.
    // CSP is a check on the REQUEST, not only on the execution.
    let evil_seen = evil_log.lock().unwrap().clone();
    assert!(
        evil_seen.is_empty(),
        "the forbidden cross-origin script must never be REQUESTED, not merely not executed — \
         blocking after the response has landed still leaks the visit.\n  evil server saw: {evil_seen:?}"
    );
    let site_seen = site_log.lock().unwrap().clone();
    assert!(
        site_seen.iter().any(|l| l.contains("/same.js")),
        "the ALLOWED same-origin script must actually have been fetched — otherwise the 'S' above \
         could only have come from somewhere else.\n  site server saw: {site_seen:?}"
    );

    // ── 3. A policy delivered by <meta http-equiv>, with no response header.
    let page = load(&format!("http://{site}/meta"), &fonts);
    let got = out_of(&page);
    assert_eq!(
        got, "",
        "a `<meta http-equiv=\"Content-Security-Policy\">` policy is enforced exactly like a header \
         one; `script-src 'none'` must stop the inline script.\n  got: {got:?}"
    );

    // ── 4. The control: no policy, nothing blocked. Also proves no leak from the two policies above.
    let page = load(&format!("http://{site}/nopolicy"), &fonts);
    let got = out_of(&page);
    assert_eq!(
        got, "CX",
        "a document with NO CSP must run every script, inline and cross-origin alike. If this is \
         empty or partial, enforcement is leaking from a previous navigation onto a page that never \
         sent a policy — which breaks innocent sites, and is worse than not enforcing at all.\n  \
         got: {got:?}"
    );
    let evil_seen = evil_log.lock().unwrap().clone();
    assert!(
        evil_seen.iter().any(|l| l.contains("/evil.js")),
        "with no policy the cross-origin script must be fetched normally.\n  evil saw: {evil_seen:?}"
    );
}
