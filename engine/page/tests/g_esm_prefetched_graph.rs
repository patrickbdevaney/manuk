//! **G_ESM_PREFETCHED_GRAPH — the SHELL path resolves a module import graph across paint (B3b-iii).**
//!
//! `g_esm_page_graph` proved the module-graph pre-fetch on the `load_async` path (streaming/agent). The
//! interactive shell does NOT use `load_async`: it navigates through `prefetch_document` (fetch the
//! document AND all its subresources off the UI thread) → `from_prefetched_blocking_only` (build + run
//! only the paint-blocking scripts) → paint → `run_deferred_scripts` (the deferred/module pass, called
//! LATER). For a native-ESM site to work in the window, the pre-fetched graph must ride from the
//! off-thread fetch, onto the page, and survive that blocking→paint→deferred gap until the module pass
//! seeds it. That carry — `Prefetched.module_graph_sources` → `Page.module_graph_sources` →
//! `run_deferred_scripts` — is exactly what this gate exercises, and it is the seam B3b-iii adds.
//!
//! Same two-level graph as `g_esm_page_graph` (inline root → `/esm-a.js` → `/esm-b.js`), but the whole
//! document is fetched from the origin by `prefetch_document`, and the two passes are run SEPARATELY —
//! the blocking pass first, then (as the shell does after it paints) the deferred pass — so a map that
//! did not survive on the page would be gone by the time the module ran.
//!
//! **RED, run:** drop the `page.module_graph_sources = module_graph_sources` carry in
//! `from_prefetched_inner` (or the pre-fetch in `prepare_prefetched`) and the deferred pass seeds an
//! empty map ⇒ `ModuleLink` cannot resolve `./esm-a.js` ⇒ `#out` stays `-`.

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_page::Loaded;
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

fn serve(path: &str) -> (u16, &'static [u8], &'static str) {
    match path {
        // The document itself is fetched by prefetch_document (unlike load_async, which is handed HTML).
        "/index.html" | "/" => (200, INDEX_HTML.as_bytes(), "text/html"),
        "/esm-a.js" => (200, ESM_A.as_bytes(), "text/javascript"),
        "/esm-b.js" => (200, ESM_B.as_bytes(), "text/javascript"),
        _ => (404, b"not found", "text/plain"),
    }
}

#[test]
fn the_shell_prefetched_path_resolves_a_module_graph_across_paint() {
    let tmp = std::env::temp_dir().join(format!("manuk-esm-pre-{}", std::process::id()));
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
    let index_url = format!("http://{addr}/index.html");
    let fonts = FontContext::new();

    // The shell's exact sequence: fetch document + subresources + module graph off-thread…
    let loaded = rt
        .block_on(manuk_page::prefetch_document(&index_url))
        .expect("prefetch_document");
    let pre = match loaded {
        Loaded::Prefetched(p) => *p,
        _ => panic!("expected a Prefetched document, got a download or plain doc"),
    };
    // …build + run BLOCKING scripts only, then (as the shell does after it paints) the deferred pass.
    // A module graph that did not survive on the page would be gone by this second call.
    let mut page = manuk_page::Page::from_prefetched_blocking_only(pre, &fonts, 800.0);
    page.run_deferred_scripts(&fonts, 800.0);

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("ESM PREFETCHED GRAPH PROBE: {got}");

    assert_eq!(
        got, "answer:42",
        "the shell's prefetch_document → from_prefetched_blocking_only → run_deferred_scripts path must \
         resolve the inline module's relative import graph: prepare_prefetched must pre-fetch \
         ./esm-a.js AND ./esm-b.js, the map must ride onto the page and survive to the deferred pass, \
         and the runner must link + evaluate — so `answer` (42) reaches #out.\n  got: {got:?}\n  `-` \
         means the graph did not survive the blocking→paint→deferred gap (empty map at ModuleLink), the \
         exact seam B3b-iii closes."
    );
}
