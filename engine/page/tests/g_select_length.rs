//! **G_SELECT_LENGTH — `select.length` is the option count, and `select.length = n` resizes the list.**
//!
//! `select.length` returned `0` — the `length` property was wired to the CharacterData text length, which
//! is `0` for a non-text node. So the option count was invisible and the classic `select.length = 0`
//! "clear the dropdown" idiom (and `select.length = n` resize) did nothing. Each claim is a way this
//! goes RED:
//!
//!   * `select.length` counts the options (descendants, so options inside an `<optgroup>` count).
//!   * `select.length = 1` truncates — trailing options are removed from their own parents.
//!   * `select.length = 3` grows — bare `<option>` elements are appended.
//!   * a Text node's `.length` (CharacterData) is UNCHANGED — the overload must not break it.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<select id="s"><option>a</option><optgroup><option>b</option></optgroup><option>c</option></select>
<p id="tx">hello</p>
<div id="out">-</div><script>
var r=[];
function k(n,v){ r.push(n+':'+v); }
var s=document.getElementById('s');
k('get', String(s.length));                 // 3 (incl. the option inside the optgroup)
s.length=1;
k('trunc', s.options.length+'/'+s.options[0].textContent); // 1/a
s.length=3;
k('grow', s.options.length);                 // 3 (two bare options appended)
k('txt', document.getElementById('tx').firstChild.length); // CharacterData length still 5
document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn select_length_counts_and_resizes_options() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://select-length.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "get:3",     // option count, not 0
        "trunc:1/a", // length=1 removed b and c
        "grow:3",    // length=3 appended empty options
        "txt:5",     // CharacterData.length overload preserved
    ] {
        assert!(
            got.contains(claim),
            "G_SELECT_LENGTH: expected {claim} in {got:?}\n  \
             select.length must be the option count and be settable to truncate/extend the list, \
             without breaking CharacterData.length."
        );
    }
}
