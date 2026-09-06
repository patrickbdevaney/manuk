//! **G_END_PADDING_IS_AS_NECESSARY — the scrollable region's extra padding attaches to the MARGIN
//! box, and loses to a BORDER box that already reaches further.**
//!
//! CSS Overflow 3: *"additional padding … **as necessary** to enable scroll positions that satisfy
//! the requirements of both `place-content: start` and `place-content: end`"*. Surface audit #85
//! named that clause as unmeasured residue and check #136 carried it; this is it, measured.
//!
//! The reversed-axis branch of `compute_scroll_metrics` took only the BORDER-box start, so a scroller
//! whose content has no margins came out one padding short on every reversed axis — `direction: rtl`
//! and every `vertical-rl` block axis.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`), coordinates relative to the padding box:
//!
//! ```text
//!                                                        Chrome     before     after
//!   A  padding 1/4/8/16, no border, one 350x10 child, NO margins
//!      a1  ltr                    CONTROL                370/109    370/109    370/109  ✓
//!      a2  direction: rtl                                370/109    354/109    370/109
//!      a3  writing-mode: vertical-rl                     370/109    354/109    370/109
//!   B  the `negative-margin-002` wrapper, 300x300 child at `margin: -100px`
//!      b1  ltr                    CONTROL                216/201    216/201    216/201  ✓
//!      b2  direction: rtl         REGRESSION ARM         204/201    204/201    204/201
//!      b3  writing-mode: vertical-rl  REGRESSION ARM     204/201    204/201    204/201
//!   C  the `css-flexbox/negative-overflow` box, three 110px items, padding 10, gap 10
//!      c1  ltr                    CONTROL                370/130    370/130    370/130  ✓
//!      c2  direction: rtl                                370/130    360/130    370/130
//!      c3  writing-mode: vertical-rl                     130/370    120/370    130/370
//! ```
//!
//! ⭐⭐⭐ **THE A AND B BATTERIES DISAGREE ABOUT WHICH TERM WINS, AND THAT IS THE WHOLE RULE.** `a2`
//! needs the padding (`margin −234 − 16 = −250` beats `border −234`); `b2` must NOT have it
//! (`margin −4 − 16 = −20` loses to `border −104`). The negative margin is what moves the margin box
//! forward, and a fixture without margins — or without a border — cannot tell the two rules apart.
//!
//! ⚠ `b2`/`b3` are REGRESSION ARMS: they are the rows a previous attempt at this fix (t1449) broke,
//! 174 of them, and they are why that attempt was refused. The refusal was right; the contradiction
//! it reported was not — it rested on a hand-derived coordinate that t1450 measured and corrected.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>body{margin:0}
.p{width:100px;height:100px;overflow:scroll;display:block;scrollbar-width:none;padding:1px 4px 8px 16px}
.w{width:80px;height:80px;border:1px solid #d1d1d2;padding:1px 4px 8px 16px;
   border-width:1px 2px 3px 4px;border-right-width:50px;border-bottom-width:40px;
   overflow:scroll;scrollbar-width:none}
.k{margin:-100px;height:300px;width:300px}
.f{width:100px;height:100px;overflow:scroll;display:inline-flex;padding:10px;gap:10px;align-items:start;border:solid 3px;scrollbar-width:none}
.i{min-width:110px;min-height:110px}
</style></head><body>
<div class="p" id="a1"><div style="width:350px;height:10px"></div></div>
<div class="p" id="a2" style="direction:rtl"><div style="width:350px;height:10px"></div></div>
<div class="p" id="a3" style="writing-mode:vertical-rl"><div style="width:350px;height:10px"></div></div>
<div class="w" id="b1"><div class=k></div></div>
<div class="w" id="b2" style="direction:rtl"><div class=k></div></div>
<div class="w" id="b3" style="writing-mode:vertical-rl"><div class=k></div></div>
<div class="f" id="c1"><div class=i>1</div><div class=i>2</div><div class=i>3</div></div>
<div class="f" id="c2" style="direction:rtl"><div class=i>1</div><div class=i>2</div><div class=i>3</div></div>
<div class="f" id="c3" style="writing-mode:vertical-rl"><div class=i>1</div><div class=i>2</div><div class=i>3</div></div>
<div id="out">-</div>
<script>
function s(k){var e=document.getElementById(k);return k+'='+e.scrollWidth+'/'+e.scrollHeight;}
document.getElementById('out').textContent=['a1','a2','a3','b1','b2','b3','c1','c2','c3'].map(s).join(' ');
</script></body></html>"##;

#[test]
fn the_extra_padding_is_added_only_as_necessary() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://asnec.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("END PADDING AS NECESSARY: {got}");

    // ── VACUITY. The un-reversed rows must already be exact, or these rows are measuring whether the
    //    end padding exists at all rather than WHERE it attaches.
    assert!(
        got.contains("a1=370/109") && got.contains("b1=216/201") && got.contains("c1=370/130"),
        "VACUOUS: an un-reversed axis is not Chrome-exact, so the reversed rows below are not \
         measuring the attachment point — got {got:?}"
    );

    for (claim, why) in [
        ("a2=370/109", "⭐ THE MECHANISM. A child with NO margins puts its margin box at its border box, so the container's start padding extends the region: −234 − 16 = −250, and 120 + 250 = 370."),
        ("a3=370/109", "the same on a `vertical-rl` BLOCK axis — the rule is about the reversed axis, not about `direction`."),
        ("b2=204/201", "⚠⚠ REGRESSION ARM. A child at `margin: -100px` has its margin box 100px INSIDE its border box, so `−4 − 16 = −20` loses to the border box's −104 and NO padding is added: 100 + 104 = 204. 174 subtests of this shape were the reason t1449 was refused."),
        ("b3=204/201", "the `vertical-rl` twin of b2 — the losing case has to hold on both reversed axes."),
        ("c2=370/130", "a FLEX container: three 110px items and two 10px gaps reach 350, and the reversed axis adds the 10px start padding twice over the two edges → 370."),
        ("c3=130/370", "the flex box on a `vertical-rl` block axis, where the single item's 110px plus both paddings is 130."),
        ("a1=370/109", "CONTROL — the un-reversed axis, which the existing per-contribution end padding already handled."),
        ("b1=216/201", "CONTROL — and the row that proves the `as necessary` clause is not a reversed-axis special case: the border box wins here too, in the FORWARD direction."),
        ("c1=370/130", "CONTROL — the flex box un-reversed."),
    ] {
        assert!(
            got.contains(claim),
            "G_END_PADDING_IS_AS_NECESSARY: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// Z1  drop the margin-box term (the pre-tick state)
//       -> a2, a3, c2 and c3 come up one padding short; all six controls and both regression arms
//          stay green, which is what says the defect is the MISSING term and not a wrong one.
// Z2  take the margin-box term WITHOUT the `min` against the border box
//       -> b2 and b3 read 120 — the margin box is 100px inside the border box, so dropping the `min`
//          throws the border box away entirely. The `as necessary` clause deleted, and the
//          174-subtest shape t1449 was refused for.
// Z3  use `padding-right`/`padding-bottom` as the start padding
//       -> a2 reads 358: the right padding is 4 where the left is 16, which is why an ASYMMETRIC
//          fixture is required to see this at all.
// Z4  drop `start_margin` from the walk (treat every box as having none)
//       -> b2 and b3 read 220 again, by the other route: without the margin the margin box IS the
//          border box, so the padding always wins.
