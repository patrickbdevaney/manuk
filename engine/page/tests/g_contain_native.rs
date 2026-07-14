//! **G_CONTAIN_NATIVE — a panic inside a JS native must kill the PAGE'S CALL, not the browser.**
//!
//! Every DOM method is an `extern "C"` function, and `extern "C"` is **`nounwind`**. A Rust panic inside
//! one is *"panic in a function that cannot unwind"* → **SIGSEGV, core dumped** — **the whole browser, and
//! every tab the user had open, because one page hit one bad index.**
//!
//! That is Bar 0's founding promise inverted. WPT found it for real in tick 46: a stale `NodeId` from a
//! previous document indexed past the end of a smaller arena, inside `appendChild`, and the process died.
//! **Fixing that one index was prevention of one INSTANCE. This is containment of the CLASS.**
//!
//! **A guarantee nothing has ever tested is a hope.** So a native panics here on purpose — and the
//! assertion is that the page keeps running and the process is still alive to report it.
//!
//! Remove the `catch_unwind` at the native boundary and this test does not fail politely: **the test
//! binary ABORTS.** That is exactly the point, and it is what makes the gate falsifiable.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var R = [];
    // The native panics. Without containment, the process dies HERE and nothing below runs.
    var threw = 'no';
    try { document.__panicProbe(); R.push('returned:true'); }
    catch (e) { threw = String(e); }
    R.push('threwToJs:' + (threw === 'no'));   // contained natives return undefined; they do not throw

    // **The page must still work afterwards.** Containment that leaves the page dead is not containment.
    var d = document.createElement('div');
    d.textContent = 'still alive';
    document.body.appendChild(d);
    R.push('domStillWorks:' + (document.querySelectorAll('div').length >= 2));
    R.push('survived:true');
    document.getElementById('out').textContent = R.join(' ');
  </script></body></html>"#;

#[test]
fn a_panicking_native_does_not_take_the_browser_with_it() {
    // The probe native is registered only under this variable, so it has no production surface.
    std::env::set_var("MANUK_PANIC_PROBE", "1");

    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://contain.test/", &fonts, 800.0);

    // Reaching this line at all is half the assertion: without `catch_unwind` at the FFI edge, the panic
    // above could not unwind out of `extern "C"` and this process would already be gone.
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in ["returned:true", "threwToJs:true", "domStillWorks:true", "survived:true"] {
        assert!(
            got.contains(claim),
            "G_CONTAIN_NATIVE: expected `{claim}`\n  got: {got}\n\n  \
             A panic inside an `extern \"C\"` JS native cannot unwind — it ABORTS THE PROCESS. Every tab \
             the user had open dies because one page hit one bad index. The boundary must catch it, log \
             it loudly, return `undefined`, and let the page carry on."
        );
    }
}
