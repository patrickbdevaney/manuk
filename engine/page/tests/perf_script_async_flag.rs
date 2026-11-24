//! PERF.1.2.b.b.a.a: the page crate sees each pending dynamic script's `async` state.
//! `createElement('script')` is force-async; `s.async = false` clears it.
#![cfg(feature = "spidermonkey")]
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8></head><body><script>
var a = document.createElement('script'); a.src = 'https://example.invalid/forced.js';
var b = document.createElement('script'); b.async = false; b.src = 'https://example.invalid/ordered.js';
document.body.appendChild(a); document.body.appendChild(b);
</script></body></html>"##;

#[test]
fn pending_dynamic_scripts_report_async_state() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://example.invalid/page.html",
        &fonts,
        800.0,
    ));
    let got = page.pending_dynamic_script_async_flags();
    assert_eq!(
        got,
        vec![
            ("https://example.invalid/forced.js".to_string(), true),
            ("https://example.invalid/ordered.js".to_string(), false)
        ]
    );
}
