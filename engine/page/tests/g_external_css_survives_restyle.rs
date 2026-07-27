//! **G_EXTERNAL_CSS_SURVIVES_RESTYLE — a re-cascade must not strip the site's `<link>`ed CSS.**
//!
//! `Page` fetches every `<link rel="stylesheet">` and keeps the bytes in `external_css` precisely so
//! that a **later** cascade can rebuild the full sheet list. Eight re-cascade sites did not use it.
//! They rebuilt from `MinimalCascade::collect_style_elements`, which sees inline `<style>` blocks and
//! **nothing else** — so every one of them silently deleted every external stylesheet on the page and
//! re-styled the document against UA defaults.
//!
//! **That is not a subtle divergence; it is the whole design of the page gone.** Everything becomes a
//! full-width block in the default serif at 16px, stacked vertically, and the page grows several
//! times taller than it should. `keirin.jp` measured exactly that shape: 9 author sheets (375KB)
//! logged as *applied*, and then a resolved `fetch()` re-cascaded them away — coverage 97.8% (we
//! render every element Chrome does) against SHAPE **2.2%** (we place almost none of them right),
//! with our boxes reading `[8 … 1184×…] {serif/16}` where Chromium read `{Meiryo UI/…}`.
//!
//! The trigger list is the reason this is a class and not a site: a resolved `fetch`/XHR, a click, a
//! WebSocket frame, a streamed body chunk, `postMessage`, `popstate`, the deferred-script pass, and
//! the incremental relayout. **Any interaction at all, on any page whose CSS is in a `<link>`** —
//! which is essentially every page on the web.
//!
//! Two independent triggers are asserted here, because the defect was one rule with eight
//! implementations: fixing the one a gate happens to exercise leaves the other seven live.
//!
//! Served from a local socket, so the gate is hermetic and cannot false-RED on the network.

use manuk_text::FontContext;
use std::io::{Read, Write};
use std::net::TcpListener;

/// The author width is deliberately nothing like the 800px viewport, so "the external sheet applied"
/// and "the sheet was stripped and the div became a full-width block" can never be confused.
const AUTHOR_WIDTH: i64 = 321;

const HTML: &str = r##"<!doctype html><html><head>
<link rel="stylesheet" href="site.css">
</head><body>
<div id="box">styled by an EXTERNAL sheet</div>
<div id="hit">click me</div>
<div id="out">-</div>
<script>
  // A resolved `fetch()` that MUTATES the DOM — the shape of every data-driven page, and the
  // trigger that re-cascades in `resolve_fetch`.
  fetch('data.json').then(function (r) { return r.text(); }).then(function (t) {
    document.getElementById('out').textContent = 'fetched:' + t.trim();
  });
  // …and a click handler that mutates the DOM, for the second trigger.
  document.getElementById('hit').addEventListener('click', function () {
    document.getElementById('out').textContent += ' clicked';
  });
</script>
</body></html>"##;

const CSS: &str = "#box { width: 321px; height: 40px; }\n";
const DATA: &str = "ok";

fn serve(path: &str) -> (u16, &'static [u8], &'static str) {
    match path {
        "/site.css" => (200, CSS.as_bytes(), "text/css"),
        "/data.json" => (200, DATA.as_bytes(), "text/plain"),
        _ => (404, b"not found", "text/plain"),
    }
}

/// The laid-out width of `#id`, or `None` if it has no box.
fn width_of(page: &manuk_page::Page, id: &str) -> Option<i64> {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, id);
    let node = *hits.first()?;
    page.root_box
        .node_rects(page.dom())
        .get(&node)
        .map(|r| r.width.round() as i64)
}

#[test]
fn a_recascade_keeps_the_external_stylesheets_it_already_fetched() {
    let tmp = std::env::temp_dir().join(format!("manuk-extcss-{}", std::process::id()));
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
    let mut page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, &base, &fonts, 800.0).await;
        // The external-CSS phase AND the page-fetch pump both live in `finish_loading`; a gate that
        // stops at `load_async` never reaches the code under test and would pass by not looking.
        p.finish_loading(&fonts, 800.0).await;
        p
    });

    // ── PRECONDITION, asserted rather than assumed: the trigger really fired.
    //
    // If the fetch never resolved, no re-cascade ran, and a green here would mean "we did not
    // exercise the bug" — the exact shape of a gate that cannot go red.
    let out = page.dom();
    let out_node = manuk_css::query_selector_all(out, out.root(), "#out")[0];
    let text = out.text_content(out_node);
    println!(
        "EXTERNAL CSS PROBE: out={text:?} box_width={:?}",
        width_of(&page, "#box")
    );
    assert!(
        text.contains("fetched:ok"),
        "the gate's own trigger did not fire — the page's fetch never resolved into the DOM, so no \
         re-cascade happened and this test would pass without exercising anything.\n  out: {text}"
    );

    // ── TRIGGER 1 — a resolved `fetch()` that mutated the DOM (`resolve_fetch`).
    assert_eq!(
        width_of(&page, "#box"),
        Some(AUTHOR_WIDTH),
        "G_EXTERNAL_CSS_SURVIVES_RESTYLE: after a resolved fetch re-cascaded the document, `#box` is \
         no longer {AUTHOR_WIDTH}px — the external stylesheet was stripped and the div fell back to a \
         full-width UA block. `external_css` holds the bytes; the re-cascade must rebuild its sheet \
         list from `collect_style_sources` + `external_css`, never from `collect_style_elements` \
         alone (which sees inline <style> and nothing else)."
    );

    // ── TRIGGER 2 — a click that mutated the DOM (`dispatch_click_inner`). A DIFFERENT call site of
    // the same rule, because the defect was one rule with eight implementations.
    let dom = page.dom();
    let hit = manuk_css::query_selector_all(dom, dom.root(), "#hit")[0];
    page.dispatch_click(hit, &fonts, 800.0);
    let dom = page.dom();
    let out_node = manuk_css::query_selector_all(dom, dom.root(), "#out")[0];
    let after = dom.text_content(out_node);
    assert!(
        after.contains("clicked"),
        "the click trigger did not fire — the handler never ran, so this half asserts nothing.\n  \
         out: {after}"
    );
    assert_eq!(
        width_of(&page, "#box"),
        Some(AUTHOR_WIDTH),
        "G_EXTERNAL_CSS_SURVIVES_RESTYLE: the CLICK path stripped the external stylesheet. One \
         re-cascade rule, eight call sites — fixing the one a gate exercises leaves the rest live, \
         which is why two independent triggers are asserted here."
    );
}
