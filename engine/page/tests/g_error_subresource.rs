//! **G_ERROR_SUBRESOURCE — a 404 page is a DOCUMENT to render. It is not JavaScript to execute, and
//! not CSS to apply.**
//!
//! t607 established the first half and was right: an HTTP error status arrives with a real body, and
//! every browser renders it, so `manuk_net::fetch` stopped failing on `status >= 400`. Its own comment
//! promised the rest — *"the status is not swallowed: it rides on `Response::status` for every caller
//! that cares"* — and **none of the six subresource callers cared.** Every one of them read
//! *"the request completed"* as *"the request succeeded"*:
//!
//! ```rust
//! let text = manuk_net::fetch(&url).await.ok().map(|r| r.decoded_text());
//! ```
//!
//! So a `<script src>` that 404s had its error page injected **as inline JavaScript**, and a
//! `<script type=module>` had it compiled as a module. Found on `www.welt.de`, as the last visible
//! rung of the chain t612/t613/t615 peeled:
//!
//! ```text
//!   a page module failed — SyntaxError: expected expression, got '<'
//! ```
//!
//! **The distinction is the whole point, and it is one fact answered three ways.** A 403 challenge
//! page is a *document* to the navigation path (render it — t607), *not evidence* to the certificate
//! (refuse to score it — t611), and *not code* here. One response, three consumers, three correct and
//! different answers. **Getting one right does not settle the others** — which is exactly how this
//! defect was introduced by a tick that was itself correct.

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_text::FontContext;

/// A server that answers **404 with a plausible HTML error page** for every request — the shape a CDN
/// or a mis-deployed bundle actually returns. The body is deliberately valid HTML and invalid
/// JavaScript, because that is the pair that produces the failure.
fn error_origin() -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                const BODY: &str =
                    "<!doctype html><html><body><h1>404 Not Found</h1></body></html>";
                let _ = s.write_all(
                    format!(
                        "HTTP/1.1 404 Not Found\r\nContent-Type: text/html\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{BODY}",
                        BODY.len()
                    )
                    .as_bytes(),
                );
            });
        }
    });
    format!("http://{addr}")
}

#[test]
fn an_error_page_is_never_executed_or_applied_as_a_subresource() {
    let fonts = FontContext::new();
    let origin = error_origin();

    // Three subresources, all answering 404 with HTML: a classic script, a stylesheet, and a module.
    let html = format!(
        r#"<!doctype html><html><head>
             <link rel="stylesheet" href="{origin}/missing.css">
           </head><body>
             <div id="out">clean</div>
             <script src="{origin}/missing.js"></script>
             <script type="module">import "{origin}/missing.mjs";</script>
           </body></html>"#
    );

    // **`Page::load` DOES NOT FETCH — the first draft of this gate used it and passed with the fix
    // disabled.** The sync constructor never touches the network, so every assertion below was true
    // by construction and the gate was measuring nothing. `load_async` is the path the shell and the
    // renderer actually take, and it is the one that fetches subresources. (Third vacuous first-draft
    // gate this session; the RED probe is the only reason any of the three were caught.)
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        &html,
        &format!("{origin}/index.html"),
        &fonts,
        800.0,
    ));
    let root = page.dom().root();

    // ── 1. THE ERROR PAGE'S TEXT MUST NOT BE IN THE DOCUMENT AS SCRIPT CONTENT.
    //
    // This is the assertion that catches the real defect: the fetched body was written into the
    // `<script>` node as inline text, so the `<h1>404 Not Found</h1>` markup literally became the
    // page's JavaScript. Serialising the DOM and looking for it is the most direct statement of
    // "we did not do that".
    let serialized = manuk_html::serialize_inner(page.dom(), root);
    assert!(
        !serialized.contains("404 Not Found"),
        "G_ERROR_SUBRESOURCE: the 404 page's body was injected into the document.\n\
         A failed subresource must leave NOTHING behind — instead its error HTML became script \
         content, which SpiderMonkey then compiles as JavaScript (`SyntaxError: expected \
         expression, got '<'`, welt.de's last rung). Serialized document:\n{serialized}"
    );

    // ── 2. THE SCRIPT ELEMENT STILL LOOKS EXTERNAL, which is the established "nothing to run" state.
    //
    // A failed fetch and an SRI mismatch both leave `src` in place precisely so
    // `collect_inline_scripts` skips the node. An error response must land in that same state rather
    // than inventing a third one.
    let scripts = manuk_css::query_selector_all(page.dom(), root, "script[src]");
    assert_eq!(
        scripts.len(),
        1,
        "the <script src> whose fetch 404'd must still carry `src` — that is how the rest of the \
         pipeline knows there is nothing to run. Found {} such nodes.",
        scripts.len()
    );
    assert!(
        page.dom().text_content(scripts[0]).trim().is_empty(),
        "…and it must have no inline text: the error page was written into it as code. Got: {:?}",
        page.dom().text_content(scripts[0])
    );

    // ── 3. THE PAGE STILL RENDERS. Refusing the subresource must not take the document with it —
    // that would trade one failure for a worse one, which the ratchet forbids.
    let out = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert_eq!(out.len(), 1, "the document still parsed and laid out");
    assert_eq!(
        page.dom().text_content(out[0]),
        "clean",
        "the page's own content survives a 404'd script, stylesheet and module"
    );
}
