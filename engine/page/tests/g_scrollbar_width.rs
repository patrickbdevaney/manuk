//! **G_SCROLLBAR_WIDTH — the engine computed `scrollbar-width`, published it in the CSSOM, and then
//! reserved all 15px anyway.**
//!
//! `scrollbar-width` is `engine = "gecko"` in stylo 0.19, so the servo build we borrow does not have
//! it; this engine recovers it from `MinimalCascade` and answers `getComputedStyle(el)
//! .scrollbarWidth` correctly. ⚠⚠⚠ **And that was the whole of it.** `ScrollbarWidth`'s own doc
//! comment said *"the visible-scrollbar geometry is a paint concern this engine does not model"* —
//! true when written, false the day the gutter reservation landed in `manuk_layout`, and never
//! re-checked. So `scrollbar-width: none` reported `"none"` to any script that asked while layout
//! went on taking 15px out of the content box: **right in the one channel a person checks, wrong in
//! the one the page can see.**
//!
//! ⭐ `scrollbar-width: none` is not a niche keyword. Beside `::-webkit-scrollbar { display: none }`
//! it is the standard recipe for a horizontally-scrolling carousel, a chat pane, a code block, and
//! every custom-overlay scroll area on the modern web. Taking 15px out of those elements' content
//! width re-wraps their prose — the width-launders-into-dy shape the render burndown ranks on.
//!
//! ⚠⚠ **AND `clientWidth`/`clientHeight` HAD THE SAME BUG INDEPENDENTLY.** CSSOM-View defines them
//! as the padding box *excluding the scrollbar*; `scroll_geometry_of` was handing out the full
//! padding box for every scroll container, `scrollbar-width` or not. That is the FUNCTION half of one
//! rule: every virtualised list on the web — react-window, TanStack Virtual, every data grid —
//! divides `clientHeight` by a row height to decide how many rows to render, and a `clientHeight`
//! one scrollbar too large renders a row too many and then measures the overflow it just caused.
//! Both consumers now call `manuk_layout::scrollbar_gutter`, so they cannot drift apart.
//!
//! **THE REFERENCE IS MEASURED, NOT REASONED.** Headless Chrome on this platform, this exact
//! fixture (`google-chrome --headless --disable-gpu --dump-dom`):
//!
//! ```text
//!     scrollbar-width    child offsetWidth    clientWidth    clientHeight
//!     auto  (b1)                185                185             85
//!     none  (b2)                200                200            100
//!     thin  (b3)                190                190             90
//! ```
//!
//! `thin` is **10px, not a guess** — that row exists because inventing the number was the obvious
//! alternative and would have been a constant fitted at one point.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
.box{width:200px;height:100px;overflow:scroll}
#b2{scrollbar-width:none}
#b3{scrollbar-width:thin}
/* n1 — the CONTROL: `overflow: visible`, so there is no scrollbar to hide and
   `scrollbar-width: none` must change absolutely nothing. */
#b4{width:200px;height:100px;overflow:visible;scrollbar-width:none}
/* t4 — the PADDING half of the same box model. */
#b5{width:100px;height:100px;padding:10px;overflow:scroll}
</style></head><body>
<div class=box id=b1><div id=c1 style="width:100%;height:400px"></div></div>
<div class=box id=b2><div id=c2 style="width:100%;height:400px"></div></div>
<div class=box id=b3><div id=c3 style="width:100%;height:400px"></div></div>
<div id=b4><div id=c4 style="width:100%;height:400px"></div></div>
<div id=b5><div style="width:1px;height:100px"></div></div>
<div id=out></div><script>
var s='';
for(var i=1;i<=4;i++){
  var b=document.getElementById('b'+i), c=document.getElementById('c'+i);
  s+=c.offsetWidth+','+b.clientWidth+','+b.clientHeight+';';
}
var b5=document.getElementById('b5');
s+='pad='+b5.clientWidth+','+b5.clientHeight+','+b5.scrollWidth+','+b5.scrollHeight+';';
s+='cssom='+getComputedStyle(document.getElementById('b2')).getPropertyValue('scrollbar-width');
document.getElementById('out').textContent=s;
</script></body></html>"##;

#[test]
fn the_scrollbar_gutter_obeys_scrollbar_width_in_layout_and_in_clientwidth() {
    let fonts = FontContext::new();
    let p = manuk_page::Page::load(HTML, "http://x/", &fonts, 800.0);
    let root = p.dom().root();
    let hits = manuk_css::query_selector_all(p.dom(), root, "#out");
    assert!(!hits.is_empty(), "fixture is missing #out");
    let got = p.dom().text_content(hits[0]);
    assert!(
        got.contains(';'),
        "the fixture's script must run, or this gate measures nothing — got {got:?}"
    );

    // Every row below is headless Chrome's answer for this exact fixture, not a derivation.
    //   (label, child offsetWidth, clientWidth, clientHeight)
    let expect: [(&str, i32, i32, i32); 4] = [
        ("t1 auto  → 15px gutter", 185, 185, 85),
        ("t2 none  →  0px gutter", 200, 200, 100),
        ("t3 thin  → 10px gutter", 190, 190, 90),
        ("n1 CONTROL overflow:visible, none is inert", 200, 200, 100),
    ];
    let rows: Vec<&str> = got.split(';').collect();
    for (i, (label, ow, cw, ch)) in expect.iter().enumerate() {
        let cols: Vec<i32> = rows[i]
            .split(',')
            .map(|v| v.parse().unwrap_or(-1))
            .collect();
        assert_eq!(
            (cols[0], cols[1], cols[2]),
            (*ow, *cw, *ch),
            "{label}: expected (child offsetWidth, clientWidth, clientHeight) = ({ow}, {cw}, {ch}) \
             — Chrome's own numbers for this fixture — but got {cols:?}. A child at 185 where 200 is \
             expected means layout is still reserving a scrollbar the page said to remove; a \
             clientWidth equal to the full padding box means CSSOM-View's 'excluding the scrollbar' \
             is not being applied."
        );
    }

    // ── t4: **THE PADDING HALF OF THE SAME BOX MODEL, and it is the half that was hiding behind
    // the other one.** CSS Overflow 3 §3.1 puts the container's own END padding into the scrollable
    // overflow region, and the extent was leaving it out — invisible for as long as `clientHeight`
    // was the FULL padding box, because that wrong floor lifted the under-computed extent back to
    // the right number. Four `css/css-overflow` files were passing on the floor rather than on the
    // geometry. Chrome, this exact box (`100×100`, `padding: 10px`, `overflow: scroll`, 1×100
    // filler): `clientWidth` **105**, `clientHeight` **105**, `scrollWidth` **105**, `scrollHeight`
    // **120**.
    //
    // ⚠ Those four numbers pin BOTH rules at once and neither alone produces them: 120 is content
    // 110 plus padding-bottom 10 (the inflation), and the three 105s are the client floor after the
    // scrollbar comes out (the exclusion). A fix that did only one of the two fails this row.
    let pad = rows
        .iter()
        .find_map(|r| r.strip_prefix("pad="))
        .expect("fixture must emit the pad= row");
    let cols: Vec<i32> = pad.split(',').map(|v| v.parse().unwrap_or(-1)).collect();
    assert_eq!(
        (cols[0], cols[1], cols[2], cols[3]),
        (105, 105, 105, 120),
        "t4 padded scroller: expected (clientWidth, clientHeight, scrollWidth, scrollHeight) = \
         (105, 105, 105, 120) — Chrome's own numbers — but got {cols:?}. A scrollHeight of 110 means \
         the container's end padding is missing from the scrollable overflow region; a scrollHeight \
         of 120 alongside a clientHeight of 120 means it is only right because the floor is wrong."
    );

    // ⭐ THE HALF THAT WAS ALREADY GREEN AND MUST STAY GREEN: the CSSOM answer. It was correct
    // before this fix and is exactly why the layout bug survived — if this regresses, the fix has
    // been made by breaking the reporting instead of the geometry.
    assert!(
        got.contains("cssom=none"),
        "getComputedStyle(...).scrollbarWidth must still answer 'none' — got {got:?}"
    );
}
