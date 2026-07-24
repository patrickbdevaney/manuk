//! **G_ESM_IMPORT_MAP — a `<script type=importmap>` resolves bare specifiers, end to end (tick 520).**
//!
//! The ESM import-graph subsystem (t512-517) resolves RELATIVE specifiers (`./b.js`). A bare specifier
//! (`import 'react'`) has no built-in resolution in this engine, so without an import map it fails at
//! `ModuleLink` — which is exactly how a CDN-pinned no-bundler app (`import {h} from 'preact'` mapped to
//! `https://esm.sh/preact`) fails to boot. This gate proves the import map closes that gap.
//!
//! A localhost origin serves a document whose `<script type=importmap>` declares BOTH standard forms:
//!
//! ```json
//! { "imports": { "greeter": "./lib/greeter.js", "utils/": "./lib/utils/" } }
//! ```
//!
//! and an inline module that imports through both — a bare exact key (`greeter`) and a bare trailing-
//! slash PREFIX key (`utils/num.js` → `./lib/utils/num.js`). For it to render, `load_async` must parse
//! the map, resolve the bare specifiers against the DOCUMENT url, pre-fetch the mapped files, and the
//! resolve hook must return them at link. The result `hi:42` reaching the DOM proves the whole path:
//! parse → both map forms → document-relative resolution → pre-fetch → link.
//!
//! **RED, run:** clear the `IMPORT_MAP` consult in `resolve_module_specifier` (or the
//! `page.import_map = import_map` carry) → bare `greeter` resolves to nothing (or to `base/greeter`,
//! a 404) → `ModuleLink` fails → `#out` stays `-`.

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_text::FontContext;

const INDEX_HTML: &str = r##"<!doctype html>
<html><body>
  <div id="out">-</div>
  <script type="importmap">
    { "imports": { "greeter": "./lib/greeter.js", "utils/": "./lib/utils/" } }
  </script>
  <script type="module">
    import { greet } from 'greeter';
    import { six } from 'utils/num.js';
    document.getElementById('out').textContent = greet() + ':' + (six * 7);
  </script>
</body></html>"##;

const GREETER: &str = "export function greet(){ return 'hi'; }\n";
const NUM: &str = "export const six = 6;\n";

fn serve(path: &str) -> (u16, &'static [u8], &'static str) {
    match path {
        "/lib/greeter.js" => (200, GREETER.as_bytes(), "text/javascript"),
        "/lib/utils/num.js" => (200, NUM.as_bytes(), "text/javascript"),
        _ => (404, b"not found", "text/plain"),
    }
}

#[test]
fn an_import_map_resolves_bare_specifiers_end_to_end() {
    let tmp = std::env::temp_dir().join(format!("manuk-esm-im-{}", std::process::id()));
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
    println!("ESM IMPORT MAP PROBE: {got}");

    assert_eq!(
        got, "hi:42",
        "the import map must resolve BOTH bare-specifier forms end to end: an exact key ('greeter' -> \
         ./lib/greeter.js) and a trailing-slash prefix key ('utils/num.js' -> ./lib/utils/num.js), each \
         resolved against the document url, pre-fetched, and returned by the resolve hook at link — so \
         greet() + (six*7) = 'hi:42' reaches #out.\n  got: {got:?}\n  `-` means a bare specifier did not \
         resolve through the import map (unmapped -> ModuleLink failed), the exact gap tick 520 closes."
    );
}
