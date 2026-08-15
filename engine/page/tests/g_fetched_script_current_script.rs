//! **G_FETCHED_CURRENT_SCRIPT — `document.currentScript` is the executing element for a
//! RUNTIME-FETCHED script too, not just an inline one.**
//!
//! There are three script entry points in this engine and only two of them set it. The parse path
//! (`run_deferred_scripts`) and the injected path (`run_one_script`) both do; `PageContext::eval` —
//! **the path a `<script src>` fetched at runtime takes, which is how the modern web ships nearly
//! all of its code** — did not, so `document.currentScript` was `null` inside every external script
//! and `document.currentScript.src` was a TypeError that took the loader with it.
//!
//! `G_CURRENT_SCRIPT` exists and is green, and it did not catch this: it asserts the *inline* case,
//! which is the entry point that was already right. **A family of three entry points needs a gate
//! per entry point, not a gate per behaviour** — the same shape as `insertAdjacentText` (the third
//! sibling nobody feature-detects) and of the six `apply_natural_sizes` callers where being the only
//! correct site is how the others stayed wrong.
//!
//! Found by consequence rather than by audit: `document.write` started working (t1262), an ad
//! loader on `videa.hu` that had never run before reached `document.currentScript.src`, and died
//! there. The webpack `publicPath: "auto"` chunk loader is the same line.
//!
//! Proven RED: without `set_current_script(script_node)` in `PageContext::eval`, `cs-null` reads
//! `true` and `cs-self` reads `false`.

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_text::FontContext;

/// Serves one JavaScript file that reports what `document.currentScript` was while it ran.
fn script_origin() -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                // The chunk-loader opening move, verbatim: stash currentScript, then read off it.
                const BODY: &str = "var cs = document.currentScript;\
                     window.r = [];\
                     window.r.push('cs-null:' + (cs === null));\
                     window.r.push('cs-self:' + (cs === document.getElementById('ext')));\
                     window.r.push('cs-tag:' + (cs && cs.tagName));\
                     window.r.push('cs-attr:' + (cs && cs.getAttribute('data-cfg')));\
                     document.getElementById('out').textContent = window.r.join(' ');";
                let _ = s.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\n\
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
fn a_runtime_fetched_script_sees_itself_as_current_script() {
    let fonts = FontContext::new();
    let origin = script_origin();

    // ⚠⚠⚠ **THE SCRIPT MUST BE INJECTED AT RUNTIME, AND THE FIRST DRAFT OF THIS GATE WAS NOT.**
    // A `<script src>` present in the authored markup is fetched and run by the *parse* path
    // (`run_deferred_scripts` -> `run_one_script`), which has always set `currentScript` — so that
    // version passed with the fix disabled and asserted nothing. `createElement('script')` + `src`
    // + `appendChild` is the path that reaches `PageContext::eval`, and it is also, not
    // coincidentally, how every code-split bundle on the web loads its chunks.
    let html = format!(
        r#"<!doctype html><html><body>
             <div id="out">-</div>
             <script>
               var s = document.createElement('script');
               s.id = 'ext';
               s.setAttribute('data-cfg', 'alpha');
               s.src = "{origin}/chunk.js";
               document.body.appendChild(s);
             </script>
           </body></html>"#
    );

    // `Page::load` does not fetch — it is the sync constructor, and a gate built on it would pass
    // with the fix disabled. `load_async` is the path the shell and the renderer take.
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
    // Drive the runtime-injected-script pump explicitly, so this gate names the entry point it is
    // about (`fetch_and_run_dynamic_scripts` -> `PageContext::eval`) instead of depending on which
    // load phases happen to run it.
    rt.block_on(page.fetch_and_run_dynamic_scripts(&fonts, 800.0, 4));
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("FETCHED-CURRENT-SCRIPT RESULT: {got}");

    // NON-VACUITY: if the script never ran, `#out` still reads "-" and every `contains` below
    // would be trivially false — but say so explicitly, because "the fetch quietly failed" and
    // "currentScript is wrong" are different bugs and must not be reported as each other.
    assert_ne!(
        got.trim(),
        "-",
        "G_FETCHED_CURRENT_SCRIPT: the fetched script never ran at all — this gate is measuring \
         nothing. That is a subresource-fetch failure, not a currentScript failure."
    );

    for claim in [
        "cs-null:false", // it is NOT null during an external classic script (the whole bug)
        "cs-self:true",  // it is THAT element — asserted against the page's own getElementById
        "cs-tag:SCRIPT", // as a real element reflector
        "cs-attr:alpha", // whose attributes are readable — the `data-*` config every loader reads
    ] {
        assert!(
            got.contains(claim),
            "G_FETCHED_CURRENT_SCRIPT: expected `{claim}`\n  got: {got}\n\n  \
             `document.currentScript` must be the executing <script> element during a CLASSIC \
             script, and an externally fetched script is a classic script (HTML §4.12.1). It was \
             null on this path only — the inline path was always right, which is exactly why \
             G_CURRENT_SCRIPT stayed green while every chunk loader on the web read null."
        );
    }
}
