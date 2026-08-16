//! **G_STRETCH_BLOCK_AXIS — one histogram bar, TWO mechanisms, TWO functions.**
//!
//! `css/css-sizing/stretch` failed 565 assertions whose largest single shape was `height expected 45
//! but got 10`, 56 times. Read as one bar it says *"`block-size: stretch` is treated as `auto`"*.
//! A 10-arm probe written before any fix said otherwise: five of the six in-flow box types were
//! already exact. The bar was the **float** path (t1278) and the **abspos** path (t1279), and only a
//! per-box-type probe separates them. **A directory is not a cause, a message shape is not a cause,
//! and a single numeric bar is not a cause either.**
//!
//! ## The float half — a float shrink-to-fits on `auto` in BOTH axes, and `stretch` was wired to one
//!
//! `layout_float` is a second, hand-rolled box resolution living beside `layout_block`'s, and its own
//! comments say so: shrink-to-fit, `box-sizing`, aspect-ratio and the min/max clamps were each added
//! to it **one measured defect at a time**, years after `layout_block` had them. `width: stretch`
//! arrived in that sequence. `height: stretch` did not — and could not have, because the function
//! **took no containing-block height at all**.
//!
//! So a `float: left; height: stretch` card was its own content's height. For the floated card the
//! keyword exists to serve — an image, an icon, an empty coloured panel — that is border plus
//! padding and nothing else. `auto` is not a near-miss here: a float shrink-to-fits on `auto` in
//! both axes, which is the entire point of a float, so `stretch` is the *only* spelling of *"this
//! floated column is as tall as the column beside it"*.
//!
//! ⚠ **`pch` is threaded in and consumed by the `stretch` arms ONLY.** A float's percentage
//! min/max-height has its own documented behaviour — an indefinite percentage is DROPPED rather than
//! resolved against zero, because resolving against zero erased every responsive image on the page —
//! and changing it would be a second, unmeasured mechanism riding along with a measured one.
//!
//! ⭐ **AND THE SECOND MECHANISM UNDER THE SAME HISTOGRAM BAR — the ABSPOS half (tick 1279).**
//! `stretch` on an out-of-flow box does not measure the containing block. It measures the
//! **available space**: the CB's *padding* box (which is what an abspos containing block already
//! is) less the **used** insets — an `auto` inset contributes zero — and, when **both** block insets
//! are auto, less the offset to the **static position**, because that is where the box starts.
//!
//! Two clauses, and they are what make one containing block give three different answers (padding
//! box 60px, child frame 15px):
//!
//! | abspos config | available | content | border box |
//! |---|---|---|---|
//! | `inset-block: 0` | 60 | 45 | **55** — already correct, the constraint equation covers it |
//! | `position: absolute` (both auto) | 55 | 40 | **50** — the static position is the CB's *content*-box origin, 5px in |
//! | `inset-block-start: 10px` | 50 | 35 | **45** |
//! | `inset-block-end: 10px` | 50 | 35 | **45** |
//!
//! All three broken ones answered **10** before — border and padding and nothing else — because
//! `stretch` fell into the generic `auto` case, which is definite ONLY when both insets are set.
//!
//! ⚠ The static-position offset is passed as its own scalar rather than by shrinking `cb`.
//! Shrinking `cb` would silently change what every PERCENTAGE in that function resolves against.

use manuk_text::FontContext;

// `.cb` content box is 50px tall (padding box 60px). `.test` carries 2+3 = 5px of block margin, so
// a correct `block-size: stretch` gives it a BORDER box of 50 − 5 = 45.
const HTML: &str = r##"<!doctype html><html><head><style>
  .cb { block-size: 50px; inline-size: 40px; margin: 5px; border: 2px solid black;
        padding-block: 5px; padding-inline: 3px; display: inline-block; vertical-align: top }
  .auto { block-size: auto }
  .test { margin-block-start: 2px; margin-block-end: 3px; border: 3px solid blue;
          padding: 2px; block-size: stretch; inline-size: 20px }
</style></head><body>
  <!-- ── THE CLAIM. `float` is the row that moves; the rest are the in-flow shapes that already
       worked and must keep working, since they share the keyword and not the code path. -->
  <div class="cb"><div      class="test" id="float" style="float:left"></div></div>
  <div class="cb"><div      class="test" id="div"></div></div>
  <div class="cb"><canvas   class="test" id="canvas"></canvas></div>
  <div class="cb"><input    class="test" id="input"></div>
  <div class="cb"><textarea class="test" id="textarea"></textarea></div>
  <div class="cb"><button   class="test" id="button"></button></div>

  <!-- ── THE MIN/MAX PAIR on the float path, which was equally unrepresentable: `stretch` collapses
       to `Dim::Auto`, which a min reads as ZERO and a max as "no limit", so the declaration did
       nothing at all. `fmin` is content-sized (10) and must be pushed UP to 45; `fmax` asks for
       200px (210 with its border and padding) and must be pulled DOWN to 45. -->
  <div class="cb"><div class="test" id="fmin" style="float:left; block-size:auto; min-block-size:stretch"></div></div>
  <div class="cb"><div class="test" id="fmax" style="float:left; block-size:200px; max-block-size:stretch"></div></div>

  <!-- ── CONTROLS. -->
  <!-- (1) A float with `height:auto` must still shrink to its content: 0 content + 6 border +
       4 padding = 10. If this moved, `stretch` is being applied to boxes that did not ask. -->
  <div class="cb"><div class="test" id="fauto" style="float:left; block-size:auto"></div></div>
  <!-- (2) `stretch` against an INDEFINITE containing block stays content-sized — Chrome's answer,
       and `layout_block`'s. A `None` height must not become a zero-height float. -->
  <div class="cb auto"><div class="test" id="findef" style="float:left"></div></div>
  <!-- (3) A float with a specified height is untouched by any of this. ⚠ 40, not 30: `offsetHeight`
       is the BORDER box and this is content-box sizing, so the 3px border and 2px padding are on
       top of the declared 30. Written expecting 30 and corrected by the run. -->
  <div class="cb"><div class="test" id="fpx" style="float:left; block-size:30px"></div></div>
  <!-- (4) The abspos configuration that ALREADY worked, held as a control: the constraint equation
       covers `inset-block: 0` and this tick must not disturb it. -->
  <div class="cb" style="position:relative"><div class="test" id="p0" style="position:absolute; inset-block:0"></div></div>

  <!-- ── THE ABSPOS HALF. `pauto` is the row that carries the static-position clause: it must be
       50, five LESS than `p0`'s 55, because the box starts at the CB's content-box origin. -->
  <div class="cb" style="position:relative"><div class="test" id="pauto" style="position:absolute"></div></div>
  <div class="cb" style="position:relative"><div class="test" id="ps10"  style="position:absolute; inset-block-start:10px"></div></div>
  <div class="cb" style="position:relative"><div class="test" id="pe10"  style="position:absolute; inset-block-end:10px"></div></div>
  <!-- The abspos min/max pair, derived from the SAME stretch-fit: available 55 − frame 15 = 40
       content, 50 border box. `pmin` is pushed UP from 10, `pmax` pulled DOWN from 210. -->
  <div class="cb" style="position:relative"><div class="test" id="pmin" style="position:absolute; block-size:auto; min-block-size:stretch"></div></div>
  <div class="cb" style="position:relative"><div class="test" id="pmax" style="position:absolute; block-size:200px; max-block-size:stretch"></div></div>
  <!-- CONTROL: an abspos box that never said `stretch` still sits at its static position and hugs
       its content. If this moved, the new available-space arm is reaching boxes that did not ask. -->
  <div class="cb" style="position:relative"><div class="test" id="pnone" style="position:absolute; block-size:auto"></div></div>

  <div id="out">-</div><div id="gap">-</div>
  <script>
  window.addEventListener('load', function(){
    var h=function(id){ return id+'='+document.getElementById(id).offsetHeight; };
    document.getElementById('out').textContent =
      ['float','div','canvas','input','textarea','button','fmin','fmax',
       'fauto','findef','fpx'].map(h).join(' ');
    document.getElementById('gap').textContent =
      ['p0','pauto','ps10','pe10','pmin','pmax','pnone'].map(h).join(' ');
  });
  </script></body></html>"##;

fn text(page: &manuk_page::Page, sel: &str) -> String {
    let hits = manuk_css::query_selector_all(page.dom(), page.dom().root(), sel);
    assert!(!hits.is_empty(), "{sel} must exist");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn block_axis_stretch_reaches_the_float_path() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://stretch.test/",
        &fonts,
        800.0,
    ));
    // Two lines, one assertion: the in-flow/float half before the `|`, the out-of-flow half after
    // it. They are separated because they are two mechanisms in two functions under one histogram
    // bar, and reading them side by side is what said so.
    let got = format!("{} | {}", text(&page, "#out"), text(&page, "#gap"));
    println!("STRETCH-BLOCK {got}");

    // RED, run 1 — delete the `(Dim::Auto, _) if s.height_stretch` arm from `layout_float`'s height
    // match. MOVED: `float` 45 → 10. Nothing else, including every in-flow row that carries the
    // identical declaration — which is what localises the defect to the float COPY of the rule
    // rather than to the rule.
    //
    // RED, run 2 — pass `None` instead of `pch` at both `layout_float` call sites. MOVED: `float`
    // `fmin` `fmax` → their unstretched answers, 10 / 10 / **210** (the declared 200 plus this
    // box's own 6px border and 4px padding — `offsetHeight` is the border box). The wider blast
    // radius is the point: it is the missing PARAMETER, not the missing arm, and the two are
    // separately pinned.
    //
    // RED, run 3 — drop the `s.min_height_stretch` / `s.max_height_stretch` arms from the float
    // min/max clamp. MOVED: `fmin` 45 → 10 and `fmax` 45 → 210, in opposite directions, which is
    // what says the min and the max are each doing their own work rather than one of them carrying
    // both rows.
    assert_eq!(
        got,
        "float=45 div=45 canvas=45 input=45 textarea=45 button=45 fmin=45 fmax=45 \
         fauto=10 findef=10 fpx=40 | p0=55 pauto=50 ps10=45 pe10=45 pmin=50 pmax=50 pnone=10",
        "`block-size: stretch` must reach the FLOAT path. A float shrink-to-fits on `auto` in both \
         axes — that is what a float IS — so `stretch` is the only way an author can say *this \
         floated column is as tall as its column*, and `layout_float` carried no containing-block \
         height at all, so the declaration could not be resolved even in principle. The in-flow \
         rows (`div` … `button`) carry the IDENTICAL declaration through `layout_block` and were \
         already right: they are here so that a regression in the shared keyword is \
         distinguishable from a regression in the float copy. `fmin`/`fmax` move in OPPOSITE \
         directions (pushed up from 10, pulled down from 200), because `stretch` on a min collapses \
         to `Dim::Auto` which reads as zero and on a max reads as no-limit — unrepresentable \
         without its own flag. CONTROLS: `fauto` proves an ordinary float still hugs its content, \
         `findef` proves an INDEFINITE containing block leaves it content-sized rather than \
         collapsing it to zero, `fpx` proves a specified height is untouched, and `p0` is the one \
         abspos configuration that already worked and must not be disturbed, and `pnone` proves an abspos \
         box that never said `stretch` still hugs its content at its static position. \
         OUT-OF-FLOW: `stretch` measures the AVAILABLE SPACE, not the containing block — the CB's \
         padding box less the USED insets (an `auto` inset contributes zero) and, when BOTH block \
         insets are auto, less the offset to the STATIC POSITION. `p0` and `pauto` must DIFFER by \
         exactly the CB\'s block-start padding (55 vs 50), which is the whole static-position \
         clause in one comparison; all three of `pauto`/`ps10`/`pe10` answered 10 before"
    );
}
