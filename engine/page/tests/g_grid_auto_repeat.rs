//! **G_GRID_AUTO_REPEAT — `repeat(auto-fill | auto-fit, …)` generates the tracks that FIT.**
//!
//! `grid-template-columns: repeat(auto-fill, minmax(18em, 1fr))` is *the* responsive-card idiom:
//! one declaration, no media queries, and the browser works out how many columns the container can
//! hold. Both of our cascades threw the whole thing away and produced **one** column:
//!
//! * **Stylo** hands us `RepeatCount::AutoFill`/`AutoFit`, and `template_to_tracks` folded every
//!   non-integer count through `_ => 1` — a repeat of one track became one track;
//! * **the text cascade's** `expand_grid_repeat` was a *string* rewrite that searched for the first
//!   `)` after `repeat(`. For `repeat(auto-fill, minmax(180px,1fr))` that `)` closes `minmax(`, so it
//!   tried to parse `"auto-fill"` as a count, failed, emitted nothing, and left a stray `)` behind.
//!
//! The repetition count is **not** something a cascade can compute: CSS Grid §7.2.3.1 defines it as
//! the largest N whose tracks plus gutters fit **the container's resolved inline size**, which is
//! layout's answer. taffy already models this exactly (`GridTemplateComponent::Repeat` +
//! `RepetitionCount::{AutoFill, AutoFit}`), so the fix carries the *shape* — an auto-repeat of N
//! track sizes — through our CSS model instead of collapsing it, and lets taffy count.
//!
//! **Every number below was measured against live Chromium** on
//! `tests/wpt/probes/grid-auto-repeat.html`, where we scored SHAPE 15.0% before and 100.0% after
//! (absolute placement 0.0% → 100.0%, dx=dy=dw=dh=0).
//!
//! Three things to prove, and the third is the guard rather than the feature:
//!
//! 1. `auto-fill` generates as many tracks as fit, and they divide the container;
//! 2. `auto-fit` **collapses** the repetitions that end up empty, so two items in a would-be
//!    three-track grid span the whole container instead of huddling in the first two tracks — that
//!    difference is the entire reason both keywords exist, and getting `auto-fit` wrong looks
//!    identical to getting `auto-fill` right;
//! 3. an integer `repeat(N, …)` still expands exactly as before — the count we already got right
//!    must not regress, and it shares the rewritten parser with the case that was broken.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>

<!-- 1. auto-fill in 600px with a 20px gap and a 180px minimum. N*180 + (N-1)*20 <= 600 holds at
        N=3 (580) and fails at N=4 (780), so Chromium makes THREE tracks of (600-40)/3 = 186.67,
        at x = 0 / 206.67 / 413.33. Six items therefore fill two rows of three. -->
<div id="fill" style="display:grid;grid-template-columns:repeat(auto-fill,minmax(180px,1fr));gap:20px;width:600px">
  <div id="a1">1</div><div id="a2">2</div><div id="a3">3</div>
  <div id="a4">4</div><div id="a5">5</div><div id="a6">6</div>
</div>

<!-- 2. auto-fit, SAME declaration, but only two items. The third repetition ends up empty and
        COLLAPSES (its gutter with it), so the two items share 600 - 20 = 580 => 290 each, at
        x = 0 / 310. Under auto-fill they would instead sit in two 186.67px tracks at 0 / 206.67. -->
<div id="fit" style="display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:20px;width:600px">
  <div id="b1">1</div><div id="b2">2</div>
</div>

<!-- 3. martinfowler.com's own declaration, at a container width chosen to land off a .5 rounding
        edge. 18em = 288px at the 16px root size; 2*288 + 20 = 596 fits 620, 3 does not => TWO
        tracks of (620-20)/2 = 300, at x = 0 / 320. This is the shape the site's card list wants
        and the one we rendered as a single 619px-wide column for the whole hunt. -->
<div id="mf" style="display:grid;grid-template-columns:repeat(auto-fill,minmax(18em,1fr));gap:20px;width:620px">
  <div id="c1">one</div><div id="c2">two</div><div id="c3">three</div><div id="c4">four</div>
</div>

<!-- 4. THE GUARD. An integer repeat() has a literal count, is expanded by the cascade as before,
        and must be unaffected by the rewrite that the auto- forms needed. -->
<div id="three" style="display:grid;grid-template-columns:repeat(3,1fr);gap:20px;width:600px">
  <div id="d1">1</div><div id="d2">2</div><div id="d3">3</div>
</div>

<script>
  var R = [];
  function box(id){ return document.getElementById(id).getBoundingClientRect(); }
  function px(v){ return Math.round(v); }
  function row(tag, ids){
    var p = ids.map(function(i){ return px(box(i).x); }).join(',');
    R.push(tag + 'x:' + p + ' w:' + px(box(ids[0]).width));
  }

  // 1. auto-fill: three tracks across, and the fourth item wraps to a second row.
  row('fill', ['a1','a2','a3']);
  R.push('fillwrap:' + (px(box('a4').y) > px(box('a1').y) ? 'row2' : 'row1'));

  // 2. auto-fit: the empty third repetition collapses, so two items span the container.
  row('fit', ['b1','b2']);

  // 3. the responsive-card idiom at martinfowler's own minimum.
  row('mf', ['c1','c2']);

  // 4. integer repeat() is untouched.
  row('three', ['d1','d2','d3']);

  document.getElementById('out').textContent = R.join(' | ');
</script></body></html>"##;

#[test]
fn a_auto_fill_and_auto_fit_generate_the_tracks_that_fit_while_integer_repeat_is_unchanged() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://grid.test/", &fonts, 1000.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "fillx:0,207,413 w:187",
            "`repeat(auto-fill, minmax(180px, 1fr))` in a 600px container with a 20px gap must \
             generate THREE tracks of 186.67px — the largest count whose tracks plus gutters fit \
             (CSS Grid §7.2.3.1). If this reads one x of 0 and a width near 600, the auto-repeat \
             was collapsed to a single track in the cascade and every responsive card grid on the \
             web renders as one full-width column",
        ),
        (
            "fillwrap:row2",
            "…and with three tracks the fourth item wraps onto a second row. A single-column grid \
             also puts item 4 below item 1, so this assertion is only meaningful beside the x \
             positions above — it is here to catch the opposite failure, a grid that generated \
             SIX tracks because the count ignored the container",
        ),
        (
            "fitx:0,310 w:290",
            "`auto-fit` COLLAPSES the repetitions that end up empty. Two items in a container that \
             would hold three tracks must therefore share the full 580px of non-gutter space — \
             290px each, starting at 0 and 310. If this reads `0,207 w:187` the tracks were \
             generated but not collapsed, which is `auto-fill` behaviour wearing `auto-fit`'s name \
             and leaves a third of every such row permanently blank",
        ),
        (
            "mfx:0,320 w:300",
            "martinfowler.com's own declaration, `repeat(auto-fill, minmax(18em, 1fr))`: 18em is \
             288px at the 16px root size, two of those plus a 20px gutter fit 620px and three do \
             not, so the card list is TWO 300px columns. We rendered it as one 619px column for \
             four ticks of hunting",
        ),
        (
            "threex:0,207,413 w:187",
            "THE GUARD. An integer `repeat(3, 1fr)` has a literal count, is still expanded by the \
             cascade, and must be untouched by the rewrite the auto- forms required. It shares the \
             new nesting-aware parser with them, so a regression here means the parser broke the \
             case that already worked while fixing the case that did not",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_GRID_AUTO_REPEAT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
