//! **G_MODULE_BASE_URL — an external `<script type=module>` resolved its relative imports against the
//! DOCUMENT instead of against itself, so every bundler-built site fetched the wrong URLs.**
//!
//! `fetch_external_scripts` inlines a `<script src>` into the node and drops `src`. By the time
//! `prefetch_module_graph` walks the DOM, an external module is therefore **indistinguishable from an
//! inline one** — and the walk resolved its imports against the document URL. That is correct for a
//! genuinely inline module, which is exactly why the bug hid behind a true comment:
//!
//! ```text
//!   "the document URL, since an inline module resolves its relative imports against the document"
//! ```
//!
//! True, and applied to a case it does not cover. **A module's imports resolve against the MODULE's
//! url.** For an inline module that happens to be the document; for an external one it is not.
//!
//! MEASURED on `www.welt.de`, whose entry module is `/assets/bff-section/scripts/section.module.js`
//! and imports `./chunks/react.BPdhuoKc.js`:
//!
//! ```text
//!   /assets/bff-section/scripts/chunks/react.BPdhuoKc.js   200 ·   8,391 bytes · real JavaScript
//!   /chunks/react.BPdhuoKc.js                              404 · 414,112 bytes · HTML
//! ```
//!
//! One directory tree too high, an SPA fallback answering with HTML, and `SyntaxError: expected
//! expression, got '<'`. **This is the shape of every Vite/Rollup/esbuild production build** — an
//! entry module under a hashed asset directory importing its chunks relatively — so the blast radius
//! is "sites that ship modern bundled JavaScript", not one newspaper.
//!
//! The gate serves the dependency at the SCRIPT-relative path **only**, and 404s the
//! document-relative one, so nothing but correct resolution can make it pass.

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_text::FontContext;

/// Serves `/assets/app/entry.js` (a module importing `./dep.js`) and its dependency at the
/// SCRIPT-relative `/assets/app/dep.js`. The document-relative `/dep.js` answers **404 with HTML** —
/// the SPA-fallback shape that turned this bug into a `SyntaxError` rather than a clean miss.
fn module_origin() -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                let (status, ctype, body): (&str, &str, String) = match path.as_str() {
                    "/assets/app/entry.js" => (
                        "200 OK",
                        "text/javascript",
                        "import { answer } from './dep.js';\n\
                         document.getElementById('out').textContent = 'answer:' + answer;\n"
                            .to_string(),
                    ),
                    // The ONLY place the dependency exists: relative to the SCRIPT.
                    "/assets/app/dep.js" => (
                        "200 OK",
                        "text/javascript",
                        "export const answer = 42;\n".to_string(),
                    ),
                    // Anything else — including the document-relative `/dep.js` the bug asked for —
                    // gets the SPA fallback: 404 with an HTML body.
                    _ => (
                        "404 Not Found",
                        "text/html",
                        "<!doctype html><html><body>not found</body></html>".to_string(),
                    ),
                };
                let _ = s.write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            });
        }
    });
    format!("http://{addr}")
}

#[test]
fn an_external_module_resolves_imports_against_itself_not_the_document() {
    let fonts = FontContext::new();
    let origin = module_origin();

    // The document lives at the ROOT, the module lives two directories down. That gap is the whole
    // test: `./dep.js` means `/assets/app/dep.js`, and the bug asked for `/dep.js`.
    let html = format!(
        r#"<!doctype html><html><body>
             <div id="out">-</div>
             <script type="module" src="{origin}/assets/app/entry.js"></script>
           </body></html>"#
    );

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
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    assert_eq!(
        got, "answer:42",
        "G_MODULE_BASE_URL: an external module's relative import must resolve against the MODULE's \
         url, not the document's.\n  got: {got:?}\n\n  \
         `-` means `./dep.js` was fetched as `/dep.js` (the document-relative path) instead of \
         `/assets/app/dep.js`, so the dependency never entered the pre-fetched graph and the import \
         could not link. On a real origin that wrong path does not 404 cleanly — it returns the SPA \
         fallback, 414KB of HTML on www.welt.de, which then compiles as a module and throws \
         `SyntaxError: expected expression, got '<'`. This is the shape of every Vite/Rollup build: \
         an entry module under a hashed asset directory importing its chunks relatively."
    );
}
