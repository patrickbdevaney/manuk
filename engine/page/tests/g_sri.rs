//! **G_SRI — Subresource Integrity: a script whose bytes do not match its hash does NOT run.**
//!
//! `integrity="sha384-…"` is the page telling us, in advance, exactly which bytes it expects. It is
//! the **one control a page has against a compromised or swapped CDN**, and it is the reason a site
//! can host its framework somewhere it does not control.
//!
//! Surface audit #33 measured this row: it had been carried as `unknown`, which for a security
//! control is the worst status available — nobody knows whether it works, so nobody can rely on it
//! and nobody is alarmed. The measurement found it **absent**: no `integrity` handling anywhere in
//! the tree. Every SRI-protected script on the web was executing unverified.
//!
//! **And the failure was silent, which is what makes it this repo's recurring shape rather than a
//! missing feature.** A substituted script runs, nothing throws, nothing is logged, and the page
//! cannot tell. The user gets attacker code on a page that did everything right.
//!
//! ## What each claim catches
//!
//! - **`ok`** — a script with a CORRECT `sha384` runs. Without this the gate would pass by blocking
//!   everything, which is the cheapest possible wrong implementation of a security control and the
//!   one that would make the browser useless while looking safe.
//! - **`bad`** — a script whose bytes were changed does NOT run. This is the whole point.
//! - **`none`** — a script with no `integrity` still runs (SRI is opt-in; a blanket block would
//!   break the entire web).
//! - **`empty`** — `integrity=""` does not block. Per SRI §3.3.3, an attribute with no *recognised*
//!   metadata is no requirement at all. An implementation that treats "no valid hashes" as "nothing
//!   can match" fails closed on a page that is not asking for anything.
//! - **`weak`** — an attribute naming only an UNKNOWN algorithm (`md5-…`) does not block either,
//!   same clause. The tempting reading — "it asked for integrity, so enforce something" — bricks
//!   pages while protecting nobody.
//! - **`strongest`** — when several algorithms are listed, the STRONGEST present is the one that
//!   decides. A page that lists a correct `sha512` and a stale `sha256` fallback must be judged on
//!   the `sha512`; "any entry matches" would let a page's own weakest metadata downgrade its
//!   strongest, which is a real attack on the mechanism rather than a nicety.
//!
//! The digest is taken over the **raw response bytes**, never `decoded_text()`: transcoding to UTF-8
//! changes the bytes, and every hash on a BOM-prefixed or non-UTF-8 script would fail for the wrong
//! reason — a "security control" that fires on innocent content teaches everyone to remove it.

use base64::Engine as _;
use sha2::Digest as _;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

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
                let (ctype, body) = route(&line)
                    .unwrap_or_else(|| ("text/plain".to_string(), "not found".to_string()));
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes());
                let _ = sock.flush();
            });
        }
    });
    addr
}

fn sha384_b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(sha2::Sha384::digest(s.as_bytes()))
}
fn sha256_b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(sha2::Sha256::digest(s.as_bytes()))
}

/// Each script appends one letter to `#out`, so the result names exactly which ones ran.
fn mark(letter: &str) -> String {
    format!("document.getElementById('out').textContent += '{letter}';")
}

#[test]
fn a_script_whose_bytes_do_not_match_its_integrity_does_not_run() {
    let sokay = mark("A");
    let sbad = mark("B");
    let snone = mark("C");
    let sempty = mark("D");
    let sweak = mark("E");
    let sstrong = mark("F");

    // The hash the page will claim for `/bad.js` is the hash of DIFFERENT bytes — a CDN swap.
    let bad_claimed = sha384_b64("document.getElementById('out').textContent += 'X';");
    // `/strong.js`: a CORRECT sha512 alongside a WRONG sha256. The strongest present must decide,
    // so this one must RUN. (Judged on the sha256 it would be blocked; judged on "any match" a
    // page's weak fallback could authorise anything.)
    let strong_ok =
        base64::engine::general_purpose::STANDARD.encode(sha2::Sha512::digest(sstrong.as_bytes()));

    let (okay_h, bad_h, none_c, empty_c, weak_c, strong_c) = (
        sha384_b64(&sokay),
        bad_claimed,
        snone.clone(),
        sempty.clone(),
        sweak.clone(),
        sstrong.clone(),
    );
    let sokay_c = sokay.clone();
    let sbad_c = sbad.clone();
    let addr = serve(move |line| {
        let path = line.split_whitespace().nth(1).unwrap_or("");
        let js = "application/javascript".to_string();
        match path {
            "/ok.js" => Some((js, sokay_c.clone())),
            "/bad.js" => Some((js, sbad_c.clone())),
            "/none.js" => Some((js, none_c.clone())),
            "/empty.js" => Some((js, empty_c.clone())),
            "/weak.js" => Some((js, weak_c.clone())),
            "/strong.js" => Some((js, strong_c.clone())),
            _ => None,
        }
    });

    let page_html = format!(
        r#"<!doctype html><html><body><div id="out"></div>
<script src="http://{addr}/ok.js" integrity="sha384-{okay_h}"></script>
<script src="http://{addr}/bad.js" integrity="sha384-{bad_h}"></script>
<script src="http://{addr}/none.js"></script>
<script src="http://{addr}/empty.js" integrity=""></script>
<script src="http://{addr}/weak.js" integrity="md5-abcdef"></script>
<script src="http://{addr}/strong.js" integrity="sha256-{wrong256} sha512-{strong_ok}"></script>
</body></html>"#,
        wrong256 = sha256_b64("not the right bytes at all"),
    );

    let doc_addr = serve(move |line| {
        let path = line.split_whitespace().nth(1).unwrap_or("");
        if path == "/" {
            Some(("text/html".to_string(), page_html.clone()))
        } else {
            None
        }
    });

    let fonts = manuk_text::FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let loaded = rt
        .block_on(manuk_page::prefetch_document(&format!(
            "http://{doc_addr}/"
        )))
        .expect("navigation must complete");
    let page = match loaded {
        manuk_page::Loaded::Prefetched(pre) => {
            manuk_page::Page::from_prefetched(*pre, &fonts, 800.0)
        }
        _ => panic!("expected a prefetched document"),
    };
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SRI: ran = {got:?}");

    assert!(
        got.contains('A'),
        "a script with a CORRECT sha384 must RUN. Blocking everything is the cheapest wrong \
         implementation of a security control — safe-looking and useless. got {got:?}"
    );
    assert!(
        !got.contains('B'),
        "**THE POINT**: a script whose bytes do not match its `integrity` must NOT run. This is a \
         swapped CDN, and running it anyway is not a partial implementation of the promise — it is \
         the absence of it, silently. got {got:?}"
    );
    assert!(
        got.contains('C'),
        "a script with NO `integrity` must still run — SRI is opt-in, and a blanket block breaks \
         the entire web. got {got:?}"
    );
    assert!(
        got.contains('D'),
        "`integrity=\"\"` must NOT block: an attribute with no recognised metadata is no requirement \
         at all (SRI §3.3.3). Treating 'no valid hashes' as 'nothing can match' fails CLOSED on a \
         page that is not asking for anything. got {got:?}"
    );
    assert!(
        got.contains('E'),
        "an attribute naming only an UNKNOWN algorithm (`md5-…`) must not block either — same \
         clause. The tempting reading, 'it asked for integrity so enforce something', bricks pages \
         while protecting nobody. got {got:?}"
    );
    assert!(
        got.contains('F'),
        "when several algorithms are listed the STRONGEST present decides: a correct `sha512` \
         alongside a stale `sha256` fallback must RUN. Judging on the weakest would let a page's own \
         fallback downgrade its strongest metadata, which attacks the mechanism itself. got {got:?}"
    );
}
