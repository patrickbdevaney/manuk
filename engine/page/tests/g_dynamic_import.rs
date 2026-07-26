//! **G_DYNAMIC_IMPORT — `import()` threw *"Dynamic module import is disabled or not supported in this
//! context"* at every call, and the map claimed it `works`.**
//!
//! Surface audit #34 (t618) found the row: `ES modules + dynamic import()` — status `works`, gate `-`.
//! **One row asserting two capabilities reports the stronger one's verdict for both.** Static ESM was
//! real; `import()` had no host hook at all, so SpiderMonkey rejected every call by design. Every
//! code-splitting bundle uses it, and `www.welt.de` reaches it once t617 fixed module base URLs.
//!
//! **Why a synchronous hook is the honest implementation.** The spec's hook starts an async operation
//! and the embedder calls `FinishDynamicModuleImport` later. We have no synchronous network on the JS
//! thread, so the module must already be in hand — and it is: the page pre-fetches the reachable
//! graph, now including literal `import("…")` specifiers, before any script runs.
//!
//! ⚠ **THE BUG THAT PARKED THIS FOR A TICK WAS A LIFETIME, NOT THE HOOK.** t620 got every step
//! succeeding — compiled, linked, evaluated, registered, no pending exception — and the caller's
//! promise still rejected with `undefined`. The cause: `run_module` cleared the module registry the
//! instant the ROOT was linked, reasoning that SpiderMonkey's own records then keep the graph alive.
//! True for static imports; **false for dynamic ones**, because `FinishDynamicModuleImport` completes
//! the caller's promise in a LATER MICROTASK and the module must still be resolvable then. The clear
//! now happens at the end of the script pass, preserving the contract that actually matters: the
//! registry never outlives the NAVIGATION.
//!
//! **The rejection path is asserted too.** A computed specifier cannot be seen by a textual pre-scan,
//! so it misses — and the page's `.catch()` runs, which is what such a page already relies on. A
//! promise that never settles would be far worse than one that rejects, so `FinishDynamicModuleImport`
//! is called on both paths.

use manuk_text::FontContext;
use std::io::{Read, Write};
use std::net::TcpListener;

fn origin() -> String {
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
                let (st, body) = match path.as_str() {
                    "/app/chunk.js" => ("200 OK", "export const answer = 42;\n".to_string()),
                    _ => ("404 Not Found", "<!doctype html><html></html>".to_string()),
                };
                let _ = s.write_all(format!("HTTP/1.1 {st}\r\nContent-Type: text/javascript\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    format!("http://{addr}")
}

#[test]
fn dynamic_import_resolves_and_rejects_honestly() {
    let fonts = FontContext::new();
    let origin = origin();
    let html = format!(
        r#"<!doctype html><html><body><div id="out">-</div>
<script type="module">
  var R = [];
  import('./app/chunk.js')
    .then(function (m) {{ R.push('ok:' + m.answer); }}, function (e) {{ R.push('rejected:' + e); }})
    .then(function () {{
      return import('./app/never.js').then(function () {{ R.push('miss:resolved'); }},
                                           function () {{ R.push('miss:rejected'); }});
    }})
    .then(function () {{ document.getElementById('out').textContent = R.join(' '); }});
</script></body></html>"#
    );
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut page = rt.block_on(manuk_page::Page::load_async(
        &html,
        &format!("{origin}/index.html"),
        &fonts,
        800.0,
    ));
    rt.block_on(page.finish_loading(&fonts, 800.0));
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    assert!(
        got.contains("ok:42"),
        "G_DYNAMIC_IMPORT: `import('./app/chunk.js')` did not resolve to its module.\n  got: {got:?}\n\n  \
         An EMPTY result means the promise never settled. `rejected:` with the text \"Dynamic module \
         import is disabled\" means no HostImportModuleDynamically hook is installed — note there are \
         TWO SetModuleResolveHook sites and the page path uses the second, so installing at one only \
         looks correct and is not. `rejected:undefined` with everything else green is the LIFETIME bug: \
         the module registry was cleared before FinishDynamicModuleImport's microtask could resolve it."
    );
    assert!(
        got.contains("miss:rejected"),
        "G_DYNAMIC_IMPORT: an un-prefetched specifier must REJECT, not hang or resolve.\n  got: {got:?}\n\n  \
         FinishDynamicModuleImport has to be called on the failure path too, or the caller's promise \
         never settles and the page's own `.catch()` — the thing a runtime-computed specifier relies \
         on — never runs. A silent hang is the worst of the three outcomes."
    );
}
