//! # G_ESM_TLA_CYCLE — top-level await interleaves, and a module cycle resolves
//!
//! **This gate pins a capability that already worked.** The constellation carried
//! `? ESM top-level await + cyclic module records` — a named **Interop 2026** web-compat item, on
//! the grounds that *"real sites break when multiple top-level awaits or cyclic module records
//! resolve in the wrong order"*. Probed rather than planned against, and both already work on the
//! real page path. That makes this a **measure-and-pin** tick: unmeasured and working is one
//! regression away from unmeasured and broken, and nothing would say so.
//!
//! ## The probe that nearly published a false absence
//!
//! The first version drove the graph with `Page::load` + `take_fetches`, and printed `-`: nothing
//! ran. That reads exactly like "top-level await is unsupported". **The control saved it** — the
//! same graph with every `await` deleted also printed `-`, so the instrument was wrong, not the
//! subject: an external module graph is pre-fetched by `Page::load_async`, never by the page fetch
//! queue. This is the standing rule paying for itself: *before publishing an absence, name the code
//! path that would deliver it and show it ran.*
//!
//! ## Why `tick`/`two` and not "it didn't throw"
//!
//! Asserting that a TLA module merely *runs* is satisfied by an engine that ignores `await` at
//! module scope entirely. So the two async modules carry **different numbers of awaits** and stamp
//! a shared counter on completion:
//!
//! * `/tla.js` — imported FIRST, **three** awaits
//! * `/tla2.js` — imported SECOND, **one** await
//!
//! Under real async-module semantics they interleave and the shorter one finishes first, so the
//! counter reads `tick:2 two:1` — **the reverse of declaration order**. An engine that ran modules
//! synchronously in declaration order, or stripped the awaits, produces `tick:1 two:2` and every
//! other claim in this file still passes. That inversion is the whole discriminating power here.
//!
//! ## The cycle
//!
//! `/cycle-a.js` imports `/cycle-b.js`, which imports back. This is legal precisely because the
//! binding `b` reads is hoisted-but-uninitialised at evaluation time and initialised by the time a
//! *function* runs — so `a()` returning `a+A` is evidence the cycle was linked with live bindings
//! rather than snapshotted values or refused.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! | `/tla2.js` given three awaits instead of one (equal to `/tla.js`) | RED — `two:1` becomes `two:2`; the interleaving claim is what notices |
//! | the 404 arm — one module of the graph not served | RED — `ran:true` absent; the record is bare `-` |

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_text::FontContext;

const INDEX: &str = r##"<!doctype html><html><body>
  <div id="out">-</div>
  <script type="module" src="/main.js"></script>
</body></html>"##;

fn serve(path: &str) -> Option<&'static str> {
    match path {
        "/main.js" => Some(
            r#"
import { ready, tick } from '/tla.js';
import { two } from '/tla2.js';
import { a } from '/cycle-a.js';
function p(s) { var o = document.getElementById('out');
                o.textContent = (o.textContent === '-' ? '' : o.textContent + ' ') + s; }
p('ran:true');
p('tla:' + ready);
p('tick:' + tick);
p('two:' + two);
p('cycle:' + a());
"#,
        ),
        // THREE awaits — imported first, must finish LAST.
        "/tla.js" => Some(
            r#"
import { next } from '/counter.js';
await Promise.resolve();
await Promise.resolve();
export const ready = await Promise.resolve('resolved');
export const tick = next();
"#,
        ),
        // ONE await — imported second, must finish FIRST.
        "/tla2.js" => Some(
            r#"
import { next } from '/counter.js';
await Promise.resolve();
export const two = next();
"#,
        ),
        "/counter.js" => Some("let n = 0;\nexport function next() { return ++n; }\n"),
        "/cycle-a.js" => Some(
            r#"
import { b } from '/cycle-b.js';
export function a() { return 'a+' + b(); }
export const marker = 'A';
"#,
        ),
        "/cycle-b.js" => Some(
            r#"
import { marker } from '/cycle-a.js';
export function b() { return marker; }
"#,
        ),
        _ => None,
    }
}

#[test]
fn top_level_await_interleaves_and_a_module_cycle_resolves() {
    let tmp = std::env::temp_dir().join(format!("manuk-esm-tla-{}", std::process::id()));
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
                let (status, body) = match serve(&path) {
                    Some(js) => (200u16, js),
                    None => (404u16, ""),
                };
                let head = format!(
                    "HTTP/1.1 {status} {}\r\nContent-Type: text/javascript\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n",
                    if status == 200 { "OK" } else { "Not Found" },
                    body.len()
                );
                let _ = sock.write_all(head.as_bytes());
                let _ = sock.write_all(body.as_bytes());
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
    // `load_async`, NOT `load` — an external module graph is pre-fetched here and by nothing else.
    // See the module note: driving this with `take_fetches` produces `-` for a working engine.
    let page = rt.block_on(manuk_page::Page::load_async(INDEX, &base, &fonts, 800.0));

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("ESM TLA/CYCLE PROBE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_ESM_TLA_CYCLE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "ran:true",
        "the root module ran at all. A bare `-` means the graph was never pre-fetched, which is \
         what `Page::load` (rather than `load_async`) produces for a perfectly working engine — the \
         false absence this gate's own first draft nearly published",
    ),
    (
        "tla:resolved",
        "a top-level `await` produced a value a DEPENDENT module reads. If TLA were unsupported \
         this is a SyntaxError at compile and nothing above it runs either",
    ),
    (
        "two:1",
        "THE DISCRIMINATING CLAIM. `/tla2.js` is imported SECOND and has ONE await; `/tla.js` is \
         imported first and has THREE. Real async-module semantics interleave them, so the shorter \
         one stamps the shared counter FIRST. An engine that ran modules synchronously in \
         declaration order — or stripped the awaits — gives `two:2`, and every other claim here \
         still passes",
    ),
    (
        "tick:2",
        "the other half of the same inversion, asserted separately so a counter that simply never \
         advanced cannot satisfy it",
    ),
    (
        "cycle:a+A",
        "a genuine import cycle (a -> b -> a) links with LIVE bindings: `b()` reads `marker` from \
         the module that is still evaluating when `b` is linked. A cycle that was refused throws, \
         and one linked by value snapshot gives `a+undefined`",
    ),
];
