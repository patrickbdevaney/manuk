//! **G_ESM_PAGE_GRAPH — an inline `<script type=module>` imports a relative graph, end to end.**
//!
//! ESM import-graph **B3b-ii** — the async producer on the real page path. The JS-layer gate
//! (`g_esm_import_graph`) proves the loader core: given a *pre-fetched* source map, `run_module` drives
//! the population walk, links, evaluates, and clears the registry (B1→B2→B3→B3b-i). What it cannot prove
//! is that a real page ever *fills* that map — that is this gate.
//!
//! It stands up a localhost origin serving a two-level module graph:
//!
//! ```text
//!   <inline module>  import { answer } from './esm-a.js';  document.getElementById('out') = answer
//!        └── /esm-a.js  import { six } from './esm-b.js';   export const answer = six * 7
//!                 └── /esm-b.js  export const six = 6
//! ```
//!
//! and loads the document through `Page::load_async` — the exact path the shell/render use. For the
//! import to resolve, `load_async` must: scan the inline module's static `import`s, fetch `./esm-a.js`
//! off the UI thread, scan *its* imports, fetch `./esm-b.js` (the transitive dependency — this is why the
//! graph is two levels, not one), seed the resolved-url → source map into the JS layer, and let the
//! module runner link + evaluate it. The binding `answer` (42) reaching a DOM node proves the whole
//! producer→consumer seam: pre-fetch on the page path, then link over the pre-fetched map.
//!
//! **RED, run:** neuter `prefetch_module_graph` to `return HashMap::new()` (or drop the
//! `set_module_graph_sources` call in `load_async`) and the map is empty ⇒ `run_module` links a
//! self-contained module ⇒ `module_resolve_hook` returns null for `./esm-a.js` ⇒ `ModuleLink` fails ⇒
//! the module never evaluates ⇒ `#out` stays `-`. A miss is loud-but-safe: the page renders, the import
//! just does not resolve, exactly as documented.

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_text::FontContext;

const INDEX_HTML: &str = r##"<!doctype html>
<html><body>
  <div id="out">-</div>
  <script type="module">
    import { answer } from './esm-a.js';
    document.getElementById('out').textContent = 'answer:' + answer;
  </script>
</body></html>"##;

const ESM_A: &str = "import { six } from './esm-b.js';\nexport const answer = six * 7;\n";
const ESM_B: &str = "export const six = 6;\n";

/// Serve the two module files (and 404 everything else so nothing hangs). One connection per request;
/// `Connection: close` so the client's fetch completes promptly.
fn serve(path: &str) -> (u16, &'static [u8], &'static str) {
    match path {
        "/esm-a.js" => (200, ESM_A.as_bytes(), "text/javascript"),
        "/esm-b.js" => (200, ESM_B.as_bytes(), "text/javascript"),
        _ => (404, b"not found", "text/plain"),
    }
}

#[test]
fn an_inline_module_imports_a_relative_graph_end_to_end() {
    let tmp = std::env::temp_dir().join(format!("manuk-esm-page-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut sock) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                let (status, body, ctype) = serve(&path);
                let head = format!(
                    "HTTP/1.1 {status} {}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n\
                     Connection: close\r\n\r\n",
                    if status == 200 { "OK" } else { "Not Found" },
                    body.len()
                );
                let _ = sock.write_all(head.as_bytes());
                let _ = sock.write_all(body);
                let _ = sock.flush();
            });
        }
    });

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let base = format!("http://{addr}/index.html");
    let fonts = FontContext::new();
    let page = rt.block_on(manuk_page::Page::load_async(
        INDEX_HTML, &base, &fonts, 800.0,
    ));

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("ESM PAGE GRAPH PROBE: {got}");

    assert_eq!(
        got, "answer:42",
        "the inline module's relative import graph must resolve on the real page path: `load_async` \
         must pre-fetch ./esm-a.js AND its transitive dependency ./esm-b.js, seed the module-graph \
         source map, and let the runner link + evaluate — so `answer` (six*7 = 42) reaches #out.\n  \
         got: {got:?}\n  `-` means the graph was never pre-fetched (map empty ⇒ ModuleLink could not \
         resolve ./esm-a.js), which is the exact gap B3b-ii closes."
    );
}
