//! **G_OPTIONS_LENGTH — `select.options.length = n` resizes the LIVE dropdown (the "clear it" idiom).**
//!
//! `select.options` hands back a fresh Array decorated with the HTMLOptionsCollection methods
//! (`namedItem`/`add`/`remove`). Its `.length` was a plain Array length, so the single most common
//! way old-school / non-framework code empties a dependent dropdown —
//!
//! ```js
//! sel.options.length = 0;                 // country picker: clear before repopulating
//! for (const c of countries) sel.add(new Option(c.name, c.code));
//! ```
//!
//! — truncated the SNAPSHOT the getter had just returned and left the DOM completely untouched: the
//! `<option>`s stayed, the next read of `sel.options` handed them right back, and the "cleared"
//! dropdown showed every stale row on top of the freshly-added ones. A dead expando.
//!
//! `HTMLOptionsCollection.length` is a LIVE writable accessor. Setting it now routes to the select's
//! own option list (truncate removes trailing options; grow appends bare `<option>`s), so the idiom
//! actually clears/resizes the control. Cascading `<select.options.length = 0>` cascades correctly.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `clear` — `sel.options.length = 0` empties the DOM: `querySelectorAll('option')` is now 0 AND a
//!     fresh `sel.options.length` reads 0. RED: unwrap the Proxy (return the bare decorated Array) and
//!     the assignment truncates only the snapshot — the DOM keeps all three options, so `clear` reads 3.
//!   * `grow` — `sel.options.length = 3` on a 1-option select appends two bare options (DOM count 3).
//!   * `repopulate` — after clearing, `sel.add(new Option())` starts from an empty list, so a country
//!     picker rebuilt from scratch has exactly the new rows, not the new rows stacked on the stale ones.
//!   * `read-live` — reading `sel.options.length` reflects the live option count, not a stale snapshot.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<select id="s"><option>a</option><optgroup><option>b</option></optgroup><option>c</option></select>
<select id="g"><option>only</option></select>
<select id="rp"><option>stale1</option><option>stale2</option></select>
<div id="out">-</div><script>
var r=[];
function k(n,v){ r.push(n+':'+v); }

var s=document.getElementById('s');
k('before', String(s.options.length));            // 3
s.options.length = 0;                             // the clear-the-dropdown idiom
// the DOM itself must be empty, AND a fresh collection read must agree
k('clear', s.querySelectorAll('option').length + '/' + s.options.length); // 0/0

var g=document.getElementById('g');
g.options.length = 3;                             // grow: append two bare options
k('grow', g.querySelectorAll('option').length);   // 3

var rp=document.getElementById('rp');
rp.options.length = 0;                            // clear...
var o1=document.createElement('option'); o1.value='US'; rp.add(o1);
var o2=document.createElement('option'); o2.value='CA'; rp.add(o2);
// exactly the two fresh rows — not stacked on the two stale ones
k('repopulate', rp.options.length + '/' + rp.options[0].value + ',' + rp.options[1].value); // 2/US,CA

k('readLive', String(rp.options.length));         // 2, live

document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn options_collection_length_setter_resizes_the_live_dropdown() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://options-length.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("OPTIONS LENGTH RESULT: {got}");

    for claim in [
        "before:3",           // three options to start (incl. the one inside the optgroup)
        "clear:0/0", // length=0 emptied the LIVE DOM, and a fresh read agrees — the whole bug
        "grow:3",    // length=3 appended two bare options
        "repopulate:2/US,CA", // clear-then-add rebuilt from empty, not stacked on stale rows
        "readLive:2", // length reads the live option count
    ] {
        assert!(
            got.contains(claim),
            "G_OPTIONS_LENGTH: expected {claim} in {got:?}\n  \
             select.options.length must be a LIVE writable accessor: setting it truncates/extends the \
             select's real option list (the `sel.options.length = 0` clear-the-dropdown idiom), not a \
             throwaway snapshot Array."
        );
    }
}
