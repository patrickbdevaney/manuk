//! **G_CROSS_WINDOW_REACH — a page cannot read another window's DOM/storage, and cannot forge a
//! cross-window message (SEC.DID.8, census row P9).**
//!
//! ```text
//! A  window.opener (cross-origin handle): .document/.localStorage/.sessionStorage/.location → undefined
//! A  window.open() handle: same surface, nothing but postMessage/close/focus/blur/closed
//! B  storage is keyed by the host-derived document origin, never a page argument
//! C  a page cannot forge the e.origin of a message: the queued origin is the page's own location.origin
//!    even when the page calls the native seam directly with a different origin
//! E  a page cannot post to a window id it holds no handle to (guessed id): nothing is queued
//! F  a page that never called window.open cannot post to a tab another page opened, nor to any
//!    guessed id — handles are per document, not per thread (the shell runs every tab on one thread)
//! G  `event.source` is the host-assigned window id: a page that overwrites `__winId` (or passes a
//!    different source to the native seam) still queues its true id (SEC.DID.8.b)
//! D  vacuity: an honest window.open().postMessage still queues, with the sender's real origin
//! ```
use manuk_page::Page;

fn read(page: &Page) -> String {
    let dom = page.dom();
    manuk_css::query_selector_all(dom, dom.root(), "#__r__")
        .first()
        .map(|&n| dom.text_content(n))
        .unwrap_or_default()
}

#[test]
fn a_page_cannot_read_or_forge_across_windows() {
    let fonts = manuk_text::FontContext::new();

    // ── A: a handle to another window exposes no DOM or storage. ───────────────────────────────────
    let html = "<!doctype html><html><body><script>\
        var h = window.open('https://bank.test/', 'w');\
        var o = []; \
        ['document','localStorage','sessionStorage','location','frames','indexedDB'].forEach(function (k) {\
          try { o.push(k + '=' + (typeof h[k])); } catch (e) { o.push(k + '=' + e.name); }\
        });\
        var pre = document.createElement('pre'); pre.id = '__r__'; pre.textContent = o.join('|');\
        document.body.appendChild(pre);\
        </script></body></html>";
    let page = Page::load(html, "https://evil.test/", &fonts, 800.0);
    let got = read(&page);
    println!("XW A: {got}");
    for k in [
        "document",
        "localStorage",
        "sessionStorage",
        "location",
        "frames",
        "indexedDB",
    ] {
        assert!(
            got.contains(&format!("{k}=undefined")),
            "G_CROSS_WINDOW_REACH A: a window handle exposes `{k}` — a cross-origin read. got {got}"
        );
    }

    // ── C: forging the sender origin through the native seam. ──────────────────────────────────────
    let html = "<!doctype html><html><body><script>\
        var h = window.open('https://bank.test/', 'w');\
        h.postMessage({honest: 1}, '*');\
        try { __postMessage('4242', '{\"forged\":1}', 'https://bank.test', '0'); } catch (e) {}\
        try { __makeWindowRef(4242).postMessage({guess: 1}, '*'); } catch (e) {}\
        </script></body></html>";
    let page = Page::load(html, "https://evil.test/", &fonts, 800.0);
    let msgs = page.take_messages();
    println!("XW C: {msgs:?}");
    let honest = msgs
        .iter()
        .filter(|m| m.1.contains("honest"))
        .collect::<Vec<_>>();
    assert_eq!(
        honest.len(),
        1,
        "D vacuity: honest postMessage must still queue: {msgs:?}"
    );
    assert_eq!(
        honest[0].2, "https://evil.test",
        "D: origin is the sender's"
    );
    // E (SEC.DID.8.a): posting to ids the page holds no handle to (guessed, via the native seam or a
    // hand-made ref) queues nothing — only the honest window.open handle's message is queued.
    assert_eq!(
        msgs.len(),
        1,
        "G_CROSS_WINDOW_REACH E: a page posted to a window id it holds no handle to: {msgs:?}"
    );
    for m in &msgs {
        assert_eq!(
            m.2, "https://evil.test",
            "G_CROSS_WINDOW_REACH C: a queued cross-window message carries origin `{}` that is not the \
             sender's own — a page forged its identity to another tab. msg {m:?}",
            m.2
        );
    }

    // ── F (SEC.DID.8.a): tab X opens tab B and holds its handle; tab Y never calls window.open. ────
    let html = "<!doctype html><html><body><script>\
        window.open('https://bank.test/', 'b').postMessage({fromX: 1}, '*');\
        </script></body></html>";
    let x = Page::load(html, "https://app.test/", &fonts, 800.0);
    let xs = x.take_messages();
    assert_eq!(
        xs.len(),
        1,
        "F vacuity: X's real handle to B must queue: {xs:?}"
    );
    let b = xs[0].0;
    let html = format!(
        "<!doctype html><html><body><script>\
         for (var i = 1; i <= {hi}; i++) {{\
           try {{ __postMessage(String(i), '{{\"guessY\":1}}', 'https://evil.test', '0'); }} catch (e) {{}}\
           try {{ __makeWindowRef(i).postMessage({{guessY: 1}}, '*'); }} catch (e) {{}}\
         }}\
         </script></body></html>",
        hi = b + 8
    );
    let y = Page::load(&html, "https://evil.test/", &fonts, 800.0);
    let ys = y.take_messages();
    println!("XW F: tab B = win {b}; Y queued {ys:?}");
    assert!(
        ys.is_empty(),
        "G_CROSS_WINDOW_REACH F: a page that never called window.open posted to tab ids it holds no \
         handle to (tab B = win {b}): {ys:?}"
    );

    // ── G (SEC.DID.8.b): the queued source is the host-assigned id, not the page-writable __winId. ──
    manuk_page::set_pending_identity(777, 0);
    let html = "<!doctype html><html><body><script>\
        var h = window.open('https://bank.test/', 'b');\
        window.__winId = 4242;\
        h.postMessage({spoof: 1}, '*');\
        __winId = 31337;\
        h.postMessage({spoof: 2}, '*');\
        </script></body></html>";
    let g = Page::load(html, "https://evil.test/", &fonts, 800.0);
    let gs = g.take_messages();
    println!("XW G: queued {gs:?}");
    assert_eq!(
        gs.len(),
        2,
        "G vacuity: both honest-handle posts must queue: {gs:?}"
    );
    for m in &gs {
        assert_eq!(
            m.3, 777,
            "G_CROSS_WINDOW_REACH G: event.source is the page-writable __winId, not the host-assigned \
             id 777 — a page can impersonate another window as the reply target. msg {m:?}"
        );
    }
}
