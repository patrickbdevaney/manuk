//! **G_OFFSET_PARENT_BODY — `offsetLeft` subtracted the UA's `margin: 8px` from every element on
//! every ordinary page, because the BODY is not an ordinary `offsetParent`.**
//!
//! `offsetParent` returns the **body** for any element with no positioned ancestor — which is *most*
//! elements on *most* pages. CSSOM View's step 3 then says to subtract the offsetParent's **padding
//! edge**, and applied to the body that takes off the UA `margin: 8px`: an element at the top-left of
//! an ordinary document reported **0** where Chrome and Firefox both report **8**.
//!
//! ⚠⚠⚠ **This is a spec-versus-engines case, and it was settled by MEASURING CHROME, not by reading
//! harder.** The table below is `google-chrome --headless --dump-dom`, one variable per row, each row
//! in its own `<iframe>` so it gets its own body. `x` is the element's own ICB-relative position:
//!
//! ```text
//!   body style                     x    Chrome offsetLeft
//!   margin:8                        8         8
//!   margin:0                        0         0
//!   margin:8 padding:10            18        18
//!   margin:8 border:5              13        13
//!   margin:8 padding:10 border:5   23        23
//!   margin:8 position:relative      8         0    <- the discriminator
//!   margin:8 rel padding:10        18        10
//!   margin:8 rel border:5          13         5
//!   margin:8 abs border:5          13         5
//!   CONTROL — a NON-body offsetParent, the spec's own rule, already correct here:
//!   div rel border:5 padding:10    23        10    (= 23 − its padding edge 13)
//!   div rel border:5               13         0
//! ```
//!
//! Two rules fall out, **different from each other and from the spec**: a **STATIC** body subtracts
//! **nothing** (its margin, padding and border are all included — the answer is simply the
//! ICB-relative coordinate), and a **POSITIONED** body subtracts its **BORDER-BOX** origin rather
//! than its padding edge, which is why `rel border:5` reads 5 and not 0.
//!
//! ⚠⚠ **Only the body arm moved.** The two control rows prove the general path was already right, and
//! widening the "do not subtract the border" half to every offsetParent would take
//! `div rel border:5 padding:10` from 10 to 15. It is a body case, not a re-reading of step 3.
//!
//! **How it was ranked.** `css/css-flexbox`'s histogram had one bar four and a half times the next:
//! `offsetLeft` off by exactly **+8, 533 times**. t1271 dismissed that bar — correctly, on the
//! evidence it had — because grouping by DELTA aggregates unrelated mechanisms. Re-grouping the same
//! failures **by MARKUP** (t1271's own lesson) split it, and one of the pieces was uniform: the whole
//! `.a` family had **correct widths and heights** and every `offsetLeft` short by 8, i.e. an entire
//! row of floats shifted by the body margin. ⚠ *The delta was a red herring as a KEY and a true
//! signal as a VALUE — what made it readable was regrouping, not re-thresholding.*

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 /* `align-content-horiz-002`'s geometry, reduced: floats in a row, so x is the body margin + n. */
 .f { width:20px; height:60px; float:left; margin-right:2px; }
 .a { width:20px; height:10px; }
 #relp { position:relative; border:5px solid; padding:10px; clear:both; }
 #relb { position:relative; border:5px solid; clear:both; }
</style></head><body>
 <div class="f"><div class="a" id="p1"></div></div>
 <div class="f"><div class="a" id="p2"></div></div>
 <!-- CONTROLS: a NON-body offsetParent must keep subtracting its PADDING edge (spec step 3). -->
 <div id="relp"><div class="a" id="p3"></div></div>
 <div id="relb"><div class="a" id="p4"></div></div>
 <div id="out">-</div>
 <script>
 window.addEventListener('load', function(){
   var ids=['p1','p2','p3','p4'], r=[];
   for (var i=0;i<ids.length;i++){ var e=document.getElementById(ids[i]);
     r.push(ids[i]+'='+e.offsetLeft+'/'+(e.offsetParent?e.offsetParent.tagName:'null')); }
   document.getElementById('out').textContent=r.join(' ');
 });
 </script></body></html>"##;

/// **One test, on purpose** — see `g_defer`.
#[test]
fn the_body_as_offset_parent_does_not_subtract_its_own_ua_margin() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://offset.test/",
        &fonts,
        800.0,
    ));
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    let got = page.dom().text_content(hits[0]);
    println!("OFFSET-BODY {got}");

    // RED: delete the `op_is_body` arm in `el_offset_pos` → `p1=0 p2=22`, the whole row short by the
    // UA body margin, which is the state every `check-layout-th.js` suite was measuring against.
    //
    // ⚠ `p2=30` and not just `p1=8` because a single-element assertion cannot tell "add 8 once" from
    // "measure from the ICB": both give p1=8, and only the SECOND float (which sits at 8+20+2)
    // distinguishes a constant from a coordinate space.
    //
    // ⚠⚠ `p3`/`p4` are the load-bearing CONTROLS: a non-body offsetParent must still subtract its
    // PADDING edge. Widen the body rule to every offsetParent and `p3` goes 10 → 15 and `p4` 0 → 5.
    assert_eq!(
        got,
        "p1=8/BODY p2=30/BODY p3=10/DIV p4=0/DIV",
        "a STATIC body as `offsetParent` subtracts NOTHING — its margin/padding/border are all part \
         of the answer, so `offsetLeft` is simply the ICB-relative coordinate (Chrome-measured). A \
         non-body offsetParent keeps CSSOM View step 3 and subtracts its PADDING edge — got {got:?}"
    );
}
