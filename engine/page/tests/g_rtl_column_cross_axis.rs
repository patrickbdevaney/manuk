//! **G_RTL_COLUMN_CROSS_AXIS — a `direction: rtl` COLUMN flex container laid its items out from the
//! LEFT, and the gap was NAMED IN OUR OWN SOURCE for six hundred ticks.**
//!
//! `taffy_tree::map_direction`'s doc comment said it in its own last paragraph:
//!
//! > `column`/`column-reverse` are unchanged: their main axis is the BLOCK axis, which `direction`
//! > does not flip. (RTL does flip a column's *cross*-axis start edge, which taffy cannot express —
//! > recorded in `CONSTELLATION.tsv` rather than approximated here.)
//!
//! ⚠⚠⚠ **It is not an approximation, and it is the SAME MECHANISM the grid arm has shipped since
//! t766.** A column flex container's CROSS axis *is* the inline axis, and `direction` flips the
//! inline axis — so mirroring the placed slots against the content box is exact, not a fit. Taffy has
//! no `direction` property at all, so every logical inline axis has to be carried across that
//! boundary by hand; this is the third and last carrier, beside `map_direction`'s `row` ⇄
//! `row-reverse` swap (t764) and the grid column mirror (t766).
//!
//! **How it was ranked, which matters more than the fix.** The board directs a CSS-LAYOUT tick and
//! names the failure histogram as the ranker. `css/css-flexbox` (36.2%, the top ★ lever) histogrammed
//! to **1,285 `offsetLeft` failures, 533 of them off by exactly +8** — and *that number was a trap*:
//! ⚠ **A HISTOGRAM ROW IS A SUSPECT, NOT A DEFECT.** Grouping the same failures by their MARKUP
//! instead of by their delta split the +8 into several unrelated families, and only one of them —
//! WPT's `flex-abspos-staticpos-align-self-rtl-*` — had a single mechanism under it. `8` was just a
//! common free-space width.
//!
//! ⚠⚠ **The first reduction was GREEN, and that is what redirected the tick.** A `flex-flow: column`
//! container with the whole `align-self` keyword family reproduced *nothing* — all eight values
//! already Chrome-exact. Adding `direction: rtl` to the same fixture changed **none of our numbers**,
//! which is the actual defect: not a wrong answer, an axis that was never consulted.
//!
//! Measured against WPT's authored (i.e. Chrome's) expectations, `flex-flow: column; direction: rtl`,
//! a 16px content box holding an 8px child:
//!
//! ```text
//!   align-self                       Chrome    before    after
//!   stretch / normal / auto            10        2        10
//!   start / self-start / flex-start    10        2        10
//!   end   / self-end   / flex-end       2       10         2
//!   center                              6        6         6   ← already right: this is a MIRROR
//! ```
//!
//! ⚠ **`center` being right all along is the reason the bug survived**: the commonest RTL column
//! idiom on the web is a centred stack, and a mirror is the identity on a centred box. The failure
//! only shows on a container whose items are aligned to an EDGE.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 /* WPT's `flex-abspos-staticpos-align-self-rtl-003` geometry, reduced: content box 16 wide, child 8. */
 .c { display:flex; flex-flow:column; direction:rtl; position:relative;
      padding:1px 2px; border:1px solid black; height:10px; width:16px; }
 .c > * { position:absolute; height:6px; width:8px; }
 .ltr { direction:ltr; }
 .row { flex-flow:row; }
 .inflow > * { position:static; }
</style></head><body>
 <div class="c"><div id="s1" style="align-self:stretch"></div></div>
 <div class="c"><div id="s2" style="align-self:start"></div></div>
 <div class="c"><div id="s3" style="align-self:self-start"></div></div>
 <div class="c"><div id="s4" style="align-self:flex-start"></div></div>
 <div class="c"><div id="e1" style="align-self:end"></div></div>
 <div class="c"><div id="e2" style="align-self:self-end"></div></div>
 <div class="c"><div id="e3" style="align-self:flex-end"></div></div>
 <div class="c"><div id="ct" style="align-self:center"></div></div>
 <!-- The IN-FLOW arm: the same rule, and the half that is what real RTL pages actually contain. -->
 <div class="c inflow"><div id="if1" style="align-self:flex-start"></div></div>
 <div class="c inflow"><div id="if2" style="align-self:flex-end"></div></div>
 <!-- CONTROLS. `lf1` is the LTR column (the mirror must not fire); `rr`/`rl` are RTL/LTR ROW
      containers, whose main axis is already carried by `map_direction`'s row-reverse swap and which
      must therefore NOT be mirrored a second time. -->
 <div class="c ltr inflow"><div id="lf1" style="align-self:flex-start"></div></div>
 <div class="c row"><div id="rr"></div></div>
 <div class="c row ltr"><div id="rl"></div></div>
 <div id="out">-</div>
 <script>
 window.addEventListener('load', function(){
   var ids=['s1','s2','s3','s4','e1','e2','e3','ct','if1','if2','lf1','rr','rl'], r=[];
   for (var i=0;i<ids.length;i++){ var e=document.getElementById(ids[i]);
     r.push(ids[i]+'='+e.offsetLeft); }
   document.getElementById('out').textContent=r.join(' ');
 });
 </script></body></html>"##;

/// **One test, on purpose** — see `g_defer`.
#[test]
fn an_rtl_column_flex_container_starts_its_cross_axis_at_the_right_edge() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://rtl.test/",
        &fonts,
        800.0,
    ));
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    let got = page.dom().text_content(hits[0]);
    println!("RTL-COLUMN-CROSS {got}");

    // RED: make `Ctx::container_inline_axis_is_mirrored` answer `false` for a flex container (its
    // pre-t1271 body was a grid-only predicate) → every `s*` reads 2 and every `e*` reads 10, the
    // exact inversion of Chrome, on all seven cross-axis keywords at once.
    //
    // ⚠ The keywords are asserted as THREE GROUPS rather than one string because they reach the
    // engine by three different routes — `stretch`/`normal`/`auto` is the DEFAULT path (no
    // `align-self` resolution at all), `start`/`self-start` are the writing-mode-relative spellings
    // Stylo maps through `AlignFlags` 2/12, and `flex-start` is the flex-relative one (flag 4). A
    // single assertion over all of them would pass on a fix that only reached one route.
    assert_eq!(
        got,
        "s1=10 s2=10 s3=10 s4=10 e1=2 e2=2 e3=2 ct=6 if1=10 if2=2 lf1=2 rr=10 rl=2",
        "an RTL COLUMN flex container's cross axis is the INLINE axis and starts at the RIGHT edge: \
         every cross-start keyword must read 10 and every cross-end keyword 2, for an ABSPOS static \
         position and an IN-FLOW item alike. `ct=6` is the centred case, which a mirror leaves alone. \
         CONTROLS that must not move: `lf1=2` (LTR column — the mirror must not fire) and `rr=10` / \
         `rl=2` (RTL/LTR ROW, whose axis `taffy_tree::map_direction` already carries by swapping \
         `row` for `row-reverse` — mirroring those too would apply the flip TWICE and land back \
         where it started)"
    );
}
