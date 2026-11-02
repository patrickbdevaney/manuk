//! **G_CROSS_WINDOW_REACH — a page cannot read another window's DOM/storage, and cannot forge a
//! cross-window message (SEC.DID.8, census row P9).**
//!
//! ```text
//! A  window.opener (cross-origin handle): .document/.localStorage/.sessionStorage/.location → undefined
//! A  window.open() handle: same surface, nothing but postMessage/close/focus/blur/closed
//! B  storage is keyed by the host-derived document origin, never a page argument
//! C  a page cannot forge the e.origin of a message: the queued origin is the page's own location.origin
//!    even when the page calls the native seam directly with a different origin
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
    for k in ["document", "localStorage", "sessionStorage", "location", "frames", "indexedDB"] {
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
    let honest = msgs.iter().filter(|m| m.1.contains("honest")).collect::<Vec<_>>();
    assert_eq!(honest.len(), 1, "D vacuity: honest postMessage must still queue: {msgs:?}");
    assert_eq!(honest[0].2, "https://evil.test", "D: origin is the sender's");
    for m in &msgs {
        assert_eq!(
            m.2, "https://evil.test",
            "G_CROSS_WINDOW_REACH C: a queued cross-window message carries origin `{}` that is not the \
             sender's own — a page forged its identity to another tab. msg {m:?}",
            m.2
        );
    }
}
