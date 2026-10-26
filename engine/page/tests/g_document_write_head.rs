//! **G_DOCUMENT_WRITE_HEAD — repeated `document.write` from a `<head>` script lands IN ORDER, and
//! visibly.** Each write used to go `afterend` of the running script, so W2 landed *before* W1
//! (reversed), and markup written in `<head>` was invisible (a `<div>` child of `<head>`).
//! Parser behaviour: a `<div>` in head closes head and opens body, so the writes belong in `<body>`.

use manuk_text::FontContext;

fn dump(html: &str) -> String {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(html, "https://dw.test/", &fonts, 800.0);
    let root = page.dom().root();
    let body = manuk_css::query_selector_all(page.dom(), root, "body")[0];
    page.dom()
        .text_content(body)
        .split_whitespace()
        .collect::<String>()
}

#[test]
fn script_writes_land_in_order() {
    // One #[test]: the JS runtime is per-thread and aborts if a failed assert unwinds past a live page.
    let head = dump("<html><head><script>for (var i=1;i<=3;i++) document.write('<div>W'+i+'</div>');</script></head><body>BODY</body></html>");
    let body = dump("<html><body><script>for (var i=1;i<=3;i++) document.write('<b>W'+i+'</b> ');</script>END</body></html>");
    assert_eq!(head, "W1W2W3BODY");
    assert!(body.ends_with("W1W2W3END"), "{body}"); // script source text precedes it
}
