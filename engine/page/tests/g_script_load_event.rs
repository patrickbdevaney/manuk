//! **G_SCRIPT_LOAD_EVENT — a dynamically inserted `<script src>` must tell the page it arrived.**
//!
//! We fetched injected `<script src>` and executed it correctly, and then **fired nothing**. No
//! `load`, no `error`, on the element or anywhere else. A page that injects a script and waits to be
//! told it landed waited forever — and there was no error, no log, and no failing gate, because the
//! script itself had run perfectly.
//!
//! **That is the whole contract of a script loader**, and the property form is the one they use:
//! webpack's chunk loader is literally `script.onerror = script.onload = onScriptComplete` plus a
//! timer. With neither event it falls through to the timer and rejects with `ChunkLoadError`. The
//! class is far wider than bundlers — every `loadScript()` helper on the web resolves its promise
//! here: analytics, tag managers, ad tags, reCAPTCHA, payment and map SDKs. A loader with no
//! completion signal does not fail loudly; it waits, and whatever it was bootstrapping never starts.
//!
//! TWO defects, one symptom, and each is separately RED-provable:
//!   1. the dynamic-script runner dispatched no event at all; and
//!   2. `__dispatchEvent` — the ONE element dispatch point — walked only `__listeners` and never
//!      invoked the `on<type>` **property** handler. So `addEventListener('load', …)` would have
//!      worked and `s.onload = …` silently did nothing, which is exactly backwards from what the
//!      loaders in the wild use.
//!
//! **`load` fires even when the script THREW.** The event reports that fetching and running
//! happened, not that the code succeeded (a throw goes to `window.onerror`). Gating `load` on
//! success would strand every loader whose chunk has a benign error — the same bug, smaller blast
//! radius — so `threw-still-load:true` is asserted here as a rule rather than left to chance.
//!
//! ⚠ `once:1` is not decoration: it is the guard that the property handler and the listener list did
//! not BOTH fire the same registration. Inline `onclick=` attributes compile onto a marked expando
//! and register via `addEventListener`, leaving `el.onclick` undefined, so they cannot double-fire —
//! this asserts that invariant instead of trusting it.
//!
//! Served from a local socket, so the gate is hermetic and cannot false-RED on the network.

use manuk_text::FontContext;
use std::io::{Read, Write};
use std::net::TcpListener;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
  window.__log = [];
  function mark(s) {
    window.__log.push(s);
    var o = document.getElementById('out');
    if (o) o.textContent = window.__log.join(' ');
  }

  // THE WEBPACK SHAPE: the completion handler is assigned as a PROPERTY, not a listener.
  var ok = document.createElement('script');
  ok.src = "chunk.js";
  ok.onload = function () {
    window.__n = (window.__n || 0) + 1;
    mark('prop-load:true');
    mark('ran:' + (window.__ran === 1));
    mark('once:' + window.__n);
  };
  // …and a listener beside it, so a fix that moves the property handler cannot quietly break the
  // path that already worked.
  ok.addEventListener('load', function () { mark('ael-load:true'); });
  document.head.appendChild(ok);

  // A chunk that 404s must REJECT, not hang: `error` is the other half of the same contract.
  var bad = document.createElement('script');
  bad.src = "missing.js";
  bad.onerror = function () { mark('prop-error:true'); };
  document.head.appendChild(bad);

  // A script whose body THROWS still loaded. Gating `load` on success stalls the loader.
  var boom = document.createElement('script');
  boom.src = "throws.js";
  boom.onload = function () { mark('threw-still-load:true'); };
  document.head.appendChild(boom);
</script>
</body></html>"##;

const CHUNK: &str = "window.__ran = 1;\n";
const THROWS: &str = "throw new Error('benign');\n";

fn serve(path: &str) -> (u16, &'static [u8], &'static str) {
    match path {
        "/chunk.js" => (200, CHUNK.as_bytes(), "text/javascript"),
        "/throws.js" => (200, THROWS.as_bytes(), "text/javascript"),
        _ => (404, b"not found", "text/plain"),
    }
}

#[test]
fn an_injected_script_fires_load_and_error_including_the_property_handler() {
    let tmp = std::env::temp_dir().join(format!("manuk-script-load-{}", std::process::id()));
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
    // The dynamic-script phase lives in `finish_loading`, not in `load_async` — a gate that stops at
    // `load_async` never reaches the code under test and would pass by not looking.
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, &base, &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SCRIPT LOAD EVENT PROBE: {got}");

    for claim in [
        "prop-load:true",  // the `onload` PROPERTY handler — the form every loader uses
        "ran:true",        // …and the body really executed, so `load` is not a lie
        "ael-load:true",   // addEventListener still works: no regression on the path that did
        "prop-error:true", // a 404 chunk REJECTS rather than hanging
        "threw-still-load:true", // a throwing script still LOADED
        "once:1",          // …exactly once — property + listener must not double-fire
    ] {
        assert!(
            got.contains(claim),
            "G_SCRIPT_LOAD_EVENT: expected `{claim}`\n  got: {got}\n\n  \
             A dynamically inserted <script src> must fire `load` when it has been fetched and run, \
             and `error` when it cannot be fetched — on the `on<type>` PROPERTY as well as on \
             addEventListener listeners. webpack's chunk loader is `script.onerror = script.onload = \
             onScriptComplete`; with no event it falls through to its timeout and rejects with \
             ChunkLoadError, and the app it was bootstrapping never starts."
        );
    }
    assert!(
        !got.contains("once:2"),
        "G_SCRIPT_LOAD_EVENT: the load handler fired TWICE — the `on<type>` property and the \
         listener list are both invoking the same registration.\n  got: {got}"
    );
}
