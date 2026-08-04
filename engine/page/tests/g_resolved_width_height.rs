//! **G_RESOLVED_WIDTH_HEIGHT — `getComputedStyle(el).width` is the USED value in px, not the
//! specified one.**
//!
//! ⚠⚠⚠ **WE ANSWERED WITH THE AUTHOR'S STRING WHILE THE REAL NUMBER SAT ONE FIELD AWAY.** CSSOM makes
//! `width`/`height` two of the handful of properties whose *resolved value* is the **used value**
//! whenever the element generates a box. We returned the computed value verbatim, so a page read
//! `auto`, `50%`, or a raw `calc(-0.016662598px + 33.333336%)` string where Chrome reads `580px`,
//! `300px`, `199.984px` — measured, six elements, before a line was changed:
//!
//! ```text
//!                                     Chrome     ours (before)
//!   block, width:auto, pad 5, bd 2     580px        auto
//!   block, width:50% of 600            300px        50%
//!   abspos with left+right             740px        auto
//!   flex item, flex:1, in a 400px      400px        auto
//!   ANY height                          20px        auto        <- uniformly
//! ```
//!
//! **It was never a layout gap.** `offsetWidth` on the same elements was already exact (594 and 300
//! against Chrome's 594 and 300), and `computed_style_js` has taken the element's layout `rect` since
//! the transform work. The binding was declining to publish what layout had already computed — the
//! same shape as `getComputedStyle(el).transform`, which was applied for sixty ticks before it
//! reached JavaScript.
//!
//! **What it costs.** `parseInt($(el).css('width'))` is `NaN` on every jQuery page. jQuery's
//! `getWidthOrHeight` survives only because it falls back to `offsetWidth` *when it sees `auto`* — and
//! that fallback is itself gated on `elem.getClientRects().length`, so an engine that answered `auto`
//! and had no `getClientRects` would return `0` and every measure-then-size widget would size to
//! nothing. Every animation library that pins a start value (`el.style.width =
//! getComputedStyle(el).width` before a transition) reads this directly, with no fallback at all.
//!
//! **The box reported is the one the element's own `box-sizing` names** — Chrome-measured, because the
//! plausible answer (always the content box) is wrong for `border-box`.
//!
//! ⚠ **Two guards, both Chrome-measured, both of which "always report the rect" would break**: a
//! `display:none` element reports its computed value (`auto`), and a non-replaced **inline** reports
//! `auto` too — Chrome says `auto` for a `<span>` whose `offsetWidth` is a real number.
//!
//! **Proven RED**: return `dim_css(&cs.width)` / `dim_css(&cs.height)` again and nine claims fail,
//! including every `height` row and `jq-parse-width=NaN`, which is the consequence rather than the
//! symptom.
//!
//! ⚠ **One case is deliberately NOT resolved and is named here rather than asserted**: `width:auto`
//! together with a PERCENTAGE padding. That padding resolves against the containing block's width,
//! which this seam does not hold, so the element keeps its specified value instead of getting a
//! confidently wrong number. Asserting the wrong value would make this gate fail on the tick that
//! fixes it — the "honest no-stub becomes a lie when the cap lands" trap.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  #host { width: 600px; font: 16px/20px monospace; position: relative; }
  #a  { display:block; padding:5px; border:2px solid; margin:3px; }
  #b  { display:block; width:50%; height:40px; }
  #c  { border:3px solid; }
  #d  { display:block; position:absolute; left:10px; top:20px; right:30px; height:11px; }
  #e  { display:none; width:70px; height:70px; }
  #bb { display:block; box-sizing:border-box; width:200px; padding:10px; border:5px solid; height:60px; }
  #cb { display:block; box-sizing:content-box; width:200px; padding:10px; border:5px solid; height:60px; }
  #fx { display:flex; width:400px; } #fi { flex:1; height:12px; }
  #pc { display:block; width:33.333%; }
  #pp { display:block; width:100px; padding:10%; }
</style></head><body>
<div id="host">
  <div id="a">A</div><div id="b"></div><span id="c">C</span><div id="d"></div><div id="e"></div>
  <div id="bb"></div><div id="cb"></div>
  <div id="fx"><div id="fi">f</div></div>
  <div id="pc"></div><div id="pp"></div>
</div>
<div id="out">-</div>
<script>
  var R = [];
  var p = function (k, v) { R.push(k + '=' + v); };
  var q = function (id, pr) { return getComputedStyle(document.getElementById(id))[pr]; };

  // ── The rule: a rendered box reports its USED size in px.
  p('a.w',  q('a', 'width'));      // width:auto in a 600px block, content box = 600 - 6(margin) - 4(bd) - 10(pad)
  p('a.h',  q('a', 'height'));     // one 20px line
  p('b.w',  q('b', 'width'));      // 50% of 600
  p('b.h',  q('b', 'height'));
  p('d.w',  q('d', 'width'));      // abspos sized by left+right
  p('bb.w', q('bb', 'width'));     // box-sizing:border-box -> the BORDER box
  p('cb.w', q('cb', 'width'));     // box-sizing:content-box -> the CONTENT box
  p('fi.w', q('fi', 'width'));     // a flex item's used width
  p('fi.h', q('fi', 'height'));

  // ── The guards.
  p('c.w',  q('c', 'width'));      // non-replaced inline -> auto, even though it HAS a border box
  p('c.h',  q('c', 'height'));
  p('e.w',  q('e', 'width'));      // display:none -> the COMPUTED value, not the used one
  p('e.h',  q('e', 'height'));

  // ── The consequence, in the form libraries actually write it.
  p('jq-parse-width',  parseInt(q('a', 'width'), 10));
  p('jq-parse-height', parseInt(q('b', 'height'), 10));
  p('endsWithPx',      /px$/.test(q('b', 'width')) && /px$/.test(q('a', 'height')));
  p('noPercentLeak',   q('b', 'width').indexOf('%') < 0);
  p('noCalcLeak',      q('pc', 'width').indexOf('calc(') < 0);
  p('frac-isPx',       /^[0-9.]+px$/.test(q('pc', 'width')));
  // A percentage PADDING on a content-box element: the specified width IS the content box, so the
  // fallback path lands on Chrome's answer. The case that genuinely needs the containing block is
  // `width:auto` WITH a percentage padding, and that one is named in the module doc, not asserted —
  // pinning a known-wrong value here would make the gate fail on the tick that fixes it.
  p('pct-padding-keeps-specified', q('pp', 'width'));

  // ── Cross-check against the geometry that was ALREADY right, so the two can never drift.
  var a = document.getElementById('a');
  p('agrees-with-offset',
    (parseFloat(q('a', 'width')) + 2 * 2 + 2 * 5) === a.offsetWidth);
  var bb = document.getElementById('bb');
  p('borderbox-agrees', parseFloat(q('bb', 'width')) === bb.offsetWidth);

  document.getElementById('out').textContent = R.join(' ');
</script>
</body></html>"##;

#[test]
fn computed_width_and_height_are_the_used_value_in_pixels() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://rw.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("RESOLVED W/H: {got}");

    for (claim, why) in [
        // ── The rule.
        (
            "a.w=580px",
            "THE DEFECT: `width:auto` in a 600px block resolves to the used CONTENT box \
             (600 - 6 margin - 4 border - 10 padding). We said `auto`",
        ),
        (
            "a.h=20px",
            "and HEIGHT was uniformly `auto` — every single element, whatever its box",
        ),
        ("b.w=300px", "a PERCENTAGE must resolve; we returned the literal `50%`"),
        ("b.h=40px", "a specified px passes through unchanged — the case that always worked"),
        (
            "d.w=560px",
            "an absolutely-positioned box sized by `left` + `right` has no specified width at all, so \
             only the used value can answer. `#host` is `position:relative` so the containing block \
             is 600px and the number does not depend on the harness viewport",
        ),
        (
            "bb.w=200px",
            "`box-sizing: border-box` reports the BORDER box — the plausible wrong answer here is \
             the content box (170px), and Chrome says 200",
        ),
        (
            "cb.w=200px",
            "…while `content-box` reports the CONTENT box, whose offsetWidth is 230. Same declared \
             width, two different boxes, and only box-sizing tells them apart",
        ),
        ("fi.w=400px", "a flex item's width is decided by the flex algorithm, never by its own style"),
        ("fi.h=12px", ""),
        // ── The guards.
        (
            "c.w=auto",
            "GUARD: a non-replaced INLINE reports `auto` in Chrome even though it has a border box. \
             `always report the rect` passes every claim above and breaks the commonest element",
        ),
        ("c.h=auto", "the same guard on the other axis"),
        (
            "e.w=70px",
            "GUARD: `display:none` generates no box, so CSSOM says report the COMPUTED value — here \
             the author's own `70px`, NOT a used value of 0",
        ),
        ("e.h=70px", "the same, on height"),
        // ── The consequence.
        (
            "jq-parse-width=580",
            "THE ACTUAL BROKEN CALL: `parseInt($(el).css('width'))` — `NaN` against `auto`, which is \
             how a measure-then-size widget sizes to nothing",
        ),
        ("jq-parse-height=40", ""),
        ("endsWithPx=true", "the resolved value is a px LENGTH, not a keyword"),
        ("noPercentLeak=true", "…and never the author's percentage"),
        (
            "noCalcLeak=true",
            "…and never a raw `calc()` string. A 33.333% width came out of our cascade as \
             `calc(-0.016662598px + 33.333336%)`, which no `parseFloat` on the web survives",
        ),
        (
            "frac-isPx=true",
            "a fractional percentage resolves to a bare px length. The exact sub-pixel value is an \
             engine-vs-Chrome rounding question and is deliberately NOT asserted here",
        ),
        (
            "pct-padding-keeps-specified=100px",
            "a percentage PADDING must not corrupt the answer: on a content-box element the \
             specified width IS the content box, so the refuse-and-fall-back path lands exactly on \
             Chrome. An implementation that subtracted a percentage as if it were px fails here",
        ),
        // ── Reconciliation with the number that was already right.
        (
            "agrees-with-offset=true",
            "RECONCILIATION: content width + border + padding must equal `offsetWidth`, which was \
             correct all along. Two readings of one box that disagree mean one of them is invented",
        ),
        (
            "borderbox-agrees=true",
            "and for `border-box` the resolved width IS `offsetWidth`",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_RESOLVED_WIDTH_HEIGHT: missing `{claim}`{}\n  got: {got}",
            if why.is_empty() { String::new() } else { format!("\n  — {why}") }
        );
    }
}
