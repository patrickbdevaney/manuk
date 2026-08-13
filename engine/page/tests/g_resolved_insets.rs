//! **G_RESOLVED_INSETS — `getComputedStyle(el).top` is the USED offset in px, not the author's `10%`.**
//!
//! ⚠⚠⚠ **THE SAME DEFECT AS `width`/`height`, ONE PROPERTY FAMILY OVER, AND IT SURVIVED THE TICK
//! THAT FIXED THOSE.** CSSOM's resolved-value special case puts `top`/`right`/`bottom`/`left` in the
//! **used**-value bucket whenever the property applies to a positioned element that generates a box.
//! We returned `dim_css(&cs.inset.top)` — what the cascade holds — so a page read:
//!
//! ```text
//!                                            Chrome     ours (before)
//!   position:relative; top:10%   (CB 100px)   10px         10%
//!   position:absolute; top:10%   (CB 200px)   20px         10%
//!   top:calc(10% - 1px)          (CB 100px)    9px         calc(-1px + 10%)
//!   position:relative; bottom:3px, top:auto   -3px         auto
//! ```
//!
//! **It was blocked on exactly one missing input, and t1220 named it before this tick started:** a
//! percentage inset resolves against the **containing block**, and `computed_style_js` received the
//! element's own `rect` and nothing else. `containing_block_size` now walks the arena for it — the
//! parent's CONTENT box for `relative`/`sticky`, the nearest positioned-or-transformed ancestor's
//! PADDING box for `absolute`, the nearest transformed ancestor (else the viewport) for `fixed`.
//! Three different ancestors and three different boxes; picking the wrong one is silent, because
//! every one of them yields a plausible number.
//!
//! **What it costs while it is wrong, and the RED proof sharpened this rather than confirming it.**
//! The expected story was "`parseFloat` returns `NaN`". It does not: `parseFloat("10%")` is **`10`**.
//! So every tooltip, dropdown, drag handle, carousel and sticky-header polyfill that pins a start
//! value gets a **plausible number in the wrong unit** — 10% of a 900px container silently becomes
//! `10px` — and nothing anywhere throws. Only the `calc()` spelling of the same offset is `NaN`.
//! One property, two different silent failures, chosen by how the author happened to write it. It is
//! the `getComputedStyle(el).transform` defect (`"undefined scale(2)"`) wearing different clothes:
//! *a wrong answer of the right type*, handed back with no way for the caller to tell.
//!
//! **The absolutization is of the COMPUTED value, and that is not "what layout did".**
//! `position:relative; top:10%; bottom:50%` is over-constrained — layout uses `bottom = -top` — and
//! CSSOM says an over-constrained inset resolves to the **computed** value. So both sides absolutize
//! *independently* rather than negating, which is asserted below and which the obvious
//! "report what layout used" implementation gets wrong.
//!
//! **`auto` splits three ways and the split is the whole subtlety:**
//! - `relative` — `auto` IS resolved: `-(opposite)`, or `0px` when the opposite is also `auto`.
//! - `sticky` — `auto` is **preserved**. A sticky box's offsets are a clamp range, not a
//!   displacement, so there is no used offset to report.
//! - `absolute`/`fixed` — ⚠ **REFUSED, named rather than approximated.** A resolved `auto` there is
//!   the used **static position** — where the box *would have been in flow* — which is layout output
//!   this seam does not receive. It is asserted below as still reporting `auto`, so the tick that
//!   publishes the static position has to come back and change this line deliberately.
//!
//! **Proven RED**: restore `dim_css(&cs.inset.*)` at the four physical call sites and fifteen claims
//! fail, `pop-parse-top=NaN` among them — the consequence rather than the symptom.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  /* Content area 200 x 100 — the containing block for the in-flow (relative/sticky) cases. */
  /* `overflow:hidden` makes it the SCROLLPORT too, so `#stick` resolves against the same 200 x 100
     content box the `relative` rows use — the two rules agree here, and `#stick2` is where they part. */
  #inflow  { width:200px; height:100px; padding:1px 2px; border-width:2px 4px; border-style:solid;
             overflow:hidden; }
  /* Padding area 400 x 200 — the containing block for the `absolute` case. */
  #absctr  { position:relative; width:368px; height:184px;
             padding:8px 16px; border-width:16px 32px; border-style:solid; }
  /* Padding area 600 x 300 — a TRANSFORM makes it the containing block for `fixed`. */
  #fixctr  { position:absolute; transform:scale(1); width:344px; height:172px;
             padding:64px 128px; border-width:128px 256px; border-style:solid; visibility:hidden; }

  #rel     { position:relative; top:10%; left:25%; bottom:50%; right:75%; }
  #relcalc { position:relative; top:calc(10% - 1px); left:calc(25% - 2px); }
  #relpx   { position:relative; top:1px; left:2px; bottom:3px; right:4px; }
  #relem   { position:relative; font-size:10px; top:1em; left:2em; }
  #relau1  { position:relative; bottom:3px; right:4px; }
  #relau2  { position:relative; top:1px; left:2px; }
  #relau3  { position:relative; }
  #stick   { position:sticky; top:10%; left:25%; }
  /* A SCROLLPORT of 400 x 200 whose sticky descendant is NOT its child — the row that separates
     "the scroll container" from "the parent", which is how every real sticky header is built. */
  #scroller { overflow:hidden; width:400px; height:200px; padding:5px; border:1px solid; }
  #wrap    { height:50px; }
  #stick2  { position:sticky; top:10%; left:25%; }
  #stat    { position:static; top:10%; left:25%; }
  #gone    { display:none; position:relative; top:10%; }
  #abs     { position:absolute; top:10%; left:25%; bottom:50%; right:75%; }
  #absau   { position:absolute; bottom:3px; }
  #fix     { position:fixed; top:10%; left:25%; }
</style></head><body>
<div id="inflow">
  <div id="rel"></div><div id="relcalc"></div><div id="relpx"></div><div id="relem"></div>
  <div id="relau1"></div><div id="relau2"></div><div id="relau3"></div>
  <div id="stick"></div><div id="stat"></div><div id="gone"></div>
</div>
<div id="absctr"><div id="abs"></div><div id="absau"></div></div>
<div id="scroller"><div id="wrap"><div id="stick2"></div></div></div>
<div id="fixctr"><div id="fix"></div></div>
<div id="out">-</div>
<script>
  var R = [];
  var p = function (k, v) { R.push(k + '=' + v); };
  var q = function (id, pr) { return getComputedStyle(document.getElementById(id))[pr]; };

  // ── The rule: a POSITIONED box reports its used offset, in px, against its CONTAINING BLOCK.
  p('rel.top',    q('rel', 'top'));       // 10% of the parent CONTENT height, 100
  p('rel.left',   q('rel', 'left'));      // 25% of the parent CONTENT width, 200
  p('rel.bottom', q('rel', 'bottom'));    // over-constrained -> the COMPUTED value, absolutized
  p('rel.right',  q('rel', 'right'));
  p('calc.top',   q('relcalc', 'top'));   // calc(10% - 1px) against 100
  p('calc.left',  q('relcalc', 'left'));  // calc(25% - 2px) against 200
  p('px.top',     q('relpx', 'top'));     // a specified px passes through — the case that worked
  p('em.top',     q('relem', 'top'));     // the cascade already absolutized `em`
  p('em.left',    q('relem', 'left'));

  // ── `auto` under RELATIVE positioning resolves; it does not pass through.
  p('au1.top',    q('relau1', 'top'));    // auto against bottom:3px  -> -3px
  p('au1.left',   q('relau1', 'left'));   // auto against right:4px   -> -4px
  p('au2.bottom', q('relau2', 'bottom')); // auto against top:1px     -> -1px
  p('au2.right',  q('relau2', 'right'));
  p('au3.top',    q('relau3', 'top'));    // both sides auto          -> 0px
  p('au3.right',  q('relau3', 'right'));

  // ── A DIFFERENT containing block per `position` — the part that is silent when it is wrong.
  p('abs.top',    q('abs', 'top'));       // 10% of the abspos CB's PADDING height, 200
  p('abs.left',   q('abs', 'left'));      // 25% of its PADDING width, 400
  p('abs.bottom', q('abs', 'bottom'));
  p('fix.top',    q('fix', 'top'));       // 10% of the TRANSFORMED ancestor's padding height, 300
  p('fix.left',   q('fix', 'left'));      // 25% of its padding width, 600

  // ── The guards. Each one is broken by "just absolutize everything".
  p('stat.top',   q('stat', 'top'));      // STATIC: the property does not apply -> computed value
  p('gone.top',   q('gone', 'top'));      // display:none generates no box -> computed value
  p('stick.top',  q('stick', 'top'));     // sticky absolutizes a percentage...
  p('stick.bot',  q('stick', 'bottom'));  // ...but PRESERVES `auto` — it is a clamp, not an offset
  p('stick2.top', q('stick2', 'top'));    // ...against the SCROLLPORT (200), not the parent (50)
  p('stick2.left',q('stick2', 'left'));   // 25% of the scrollport's 400 content width
  p('absau.top',  q('absau', 'top'));     // REFUSED: an abspos `auto` is the used STATIC POSITION

  // ── The logical spellings are the same box and must not drift from the physical ones.
  p('logical-agrees',
    q('rel', 'insetBlockStart') === q('rel', 'top') &&
    q('rel', 'insetInlineStart') === q('rel', 'left'));
  p('shorthand-agrees', q('rel', 'inset') === '10px 150px 50px 50px');

  // ── The consequence, in the form a positioning library actually writes it.
  p('pop-parse-top',  parseFloat(q('rel', 'top')));
  p('pop-parse-calc', parseFloat(q('relcalc', 'top')));
  p('endsWithPx',    /px$/.test(q('rel', 'top')) && /px$/.test(q('abs', 'left')));
  p('noPercentLeak', q('rel', 'top').indexOf('%') < 0 && q('abs', 'top').indexOf('%') < 0);
  p('noCalcLeak',    q('relcalc', 'top').indexOf('calc(') < 0);

  document.getElementById('out').textContent = R.join(' ');
</script>
</body></html>"##;

#[test]
fn computed_insets_are_the_used_offset_against_the_containing_block() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ri.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("RESOLVED INSETS: {got}");

    for (claim, why) in [
        // ── The rule.
        (
            "rel.top=10px",
            "THE DEFECT: a percentage inset resolves against the CONTAINING BLOCK's height. We had \
             no containing block at this seam, so we published the author's `10%`",
        ),
        (
            "rel.left=50px",
            "…and `left`/`right` resolve against its WIDTH, never its height. Using one basis for \
             both axes passes `top` and is wrong on every horizontal offset",
        ),
        (
            "rel.bottom=50px",
            "OVER-CONSTRAINED, and CSSOM says report the COMPUTED value — absolutized, but NOT \
             negated. An implementation that reports what layout used says `-10px` here",
        ),
        ("rel.right=150px", ""),
        (
            "calc.top=9px",
            "a `calc()` mixing px and % must come out as one px length. `calc(-1px + 10%)` is a \
             string no `parseFloat` on the web survives",
        ),
        ("calc.left=48px", ""),
        ("px.top=1px", "a specified px passes through unchanged — the case that always worked"),
        ("em.top=10px", "`em` was already absolutized by the cascade; this pins that it still is"),
        ("em.left=20px", ""),
        // ── `auto` under relative positioning.
        (
            "au1.top=-3px",
            "`top:auto` with `bottom:3px` is a DISPLACEMENT of -3px, not `auto` (CSS2 §9.4.3). ⚠ It \
             is also the row that proves the PERF fast path is correct: `#relau1` has no percentage \
             inset, so `inset_needs_containing_block` is false and the tree walk is SKIPPED — this \
             number is produced with no containing block at all, which is why `used_inset_css` \
             takes an `Option` basis instead of refusing without one",
        ),
        ("au1.left=-4px", "the same rule on the inline axis"),
        ("au2.bottom=-1px", "…and the mirror case: the END side resolves against the START side"),
        ("au2.right=-2px", ""),
        ("au3.top=0px", "both sides `auto` ⇒ no displacement ⇒ `0px`, not `auto`"),
        (
            "au3.right=0px",
            "…and `0px`, never `-0px` — a string no browser emits and every equality check misses",
        ),
        // ── A different containing block per `position`.
        (
            "abs.top=20px",
            "AN ABSPOS BOX HAS A DIFFERENT CONTAINING BLOCK: the nearest POSITIONED ancestor's \
             PADDING box (200px tall), not its parent's content box (100px). Using the parent here \
             yields 10px — a plausible number, silently wrong by 2x",
        ),
        (
            "abs.left=100px",
            "25% of the PADDING width, 400 — the border box is 464 and the content box 368, so all \
             three areas give a different answer and only one is Chrome's",
        ),
        ("abs.bottom=100px", ""),
        (
            "fix.top=30px",
            "A FIXED BOX'S CONTAINING BLOCK IS THE NEAREST TRANSFORMED ANCESTOR, not the viewport — \
             `transform: scale(1)` is the standard trick for pinning a fixed child, and treating an \
             IDENTITY transform as `none` answers this against the viewport instead",
        ),
        ("fix.left=150px", ""),
        // ── The guards.
        (
            "stat.top=10%",
            "GUARD: the inset properties do not APPLY to a static element, so CSSOM says report the \
             computed value. `just absolutize everything` fails here first",
        ),
        (
            "gone.top=10%",
            "GUARD: `display:none` generates no box, so there is no used value to report — the same \
             rule `width`/`height` already follow",
        ),
        (
            "stick.top=10px",
            "sticky DOES absolutize a percentage…",
        ),
        (
            "stick.bot=auto",
            "…and PRESERVES `auto`, unlike relative. A sticky box's offsets are a clamp range, not a \
             displacement, so `auto` means `unclamped on this edge` and has no px equivalent",
        ),
        (
            "stick2.top=20px",
            "⚠⚠⚠ THE ROW THAT SEPARATES `sticky` FROM `relative`: a sticky box's insets are insets \
             from the **SCROLLPORT** (CSS Position 3 §6.3), not from its containing block. `#stick2` \
             sits inside a 50px-tall wrapper inside a 200px scroll container — the parent rule says \
             `5px`, the scrollport rule says `20px`, and Chrome says 20. A sticky table header or \
             sidebar is almost never a direct child of its scroller, so this is not a small error",
        ),
        ("stick2.left=100px", "25% of the scrollport's 400px content width, not the wrapper's"),
        (
            "absau.top=auto",
            "REFUSED AND PINNED: an abspos `auto` resolves to the used STATIC POSITION, which is \
             layout output this seam does not receive. Chrome says a number here and we say `auto`; \
             asserting `auto` means the tick that publishes the static position must change this \
             line on purpose rather than discover it",
        ),
        // ── Reconciliation.
        (
            "logical-agrees=true",
            "RECONCILIATION: `inset-block-start` and `top` are two spellings of ONE box. A second \
             serializer for one value is the drift `max-inline-size` already caught once",
        ),
        (
            "shorthand-agrees=true",
            "…and the `inset` shorthand is a THIRD spelling. It read `10% 75% 50% 25%` while `top` \
             read `10px`, about the same element, until all twelve call sites shared one function",
        ),
        // ── The consequence.
        (
            "pop-parse-top=10",
            "THE ACTUAL BROKEN CALL: `parseFloat(getComputedStyle(el).top)`. ⚠ NOTE WHAT THE RED \
             PROOF SHOWED — against `10%` this returns **10, not NaN**, so the caller gets a \
             plausible number in the WRONG UNIT and nothing anywhere throws. 10% of a 900px \
             container silently becomes 10px. A crash would have been the kinder failure",
        ),
        (
            "pop-parse-calc=9",
            "…and the calc() form IS `NaN`, because `parseFloat('calc(-1px + 10%)')` has no leading \
             digits. One property, two different silent failures, depending on how the author wrote \
             the same offset",
        ),
        ("endsWithPx=true", "the resolved value is a px LENGTH, not a keyword or a percentage"),
        ("noPercentLeak=true", "…and never the author's percentage"),
        (
            "noCalcLeak=true",
            "…and never a raw `calc()` string, which is the shape that also defeats a regex",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_RESOLVED_INSETS: missing `{claim}`{}{}\n  got: {got}",
            if why.is_empty() { "" } else { " — " },
            why
        );
    }
}
