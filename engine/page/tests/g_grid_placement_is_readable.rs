//! **G_GRID_PLACEMENT_IS_READABLE — `grid-column-start`/`-end` were published and `grid-row-*` were
//! not, and none of the four shorthands existed at all.**
//!
//! A grid library that reads back an item's placement got a real answer on one axis and `undefined`
//! on the other — and `undefined.split("/")` is a TypeError, so the half-answer is the more dangerous
//! of the two: the script does not fall back, it dies.
//!
//! ⭐ **Fifth instance this session of *one of the pair was mapped and the other was not*** (t1435
//! which lengths count, t1436 which direction, t1438 which coordinate space, t1445 which extent). The
//! two column lines have been published since t901; the two row lines beside them never were.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`):
//!
//! ```text
//!                                                                    Chrome              before
//!   a  grid-row-start:2; grid-row-end:span 2; grid-column:1/3
//!        grid-row-start   "2"                                                            undefined
//!        grid-row         "2 / span 2"                                                   undefined
//!        grid-area        "2 / 1 / span 2 / 3"                                            undefined
//!   b  nothing declared
//!        grid-row         "auto"        (NOT "auto / auto")                              undefined
//!        grid-area        "auto"                                                         undefined
//!   c  grid-area:1/2/3/4  grid-area     "1 / 2 / 3 / 4"                                  undefined
//!   e  grid-row-end:3     grid-area     "auto / auto / 3"                                undefined
//!   g  grid-auto-flow: column dense     "column dense"                                   undefined
//!   g2 nothing declared                 "row"                                            undefined
//! ```
//!
//! ⭐⭐ **`e` IS THE ROW THAT SAYS *TRAILING*.** The shorthand drops `auto` components from the END
//! only — `auto / auto / 3` keeps both leading `auto`s because a real line follows them, while `b`'s
//! all-`auto` case collapses to a single `auto`. A serializer that drops every `auto` reads `3` there,
//! which is a different placement.
//!
//! ⚠ `grid-area` interleaves the axes — ROW-start / COLUMN-start / ROW-end / COLUMN-end — and it is
//! the one ordering in this family that is not "start then end". `a` is the row that catches getting
//! it wrong, because its four values are all distinct.
//!
//! ⚠ NAMED RESIDUE, measured and not fixed: `grid-template` still reads `undefined` (it serialises the
//! EXPLICIT template, `"50px 50px / 100px"`, where `grid-template-rows` reports the USED tracks), and
//! a custom-ident line name (`grid-row-start: foo`) has no representation in `GridLine` at all —
//! `Auto | Line(n) | Span(n)`. Both are a cascade-side gap rather than a serialisation one.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8></head><body>
<div id="g" style="display:grid;grid-template-rows:50px 50px;grid-template-columns:100px;grid-auto-flow:column dense">
  <div id="a" style="grid-row-start:2;grid-row-end:span 2;grid-column:1 / 3"></div>
  <div id="b"></div>
  <div id="c" style="grid-area:1 / 2 / 3 / 4"></div>
  <div id="d" style="grid-row:auto"></div>
</div>
<div id="g2" style="display:grid"><div id="e" style="grid-row-end:3"></div></div>
<div id="out">-</div>
<script>
function p(id,props){var s=getComputedStyle(document.getElementById(id));
 return id+'{'+props.map(function(k){return k+'='+s[k];}).join(' | ')+'}';}
document.getElementById('out').textContent=[
 p('a',['gridRowStart','gridRowEnd','gridRow','gridColumn','gridArea']),
 p('b',['gridRowStart','gridRowEnd','gridRow','gridArea']),
 p('c',['gridRowStart','gridColumnStart','gridArea']),
 p('d',['gridRow','gridRowStart']),
 p('e',['gridRow','gridArea']),
 p('g',['gridAutoFlow']),
 p('g2',['gridAutoFlow'])
].join('  ');
</script></body></html>"##;

#[test]
fn an_items_grid_placement_reads_back_on_both_axes() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://gridplace.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("GRID PLACEMENT: {got}");

    // ── VACUITY. The COLUMN axis must already read back, or these rows are measuring whether
    //    `getComputedStyle` serves grid properties at all rather than the missing HALF.
    assert!(
        got.contains("gridColumnStart=2"),
        "VACUOUS: the column axis does not read back either, so the rows below are not measuring an \
         axis asymmetry — got {got:?}"
    );

    for (claim, why) in [
        ("gridRowStart=2 | gridRowEnd=span 2", "⭐ THE MECHANISM — the row axis, published beside the column axis it has always sat next to."),
        ("gridRow=2 / span 2", "the shorthand: both components present because neither is `auto`."),
        ("gridArea=2 / 1 / span 2 / 3", "⚠ `grid-area` INTERLEAVES the axes — row-start / column-start / row-end / column-end. All four values here are distinct, so a wrong ordering cannot hide."),
        ("gridRow=auto | gridArea=auto", "an undeclared item collapses to a single `auto` rather than repeating it four times."),
        ("gridArea=1 / 2 / 3 / 4", "the `grid-area` shorthand round-trips through the cascade and back out unchanged."),
        ("gridRow=auto / 3 | gridArea=auto / auto / 3", "⭐⭐ THE ROW THAT SAYS *TRAILING*. Only `auto`s at the END are dropped; the leading ones stay because a real line follows. A serializer that drops every `auto` reads `3`, which is a different placement."),
        ("g{gridAutoFlow=column dense}", "`grid-auto-flow` carries its `dense` half in the same string."),
        ("g2{gridAutoFlow=row}", "CONTROL — the initial value is `row`, not the empty string, so an undeclared container still answers."),
    ] {
        assert!(
            got.contains(claim),
            "G_GRID_PLACEMENT_IS_READABLE: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// Y1  drop the row longhands and the four shorthands (the pre-tick state)
//       -> every row above reads `undefined`; `gridColumnStart` stays green, which is the asymmetry.
// Y2  join the components without dropping trailing `auto`s
//       -> b reads `auto / auto` and `auto / auto / auto / auto`.
// Y3  drop EVERY `auto` rather than only the trailing ones
//       -> e reads `3` for both shorthands — a different placement, silently.
// Y4  order `grid-area` as row-start / row-end / column-start / column-end
//       -> a reads `2 / span 2 / 1 / 3`; c is unmoved (1/2/3/4 is symmetric under the swap), which is
//          why the gate needs a row whose four values are all distinct.
