//! **G_PROGRESS_OUTPUT_VALUE — `progress.position` and `output.value` (display-control values).**
//!
//! Two display controls whose script-facing value was missing:
//!   * `progress.position` — the completion fraction (`value/max` in `[0,1]`, or `-1` when indeterminate)
//!     a script driving an upload/download bar reads — was `undefined`.
//!   * `output.value` — an `<output>`'s value IS its displayed text content (calculators / live form
//!     results read it back after computing) — returned `""` (a dead expando that left the content
//!     untouched on assignment).
//!
//! Each claim is a way this goes RED.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<progress id="p" value="30" max="200"></progress>
<progress id="pi"></progress>
<output id="o">initial</output>
<div id="dv">x</div>
<div id="out">-</div><script>
var r=[];
function k(n,v){ r.push(n+':'+v); }
k('pos', String(document.getElementById('p').position));    // 30/200 = 0.15
k('indeterminate', String(document.getElementById('pi').position)); // -1
k('divPos', String(document.getElementById('dv').position)); // undefined (non-progress)
var o=document.getElementById('o');
k('oGet', JSON.stringify(o.value));                          // "initial"
o.value='42';
k('oSet', o.textContent+'/'+JSON.stringify(o.value));       // 42/"42"
document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn progress_position_and_output_value() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://progress-output.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "pos:0.15",         // value/max
        "indeterminate:-1", // no value attribute → -1
        "divPos:undefined", // a <div> has no .position
        "oGet:\"initial\"", // output.value is its text content
        "oSet:42/\"42\"",   // setting output.value updates the displayed content
    ] {
        assert!(
            got.contains(claim),
            "G_PROGRESS_OUTPUT_VALUE: expected {claim} in {got:?}\n  \
             progress.position must be value/max (−1 indeterminate) and output.value must be its text \
             content (read and settable)."
        );
    }
}
