//! **G_ANIMATION_COMPOSITION — `animation-composition: add` ADDS to the underlying value; every
//! additive keyframe used to REPLACE it.**
//!
//! Each endpoint of an animation is built by re-running the element's own cascade with the keyframe's
//! declaration block appended — which is replacement by construction, and is exactly right for the
//! default `animation-composition: replace`. `add` says the keyframe's value is *added to* the value
//! the element would otherwise have, and nothing implemented that: an element with `bottom: 50px` and
//! `from { bottom: 100px; animation-composition: add }` reported `100px` where Chrome reports `150px`.
//!
//! ⭐ **The interpolation was already correct, and that is what makes this one missing term rather
//! than a broken engine.** WPT reads `100px → 150px → 200px` across the segment where it wants
//! `150px → 200px → 250px`: the right progression, every value short by precisely the underlying.
//!
//! **Borrowed, per the ladder — option 1, no fork.** `Procedure::Add` and `Procedure::Accumulate` are
//! Stylo's own, and `animation-composition` is a real Stylo longhand whose value the keyframe block
//! already carries.
//!
//! **To watch it go RED, one mutation per claim:**
//!
//! 1. force `Composite::Replace` in `bracket`'s reader → `add:` and `acc:` report the keyframe value
//!    with the underlying dropped;
//! 2. drop the `side.1.contains(id)` declared-set check in `interpolate` → `mixed:` DOUBLES, which is
//!    the one way this fix can be worse than the bug it replaces (see below);
//! 3. composite unconditionally rather than per side → `half:` moves, since only its `from` is
//!    additive.
//!
//! ⚠⚠⚠ **`mixed:` IS THE LOAD-BEARING ROW.** `Sample::properties` is the union over *all* keyframes,
//! so an endpoint that does not mention a property already carries the underlying value there. Adding
//! underlying to underlying silently doubles it — a wrong answer of the right type, on properties the
//! author never animated additively at all. The declared-longhand set is carried per side for exactly
//! this, and this row is what proves it is consulted.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 /* WPT's own idiom: -50s of a 100s animation is the exact half-way point at time zero. */
 /* ⚠ `linear` is not decoration. The default `animation-timing-function` is `ease`, whose output at
    input 0.5 is ~0.8023 — the first draft of this fixture omitted it and every row came out with the
    RELATIONSHIPS exactly right and the progress wrong, which reads at a glance like a broken engine.
    An expectation that hard-codes a number must pin every input that number depends on. */
 div { position: relative; height: 4px; animation-duration: 100s; animation-delay: -50s;
       animation-timing-function: linear; }

 /* underlying 50 · from add 100 · to add 200  =>  at 50%: 50 + 150 = 200 */
 #add { bottom: 50px; animation-name: addboth; }
 @keyframes addboth { from { bottom: 100px; animation-composition: add }
                      to   { bottom: 200px; animation-composition: add } }

 /* CONTROL — the same keyframes with the default `replace`: at 50% it is 150, no underlying. */
 #rep { bottom: 50px; animation-name: repboth; }
 @keyframes repboth { from { bottom: 100px } to { bottom: 200px } }

 /* ONE SIDE ONLY: from adds (50+100=150), to replaces (200). At 50% => 175. */
 #half { bottom: 50px; animation-name: halfadd; }
 @keyframes halfadd { from { bottom: 100px; animation-composition: add }
                      to   { bottom: 200px } }

 /* accumulate over a single pass composes like add for these types => 200. */
 #acc { bottom: 50px; animation-name: accboth; }
 @keyframes accboth { from { bottom: 100px; animation-composition: accumulate }
                      to   { bottom: 200px; animation-composition: accumulate } }

 /* THE DOUBLING GUARD. `top` is declared ONLY in the `to` keyframe, so the `from` side carries the
    element's underlying 30px by cascade fill-in. `from` is additive — and must NOT composite a
    property it never declared, or the from-side becomes 30+30=60 and the whole segment is wrong. */
 #mixed { bottom: 50px; top: 30px; animation-name: mixedkf; }
 @keyframes mixedkf { from { bottom: 100px; animation-composition: add }
                      to   { bottom: 200px; top: 70px; animation-composition: add } }
</style></head><body style="margin:0">
<div id="add"></div><div id="rep"></div><div id="half"></div><div id="acc"></div><div id="mixed"></div>
<div id="out">-</div>
<script>
  var R = [], cs = function (id, p) { return getComputedStyle(document.getElementById(id))[p]; };
  R.push('add:' + cs('add', 'bottom'));       // 200px
  R.push('rep:' + cs('rep', 'bottom'));       // 150px
  R.push('half:' + cs('half', 'bottom'));     // 175px
  R.push('acc:' + cs('acc', 'bottom'));       // 200px
  R.push('mixed:' + cs('mixed', 'top'));      // 50px  (from 30 -> to 30+70=100, at 50% => 65)
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

// ⚠ ONE `#[test]` fn per JS gate binary — two live `PageContext`s on two threads fault SpiderMonkey.
#[test]
fn animation_composition_add_adds_to_the_underlying_value() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://composition.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "rep:150px",
            "THE CONTROL, asserted first. The default `replace` interpolates 100 -> 200 and ignores \
             the underlying 50, so 150 at the half-way point. Every additive number below is only \
             meaningful against this one — and a fix that composited unconditionally breaks HERE",
        ),
        (
            "add:200px",
            "THE GATE. `add` on both sides: underlying 50 + interpolate(100, 200) = 50 + 150 = 200. \
             Reading 150 is the pre-tick behaviour — the right progression, short by the underlying",
        ),
        (
            "half:175px",
            "compositing is PER ENDPOINT, not per animation: from = 50+100 = 150, to = 200 \
             (replace), so 175 at the half-way point. A single animation-wide flag reads 200 here",
        ),
        (
            "acc:200px",
            "`accumulate` over a single pass composes like `add` for a length. Named separately \
             because it is a different `Procedure`, and a reader should not have to assume they are \
             wired together",
        ),
        (
            "mixed:65px",
            "THE DOUBLING GUARD. `top` is declared only in the `to` keyframe, so the additive `from` \
             side must leave the cascade's underlying 30px ALONE: from 30 -> to 30+70 = 100, giving \
             65 at the half-way point. Compositing the whole property set instead makes the from \
             side 60 and reads 80 — a wrong answer of the right type on a property the author never \
             animated additively",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_ANIMATION_COMPOSITION: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
