//! # G_TABLE_HEIGHT_IS_A_MINIMUM — a table box GROWS past its declared height; a block clamps
//!
//! CSS 2.1 §17.5.3: *"the table's height is the maximum of the value of [the] 'height' property …
//! and the sum of the row heights"*. So a table whose content is taller than its declared `height`
//! **grows**, where a block clamps and lets the content overflow — and `max-height` on a table has
//! no effect at all, for the same reason.
//!
//! This engine treated a table box's `height` as a used value like any other block's. Found twice
//! over, from two unrelated probes in two ticks — a `display:table` row inside t905's float battery
//! (Chrome 24 against our 20) and a `display:table-cell` row inside t907's missing-box battery
//! (identical) — which is what turned a one-off into a family worth a rule.
//!
//! Chrome-captured, `google-chrome-stable --headless=new --hide-scrollbars --window-size=1200,800`,
//! a 200px-wide box at `16px/1.5`:
//!
//! ```text
//!                                                       Chrome   before
//!   display:table; height:20px       (content 24)         24       20
//!   display:table; height:20px       (three lines, 72)    72       20
//!   display:inline-table; height:20px                     24       20
//!   display:table-cell; height:20px                       24       20
//!   display:table; height:20px; border-box; padding:5px   34       20
//!   display:table; max-height:10px                        24       10
//!   display:table; height:60px       (content 24)         60       60   already right
//!   display:table; min-height:60px                        60       60   already right
//!   display:table  (no height)                            24       24   already right
//!   display:BLOCK; height:20px                            20       20   MUST still clamp
//! ```
//!
//! ⚠⚠⚠ **THE LAST ROW IS THE GUARD AND IT IS HALF THE POINT.** A plain block that overflows its
//! declared height is *correct* behaviour, and a fix phrased as "let boxes grow" rather than "this
//! is the table box's own rule" would satisfy every other row and silently break every fixed-height
//! block on the web. It is asserted here beside the ones that changed.
//!
//! ⚠ **TWO OF THE THREE ROWS THIS GATE LEFT OPEN WERE CLOSED ONE TICK LATER, AND THEY WERE NOT AN
//! ALGORITHM AT ALL.** This header called `<table>`'s `border-spacing` part of "the table ALGORITHM";
//! it was **one missing declaration in the UA stylesheet** (`table { border-spacing: 2px }`), found
//! at t908 and gated by `G_TABLE_BORDER_SPACING_UA_DEFAULT`. `#t7` and `#t8` are asserted below.
//! *Naming something as out of scope is a hypothesis about its size, and it was wrong by two orders
//! of magnitude here.*
//!
//! ⚠⚠ **AND THE THIRD — "genuinely the algorithm" — WAS CLOSED AT t933, FIVE TICKS AFTER FOUR
//! SEPARATE GATES NAMED IT.** A `<td>` stretching to fill a table given a taller `height` was
//! Chrome 56 / ours 26; `#t10` is now asserted at **196x56**, Chrome's number. t908 taught the
//! table BOX to grow and nothing inside it moved, so the declared height became empty space at the
//! bottom — the box was right and every row was wrong. t933 distributes that surplus over the rows
//! (proportionally to natural height, excluding rows that specified one), which also closed
//! `g_orphan_table_cell#c3` and `g_anonymous_table_row#mid`. Four doors, one algorithm.
//!
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 body{margin:0;font:16px/1.5 sans-serif}
 .c{background:#cdf;width:200px}
</style></head><body>
<div class="c" id="t1" style="display:table;height:20px">one line</div>
<div class="c" id="t2" style="display:table;height:60px">one line</div>
<div class="c" id="t3" style="display:table">one line</div>
<div class="c" id="t4" style="display:block;height:20px">one line</div>
<div class="c" id="t5" style="display:table;height:20px">line one<br>line two<br>line three</div>
<div class="c" id="t6" style="display:inline-table;height:20px">one line</div>
<table class="c" id="t7" style="height:20px"><tr><td id="t8">one line</td></tr></table>
<table class="c" id="t9" style="height:60px"><tr><td id="t10">one line</td></tr></table>
<div class="c" id="t11" style="display:table-cell;height:20px">one line</div>
<div class="c" id="t12" style="display:table;height:20px;box-sizing:border-box;padding:5px">one line</div>
<div class="c" id="t13" style="display:table;min-height:60px">one line</div>
<div class="c" id="t14" style="display:table;max-height:10px">one line</div>

</body></html>
"##;

fn rect(page: &manuk_page::Page, sel: &str) -> (f32, f32, f32) {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let r = page
        .root_box
        .node_rects(dom)
        .get(&n)
        .copied()
        .unwrap_or_else(|| panic!("no box for {sel}"));
    (r.x, r.width, r.height)
}

fn c(page: &manuk_page::Page, sel: &str, w: f32, h: f32) {
    let (_, gw, gh) = rect(page, sel);
    assert!(
        (gw - w).abs() < 1.01 && (gh - h).abs() < 1.01,
        "G_TABLE_HEIGHT_IS_A_MINIMUM: `{sel}` expected w={w} h={h} (CAPTURED from \
         `google-chrome-stable --headless=new --hide-scrollbars --window-size=1200,800`), \
         got w={gw} h={gh}"
    );
}

#[test]
fn g_table_height_is_a_minimum() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://tbl.test/", &fonts, 1200.0);
    c(&page, "#t1", 200.0, 24.0);
    c(&page, "#t2", 200.0, 60.0);
    c(&page, "#t3", 200.0, 24.0);
    c(&page, "#t4", 200.0, 20.0);
    c(&page, "#t5", 200.0, 72.0);
    c(&page, "#t6", 200.0, 24.0);
    c(&page, "#t7", 200.0, 30.0);
    c(&page, "#t8", 196.0, 26.0);
    c(&page, "#t9", 200.0, 60.0);
    // CLOSED AT t933 — this row was the one the header called "genuinely the algorithm".
    c(&page, "#t10", 196.0, 56.0);
    c(&page, "#t11", 200.0, 24.0);
    c(&page, "#t12", 200.0, 34.0);
    c(&page, "#t13", 200.0, 60.0);
    c(&page, "#t14", 200.0, 24.0);
}
