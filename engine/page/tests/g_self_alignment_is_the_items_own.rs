//! **G_SELF_ALIGNMENT_IS_THE_ITEMS_OWN — `self-start` / `self-end` resolve in the ITEM's writing
//! mode, and we resolved them in the container's.**
//!
//! CSS Box Alignment §4 draws the distinction explicitly: `start`/`end` are relative to the alignment
//! **container**'s axes, `self-start`/`self-end` to the alignment **subject**'s own. Stylo hands both
//! spellings to our mapper as `FlexStart`/`FlexEnd` — right about the EDGE, silent about whose axes
//! name it — so a grid item whose own `direction` or `writing-mode` differed from its grid's was
//! aligned to the wrong side of its cell. (The hand-rolled cascade did worse: it did not parse the
//! keywords at all, so they fell through to `auto` and the item deferred to its container.)
//!
//! ⭐ **The AXIS is chosen by the container and the SIDE is chosen by the item.** That is the whole
//! rule in one sentence, and it is why neither box's style is enough on its own.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`), a 20x20 `horizontal-tb; ltr` inline-grid
//! holding a 10x10 child; the pair is the child's offset from the grid's border box:
//!
//! ```text
//!                                                          Chrome    before    after
//!   r1  child htb  ltr, self-start          CONTROL         0,0       0,0       0,0   ✓
//!   r2  child htb  ltr, self-end            CONTROL        10,10     10,10     10,10  ✓
//!   r3  child htb  RTL, self-start                         10,0       0,0      10,0
//!   r4  child htb  RTL, self-end                            0,10     10,10      0,10
//!   r5  child v-lr ltr, self-start          CONTROL         0,0       0,0       0,0   ✓
//!   r6  child v-lr RTL, self-start                          0,10      0,0       0,10
//!   r7  child v-RL ltr, self-start                         10,0       0,0      10,0
//!   r8  child htb  ltr, plain `start`       CONTROL         0,0       0,0       0,0   ✓
//!   r9  child htb  ltr, plain `end`         CONTROL        10,10     10,10     10,10  ✓
//!   ra  RTL grid, RTL child, self-start     CONTROL        10,0      10,0      10,0   ✓
//!   rb  RTL child, plain `start`            CONTROL         0,0       0,0       0,0   ✓
//!   rc  RTL child, `center`                 CONTROL         5,5       5,5       5,5   ✓
//! ```
//!
//! ⭐⭐ **`r6` AND `r7` ARE WHAT MAKE THIS AN AXIS RULE RATHER THAN A `direction` RULE.** `r6` flips on
//! the BLOCK axis because a `vertical-lr` child's INLINE axis runs down the screen and `rtl` reverses
//! it; `r7` flips on the INLINE axis because a `vertical-rl` child's BLOCK axis runs right-to-left
//! with no `direction` involved at all. A fix written as "flip when the child is rtl" passes `r3`,
//! `r4` and `r6` and fails `r7`.
//!
//! ⚠ **`rb` IS THE ROW THAT KEEPS THE SPELLING LOAD-BEARING.** The same RTL child asking for plain
//! `start` must NOT move: `start` is the container's start, and the container is `ltr`. Without this
//! row the rule could be written as "an RTL child in an LTR grid is reversed" and still pass
//! everything else.
//!
//! ⚠ `ra` is the both-RTL row: two reversals that agree are no reversal, and it is what stops a fix
//! that keys on the ITEM alone rather than on the PAIR.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>body{margin:0;line-height:30px}
.grid{position:relative;display:inline-grid;grid-template-columns:20px;grid-template-rows:20px;width:20px;height:20px;vertical-align:bottom}
.child{background:green;width:10px;height:10px}
</style></head><body>
<div class="grid" id="c1"><div class="child" id="k1" style="writing-mode:horizontal-tb;direction:ltr;align-self:self-start;justify-self:self-start"></div></div>
<div class="grid" id="c2"><div class="child" id="k2" style="writing-mode:horizontal-tb;direction:ltr;align-self:self-end;justify-self:self-end"></div></div>
<div class="grid" id="c3"><div class="child" id="k3" style="writing-mode:horizontal-tb;direction:rtl;align-self:self-start;justify-self:self-start"></div></div>
<div class="grid" id="c4"><div class="child" id="k4" style="writing-mode:horizontal-tb;direction:rtl;align-self:self-end;justify-self:self-end"></div></div>
<div class="grid" id="c5"><div class="child" id="k5" style="writing-mode:vertical-lr;direction:ltr;align-self:self-start;justify-self:self-start"></div></div>
<div class="grid" id="c6"><div class="child" id="k6" style="writing-mode:vertical-lr;direction:rtl;align-self:self-start;justify-self:self-start"></div></div>
<div class="grid" id="c7"><div class="child" id="k7" style="writing-mode:vertical-rl;direction:ltr;align-self:self-start;justify-self:self-start"></div></div>
<div class="grid" id="c8"><div class="child" id="k8" style="align-self:start;justify-self:start"></div></div>
<div class="grid" id="c9"><div class="child" id="k9" style="align-self:end;justify-self:end"></div></div>
<div class="grid" id="ca" style="direction:rtl"><div class="child" id="ka" style="direction:rtl;align-self:self-start;justify-self:self-start"></div></div>
<div class="grid" id="cb"><div class="child" id="kb" style="direction:rtl;align-self:start;justify-self:start"></div></div>
<div class="grid" id="cc"><div class="child" id="kc" style="direction:rtl;align-self:center;justify-self:center"></div></div>
<div id="out">-</div>
<script>
function o(c,k){var a=document.getElementById(c).getBoundingClientRect(),b=document.getElementById(k).getBoundingClientRect();
 return Math.round(b.left-a.left)+','+Math.round(b.top-a.top);}
document.getElementById('out').textContent=
 'r1='+o('c1','k1')+' r2='+o('c2','k2')+' r3='+o('c3','k3')+' r4='+o('c4','k4')+' r5='+o('c5','k5')
 +' r6='+o('c6','k6')+' r7='+o('c7','k7')+' r8='+o('c8','k8')+' r9='+o('c9','k9')+' ra='+o('ca','ka')+' rb='+o('cb','kb')+' rc='+o('cc','kc');
</script></body></html>"##;

#[test]
fn self_start_and_self_end_resolve_in_the_items_own_writing_mode() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://selfalign.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SELF ALIGNMENT: {got}");

    // ── VACUITY. The plain keywords must already be right, or these rows are measuring whether
    //    self-alignment works at all rather than WHOSE writing mode resolves it.
    assert!(
        got.contains("r8=0,0") && got.contains("r9=10,10"),
        "VACUOUS: plain `start`/`end` are not Chrome-exact, so the `self-*` rows below are not \
         measuring the distinction this gate is named for — got {got:?}"
    );

    for (claim, why) in [
        ("r3=10,0", "⭐ THE MECHANISM. An RTL child's own inline start is its RIGHT edge, so `justify-self: self-start` puts it at x=10 inside an LTR grid. The BLOCK axis is unaffected — y stays 0 — which is what says this is per-axis."),
        ("r4=0,10", "the `self-end` twin of r3: the same two axes, both the other way."),
        ("r6=0,10", "⭐⭐ A `vertical-lr` child's INLINE axis runs DOWN the screen, so `rtl` reverses the BLOCK-axis alignment and leaves the inline one alone. A fix keyed on `direction` alone gets this row right for the wrong reason."),
        ("r7=10,0", "⭐⭐ A `vertical-rl` child's BLOCK axis runs RIGHT-TO-LEFT with no `direction` involved at all. This is the row that a `direction`-only fix fails, and it is why the predicate reads the writing mode too."),
        ("rb=0,0", "⚠ CONTROL — the SAME RTL child asking for plain `start` must NOT move: `start` is the CONTAINER's start and the container is LTR. This row is what keeps the spelling load-bearing."),
        ("ra=10,0", "CONTROL — an RTL child in an RTL grid: two reversals that agree are no reversal. Stops a fix that keys on the ITEM alone rather than on the PAIR."),
        ("rc=5,5", "CONTROL — `center` has no start or end to reverse and must be untouched by the flip."),
        ("r1=0,0", "CONTROL — the child matches its container; `self-start` and `start` coincide."),
        ("r5=0,0", "CONTROL — a `vertical-lr` LTR child, whose axes happen to start on the same two sides as its horizontal container's."),
    ] {
        assert!(
            got.contains(claim),
            "G_SELF_ALIGNMENT_IS_THE_ITEMS_OWN: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// V1  never reverse (`self_alignment_is_reversed` returns false — the pre-tick behaviour)
//       -> r3, r4, r6 and r7 all read the container-relative answer; every control stays green.
// V2  key the reversal on `direction` alone, ignoring the writing mode
//       -> r6 reads 10,0 and r7 reads 0,0 — the two VERTICAL children, whose axes `direction` alone
//          cannot place. Every horizontal row stays green, which is what says the writing mode is
//          load-bearing rather than decorative.
// V3  apply the flip whether or not the keyword was the `self-` spelling
//       -> rb reads 10,0 — a plain `start` dragged to the item's own edge.
// V4  choose the acting axis from the ITEM's writing mode instead of the container's
//       -> r6 and r7 swap their axes.
