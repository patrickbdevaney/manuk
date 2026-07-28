//! **G_MARKUP_SCRIPT_LOAD_EVENT — a `<script src>` written in the MARKUP that loaded successfully
//! told the page nothing, and a completion event that never fires is silent by construction.**
//!
//! The sibling gate `g_script_load_event` covers the *injected* path — `createElement('script')` →
//! `src` → `appendChild` — and has since the agoda `ChunkLoadError` tick. **The parser-inserted path
//! was never given the same treatment**, and its success case was the one hole left:
//!
//! ```text
//!   script-inserted <script src>, 200   ->  load    ✅  g_script_load_event
//!   script-inserted <script src>, 404   ->  error   ✅  g_script_load_event
//!   parser-inserted <script src>, 404   ->  error   ✅  (src survives the failed fetch, so the
//!                                                        injected-script drain picks the node up)
//!   parser-inserted <script src>, 200   ->  NOTHING ❌  <- this gate
//! ```
//!
//! One rule, two implementations, and only one of them was built. The success case is the silent one
//! because `fetch_external_scripts` **inlines the source and removes `src`** — after which nothing
//! downstream remembers the element was ever external, and it is indistinguishable from an
//! author-written inline script.
//!
//! **MEASURED against headless Chrome**, one fixture, four cases, scripts from `code.jquery.com`:
//!
//! ```text
//!   Chrome   parserOK:load | parser404:error | dynOK:load | dyn404:error | window:load
//!   Manuk                    parser404:error | dynOK:load | dyn404:error | window:load
//! ```
//!
//! **The idiom it breaks is the ordinary one.** From the served bytes of `wix.com`:
//!
//! ```text
//!   <script id="wix-footer-script" src="…"></script>
//!   document.getElementById('wix-footer-script').onload = function () {
//!     window.WixFooter.render({ target: document.querySelector('#WIX_FOOTER'), … })
//!   };
//! ```
//!
//! The script arrives, its global is defined, and the render is **never called** — the exact shape
//! cluster `C3833` (MISSING BOX: `<div>`, 32 sites, 7,544 hits) was left in at tick 696: the
//! container is emptied by the page's own client render and the re-render then performs **zero** DOM
//! operations, with nothing thrown and nothing logged. On that snapshot this takes the DOM operations
//! after the wipe from **6 to 44** and the element count from **560 to 598**.
//!
//! Served from a local socket, so the gate is hermetic and cannot false-RED on the network.

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_text::FontContext;

/// Serves exactly two scripts and 404s everything else, so nothing but a real fetch+execute can
/// satisfy the gate. `/defines.js` sets a global — assertion (3) reads it from inside the handler,
/// which is how *"the event fired"* is distinguished from *"the event fired at the right time"*.
fn script_origin() -> String {
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
                let (status, ctype, body): (&str, &str, &str) = match path.as_str() {
                    "/defines.js" => ("200 OK", "text/javascript", "window.WIDGET={ready:true};\n"),
                    "/second.js" => (
                        "200 OK",
                        "text/javascript",
                        "window.SEQ=(window.SEQ||'')+'second-ran;';\n",
                    ),
                    _ => (
                        "404 Not Found",
                        "text/html",
                        "<!doctype html><html><body>not found</body></html>",
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

fn text(page: &manuk_page::Page, sel: &str) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    assert!(!hits.is_empty(), "{sel} must exist in the document");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose** — two SpiderMonkey contexts in one binary tear down messily and the
/// binary segfaults *sometimes*, which is worse than failing (see `g_defer`).
#[test]
fn a_markup_script_that_loaded_tells_the_page_so() {
    let fonts = FontContext::new();
    let origin = script_origin();

    // `SEQ` records the ORDER things happened in, as a string, because the order is half the
    // contract: an implementation that batches every `load` to the end of the pass satisfies
    // "the event fires" and still strands the script that follows it.
    //
    // ⚠ `#seq` is rewritten on EVERY append, not once at `window.load`. Written the other way, the
    // gate reported a truncated sequence and it looked like the `error` event had stopped firing —
    // this engine fires window `load` before the injected-script drain settles, so the last entry
    // arrived after the snapshot was taken. The instrument, not the subject.
    let html = format!(
        r#"<!doctype html><html><body>
             <div id="seq">-</div>
             <div id="inline-fired">-</div>

             <script>
               window.SEQ = '';
               function S(x) {{
                 window.SEQ += x;
                 var o = document.getElementById('seq');
                 if (o) o.textContent = window.SEQ;
               }}
               // (2) A CAPTURING listener on the document. `load` on a script does not bubble, so a
               // handler can only see it here if the dispatch performs the real capture walk over the
               // element's ancestors — which a bespoke "call the element's onload property" shortcut
               // would not. Chrome fires this BEFORE the target's own attribute handler; the expected
               // string below is Chrome's, measured, not remembered.
               document.addEventListener('load', function (e) {{
                 S('capture:' + (e.target && e.target.tagName) + ';');
               }}, true);
             </script>

             <script src="{origin}/defines.js"
                     onload="S('attr-load:' + (window.WIDGET && window.WIDGET.ready) + ';')"></script>

             <script>
               // (4) An INLINE script owes NO load event. Firing at every script would satisfy every
               // other assertion here and be a different, wider bug.
               if (document.currentScript) document.currentScript.addEventListener('load', function () {{
                 document.getElementById('inline-fired').textContent = 'INLINE-LOAD-FIRED';
               }});
               S('inline-ran;');
             </script>

             <script src="{origin}/second.js"></script>

             <script src="{origin}/missing.js"
                     onload="S('404-LOAD;')" onerror="S('404-error;')"></script>
           </body></html>"#
    );

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    // `finish_loading` as well as `load_async`: the 404's `error` is reported by the injected-script
    // drain, which lives in `finish_loading`. A gate that stopped at `load_async` would never reach
    // assertion (5) and would pass by not looking.
    let page = rt.block_on(async {
        let mut p =
            manuk_page::Page::load_async(&html, &format!("{origin}/index.html"), &fonts, 800.0)
                .await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });

    let seq = text(&page, "#seq");
    println!("MARKUP SCRIPT LOAD PROBE: {seq}");

    // (1) **The event fires at all.** RED: delete the `fire_external_script_load` call from the
    // blocking loop in `PageContext::load` → `attr-load` never appears.
    assert!(
        seq.contains("attr-load:"),
        "a parser-inserted <script src> that loaded must fire `load` at its element — got {seq:?}"
    );

    // (3) **…and it fires AFTER the script ran**, so the handler sees what the script defined. RED:
    // move the dispatch to BEFORE `run_one_script` → `attr-load:undefined`. This is the assertion
    // that separates "an event was fired" from "the contract was honoured".
    assert!(
        seq.contains("attr-load:true"),
        "the `load` handler must run AFTER the script executed (window.WIDGET must be defined by \
         then) — got {seq:?}"
    );

    // (2) **The dispatch is a real event dispatch, not a property call.** `load` does not bubble, so
    // only a capture walk over the element's ancestors reaches a listener on the document — which is
    // exactly what Chrome does here (`capture:SCRIPT;` precedes `attr-load:`, measured). RED: invoke
    // the element's `onload` property directly instead of going through `__dispatchEvent` → the
    // capture entry never appears.
    assert!(
        seq.contains("capture:SCRIPT;"),
        "a capturing `load` listener on the document must see the script's load (the dispatch must \
         walk the ancestor chain, not call the property) — got {seq:?}"
    );

    // (5) **A failed fetch still reports failure, and reports it as `error`.** RED: treat every
    // external script node as loaded → `404-LOAD` appears. The success path must not swallow the
    // failure path on its way in.
    assert!(
        seq.contains("404-error;") && !seq.contains("404-LOAD"),
        "a <script src> that 404s must fire `error`, never `load` — got {seq:?}"
    );

    // (6) **In place, per script — not batched after the pass.** The first script's `load` must
    // precede the execution of the inline script that follows it, because a page is entitled to have
    // its next script see whatever the handler set up. RED: hoist the dispatch out of the loop and
    // fire the set after it → `inline-ran` precedes `attr-load`.
    let i_load = seq.find("attr-load:").expect("checked above");
    let i_inline = seq.find("inline-ran;").unwrap_or(usize::MAX);
    assert!(
        i_load < i_inline,
        "`load` must fire in place, before the NEXT script runs — got {seq:?}"
    );

    // (4) **An inline `<script>` owes no load event.** RED: fire `load` at every script rather than
    // only the external ones → this div reads `INLINE-LOAD-FIRED`.
    assert_eq!(
        text(&page, "#inline-fired"),
        "-",
        "an INLINE <script> must NOT fire a `load` event — only a script with a `src` does"
    );

    // And the document still RAN: the second external script executed. This is the guard against
    // "the load event fires because nothing else happens any more".
    assert!(
        seq.contains("second-ran;"),
        "every external script must still EXECUTE — got {seq:?}"
    );
}
