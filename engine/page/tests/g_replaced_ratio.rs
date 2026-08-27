//! **G_REPLACED_RATIO — a `<canvas>`'s dimension attributes are an INTRINSIC size and its ratio
//! transfers through a clamp; an `<img>`'s are a PRESENTATIONAL HINT and its height does not.**
//!
//! ⚠⚠⚠ **THIS GATE ASSERTED A NUMBER CHROME DOES NOT PRODUCE, AND CALLED A FIX A REGRESSION.**
//! Every claim below was re-measured against `google-chrome --headless=new` at t1345, on this exact
//! fixture, with `getBoundingClientRect`. Its original `#i` row reasoned — in prose, citing CSS2.1
//! §10.4 — that *"an 800x400 `<img>` under `max-width:100%` in a 400px column is 400x200"*. It is
//! **400x400**. The gate had never been run against the oracle; it was derived, and it spent the
//! ticks after the engine reached Chrome's answer reporting that as a regression.
//!
//! ```text
//!                                                            CHROME    this gate asserted
//!   #i  <img width=800 height=400>       max-width:100%      400x400        400x200   ✗
//!   #s  …the same WITH a real src        max-width:100%      400x400          —
//!   #n  …the same                        max-width:200px     200x400          —
//!   #c  <canvas width=800 height=400>    max-width:100%      400x200        400x200   ✓
//!   #z  <canvas width=15 height=15>      max-width:0px           0x0            0x0   ✓
//!   #u  <img width=800 height=400>       max-width:none      800x400        800x400   ✓
//! ```
//!
//! ## The asymmetry is the mechanism, and it is in HTML, not CSS
//!
//! Both elements carry `width`/`height` attributes and both end up with a ratio, but the attributes
//! mean **different things**:
//!
//! - **`<canvas width height>` are the element's INTRINSIC dimensions** — the backing store's real
//!   size. `width` and `height` in CSS stay `auto`, the box is sized from the intrinsic size, and a
//!   `max-width` clamp is therefore a §10.4 constraint violation whose adjustment is proportional:
//!   400 wide → **200 tall**.
//! - **`<img width height>` are PRESENTATIONAL HINTS** — they map to the CSS `width`/`height`
//!   properties. `height:400px` is a **specified, definite** height, and §10.4's ratio transfer
//!   applies to the axis that is `auto`, not to one the author (or the UA sheet, acting for the
//!   author) has already given a length. So the clamp narrows the width and leaves the height at
//!   400: **400x400**, and `max-width:200px` gives **200x400**, not 200x100.
//!
//! ⚠ `#s` is why this is not an artefact of the missing bitmap. A real decoded `src` — whose
//! intrinsic size would supply a ratio all by itself — still renders 400x400, because the
//! presentational-hint height outranks the intrinsic one. The absent-`src` and present-`src` rows
//! answering the SAME number is what rules out "the attributes gave no ratio at all".
//!
//! ⚠⚠ **The anti-layout-shift story the old header told is still true and is still what these
//! attributes are for** — `<img width height>` does reserve the right box before the bitmap arrives.
//! It reserves it by being a definite width AND a definite height, which is a stronger guarantee
//! than a ratio, not a weaker one. What was wrong was the claim about what happens when a clamp then
//! binds one of them.

use manuk_text::FontContext;

/// A 800x400 PNG, inline, so `#s` has a real decoded intrinsic size to compete with its attributes.
const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAyAAAAGQCAIAAAB+HHsxAAAAG0lEQVR4nO3BMQEAAADCoPVPbQ0PoAAAAAAA4G8YMAABDT1lqQAAAABJRU5ErkJggg==";

fn html() -> String {
    format!(
        r##"<!doctype html><html><body style="margin:0">
<style>
  .col {{ width: 400px }}
  img, canvas {{ max-width: 100% }}
</style>
<div id="out">-</div>
<div class="col"><img id="i" width="800" height="400"></div>
<div class="col"><canvas id="c" width="800" height="400"></canvas></div>
<div class="col"><canvas id="z" width="15" height="15" style="max-width:0px"></canvas></div>
<div class="col"><img id="u" width="800" height="400" style="max-width:none"></div>
<div class="col"><img id="s" src="{PNG}" width="800" height="400"></div>
<div class="col"><img id="n" width="800" height="400" style="max-width:200px"></div>
<script>
  var R = [];
  var r = function (id) {{ return document.getElementById(id).getBoundingClientRect(); }};
  ['i','c','z','u','s','n'].forEach(function (id) {{
    R.push(id + ':' + Math.round(r(id).width) + 'x' + Math.round(r(id).height));
  }});
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##
    )
}

#[test]
fn a_clamped_replaced_element_keeps_the_ratio_its_attributes_declare() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(&html(), "https://grr.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "c:400x200",
            "⭐ THE LOAD-BEARING ROW. A <canvas> has NO decoded bitmap, so its ratio can only have \
             come from the width/height ATTRIBUTES — and because those attributes are its INTRINSIC \
             size (not a CSS `height`), `max-width:100%` in a 400px column is a §10.4 constraint \
             violation and the height follows proportionally. 400x400 means the transfer did not \
             fire; 400x0 means the attributes gave no ratio at all",
        ),
        (
            "z:0x0",
            "`max-width:0` on a 15x15 canvas collapses BOTH axes: §10.4's adjustment is \
             proportional, so a zero used width forces a zero used height",
        ),
        (
            "i:400x400",
            "⚠ CHROME-MEASURED, AND IT IS NOT 400x200. `<img width height>` are PRESENTATIONAL \
             HINTS onto the CSS `width`/`height` properties, so `height:400px` is a SPECIFIED \
             height and the clamp has no `auto` axis to transfer into. Reading 400x200 here means \
             an <img>'s attribute height is being treated as an intrinsic dimension the way a \
             <canvas>'s is — which is the exact conflation this gate exists to keep apart",
        ),
        (
            "s:400x400",
            "…and the same <img> WITH a real decoded 800x400 `src` is also 400x400, not 400x200. \
             The presentational-hint height outranks the intrinsic one. This row is what stops \
             `#i` being read as 'the attributes gave no ratio at all'",
        ),
        (
            "n:200x400",
            "a DIFFERENT clamp value on the same element: `max-width:200px` gives 200x400, not \
             200x100. Two clamp widths against one unchanged height is a proportional transfer's \
             clearest refutation — a ratio would have moved the height twice",
        ),
        (
            "u:800x400",
            "and with the clamp removed the element keeps its attribute size unchanged — the \
             transfer fires on a constraint VIOLATION, it does not rewrite unclamped boxes",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_REPLACED_RATIO: expected `{claim}` — {why}.\n  got: {got}"
        );
    }
}
