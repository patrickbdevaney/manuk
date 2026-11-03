//! **G_DELIVER_DEADLINE — a `then`-handler that never returns, run by the host's `__deliver`
//! eval, is cut at the drain budget.**
//!
//! `run_deferred` arms `ScriptDeadline`, but the page-fetch pump settles a response by evaluating
//! `__deliver(...)` OUTSIDE any drain, so a handler that spins there ran unbounded (whitehouse.gov
//! `/live.json` on release: >90 s, RSS 8-9 GB). `event_loop::eval_deliver` arms the deadline for
//! all four `deliver*` entry points.
//!
//! **To watch it go RED:** delete the `ScriptDeadline::arm` line in `eval_deliver`
//! (`engine/js/src/event_loop.rs`); the page load then never returns and the guard thread aborts
//! the process with exit 7.

use manuk_text::FontContext;
use std::io::{Read, Write};
use std::net::TcpListener;

// `__deliverFetch` builds the Response with `globalThis.__makeResponse` synchronously inside the
// `__deliver` eval — before any microtask or drain. Spinning there can only be reached by a deadline
// armed around the delivery eval itself.
const HTML: &str = r##"<!doctype html><html><body><div id="d">x</div>
<script>
  fetch('/live.json').then(function () {});
  globalThis.__makeResponse = function () {
    document.getElementById('d').textContent = 'HANDLER-ENTERED';
    while (true) { /* runaway inside the delivery eval */ }
  };
</script></body></html>"##;

#[test]
fn a_runaway_then_handler_in_deliver_is_bounded_by_the_drain_budget() {
    let tmp = std::env::temp_dir().join(format!("manuk-deliverdl-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };
    unsafe { std::env::set_var("MANUK_MAX_DRAIN_MS", "500") };
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(90));
        eprintln!("GUARD: the runaway then-handler was never cut");
        std::process::exit(7);
    });

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let h = hits.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut sock) = stream else { continue };
            let h = h.clone();
            std::thread::spawn(move || {
                let mut buf = vec![0u8; 8192];
                let _ = sock.read(&mut buf);
                h.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let body = r#"{"id":"x"}"#;
                let _ = sock.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .as_bytes(),
                );
            });
        }
    });

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let fonts = FontContext::new();
    let base = format!("http://{addr}/index.html");
    let t = std::time::Instant::now();
    let text = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, &base, &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        let root = p.dom().root();
        let hit = manuk_css::query_selector_all(p.dom(), root, "#d");
        p.dom().text_content(hit[0])
    });
    let el = t.elapsed();
    println!(
        "DELIVER PROBE: {} fetches served, #d={text:?}, {el:?}",
        hits.load(std::sync::atomic::Ordering::SeqCst)
    );
    assert!(
        hits.load(std::sync::atomic::Ordering::SeqCst) >= 1,
        "VACUITY: the fixture's fetch never reached the server"
    );
    assert_eq!(
        text, "HANDLER-ENTERED",
        "VACUITY: the runaway handler never ran"
    );
    assert!(el < std::time::Duration::from_secs(30), "load took {el:?}");
}
