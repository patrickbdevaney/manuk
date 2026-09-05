//! **G_GRID_ZERO_MAX_TRACK_IS_ITS_MINIMUM — `minmax(<content-based>, 0px)` is `minmax(min, min)`,
//! and taffy only floors a growth limit by a FIXED base.**
//!
//! CSS Grid §12.5: *"if the growth limit is less than the base size, increase the growth limit to
//! match the base size."* A **zero** maximum therefore never caps anything — the track ends up at
//! whatever the items contribute. Taffy applies that flooring when the base comes from a fixed
//! minimum and not when it comes from the items, so every `minmax(auto, 0px)` track collapsed.
//!
//! ⚠ That is **847 failing subtests** across `css/css-grid`'s `minimum-size` family, ~20% of the
//! area's whole failing mass, and it is one sentence of the specification.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`), used `grid-template-columns`:
//!
//! ```text
//!                                                             Chrome     before      after
//!   m1  minmax(60px, 0px)                        CONTROL      60px       60px        60px  ✓
//!   m2  minmax(auto, 0px)   item width:60px                   60px        0px        60px
//!   m3  minmax(auto, 0px)   in a 40px grid                    60px        0px        60px
//!   m4  minmax(min-content, 0px)  an 8-char word              92.4375px   0px        92.4375px
//!   m5  minmax(auto, 0px) 30px    item width:60px             60px 30px   0px 30px   60px 30px
//!   m6  minmax(auto, 0px)   item min-width:60px               60px        0px        60px
//!   m7  minmax(0px, 0px)    item width:60px      CONTROL      0px         0px        0px   ✓
//!   m8  minmax(auto, 100px) item width:60px      CONTROL      100px/100   100px/100  100px/100 ✓
//!   m9  minmax(auto, 0px)   item overflow:hidden CONTROL      60px        0px        60px
//! ```
//!
//! ⭐⭐ **`m1` IS THE ROW THAT LOCALISES IT.** `minmax(60px, 0px)` — the same violation with a FIXED
//! minimum — was already Chrome-exact, so taffy's flooring rule exists and only the content-derived
//! base misses it. And `m3` shows the base itself is computed perfectly well: in a 40px grid, with no
//! free space to confuse the picture, the track reads 60. **It is the flooring that is missing, not
//! the measurement.**
//!
//! ⚠⚠ **ONLY A ZERO MAXIMUM, and that bound is the whole reason this is shippable.** The general
//! rule needs the base size, which is not known until taffy has run. Two wider remappings were
//! measured and REFUSED:
//!
//! ```text
//!   max -> auto()            minmax(auto,0px) in a 100px grid reads 100px — it absorbs the free space
//!   max -> fit_content(L)    minmax(auto,100px) with a 60px item reads 60, Chrome 100 — growth lost
//! ```
//!
//! A limit of **zero** can never exceed a base, so flooring it is unconditional and there is no
//! growth to lose. `m8` is the control that keeps the second refusal honest.
//!
//! ⚠ NAMED RESIDUE, measured: a NON-ZERO too-small maximum. `minmax(auto, 20px)` with a 60px item is
//! **60px** in Chrome and **20px** here — the same defect, in the case that needs the unknown. It is
//! the general form of this rule and it wants taffy's base size, not another remapping.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>body{margin:0}
.g{display:grid;grid-template-rows:10px}
</style></head><body>
<div class="g" id="m1" style="width:100px;grid-template-columns:minmax(60px,0px)"><div id="r1"></div></div>
<div class="g" id="m2" style="width:100px;grid-template-columns:minmax(auto,0px)"><div id="r2" style="width:60px"></div></div>
<div class="g" id="m3" style="width:40px;grid-template-columns:minmax(auto,0px)"><div id="r3" style="width:60px"></div></div>
<div class="g" id="m4" style="width:100px;grid-template-columns:minmax(min-content,0px)"><div id="r4">wwwwwwww</div></div>
<div class="g" id="m5" style="width:100px;grid-template-columns:minmax(auto,0px) 30px"><div id="r5" style="width:60px"></div><div></div></div>
<div class="g" id="m6" style="width:100px;grid-template-columns:minmax(auto,0px)"><div id="r6" style="min-width:60px"></div></div>
<div class="g" id="m7" style="width:100px;grid-template-columns:minmax(0px,0px)"><div id="r7" style="width:60px"></div></div>
<div class="g" id="m8" style="width:100px;grid-template-columns:minmax(auto,100px)"><div id="r8" style="width:60px"></div><div id="s8"></div></div>
<div class="g" id="m9" style="width:100px;grid-template-columns:minmax(auto,0px)"><div id="r9" style="width:60px;overflow:hidden"></div></div>
<div id="out">-</div>
<script>
function t(k){return getComputedStyle(document.getElementById(k)).gridTemplateColumns;}
function o(k){return document.getElementById(k).offsetWidth;}
document.getElementById('out').textContent=
 'm1='+t('m1')+' m2='+t('m2')+' m3='+t('m3')+' m4='+t('m4')+' m5='+t('m5')+' m6='+t('m6')
 +' m7='+t('m7')+' m8='+t('m8')+'/'+o('s8')+' m9='+t('m9');
</script></body></html>"##;

#[test]
fn a_zero_growth_limit_is_floored_by_the_tracks_own_base_size() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fo.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("GRID ZERO MAX TRACK: {got}");

    // ── VACUITY. The FIXED-minimum form of the same violation must already be right, or these rows
    //    are measuring whether taffy floors growth limits at all rather than which BASE it floors by.
    assert!(
        got.contains("m1=60px"),
        "VACUOUS: `minmax(60px, 0px)` is not Chrome-exact, so taffy's flooring rule is not present \
         and the rows below are not measuring the content-derived base — got {got:?}"
    );

    for (claim, why) in [
        ("m2=60px", "⭐ THE MECHANISM. A zero growth limit is floored by the base size, so the track is whatever the items contribute — 60, not 0."),
        ("m3=60px", "⭐ THE SAME IN A GRID TOO NARROW TO HOLD IT. A 40px container has no free space to distribute, so this row reads the base size directly: taffy computes it correctly and only the flooring was missing."),
        ("m4=92.4375px", "a CONTENT-measured base rather than a declared one — `min-content` of an 8-character word. The rule is about the base's PROVENANCE, so a text measurement has to work too."),
        ("m5=60px 30px", "TWO tracks: the neighbouring fixed track must be unmoved, which is what says this is a per-track floor and not a container-level effect."),
        ("m6=60px", "the base from `min-width` rather than `width` — the item's used minimum size, the other half of Grid §12.5.1's minimum contribution."),
        ("m7=0px", "CONTROL — `minmax(0px, 0px)`. The base really is zero here, so the track really is zero. Without this row the fix could be 'never let a track be 0'."),
        ("m8=100px/100", "⚠ CONTROL, and it is the row that refuses the general remapping. A NON-zero maximum must still permit growth: `minmax(auto,100px)` is 100px in Chrome, and rewriting every fixed maximum as `fit-content` reads 60."),
        ("m9=60px", "`overflow: hidden` zeroes the item's AUTOMATIC minimum size, but the item's declared `width` is still its minimum CONTRIBUTION — Grid §12.5.1 and Box Sizing §5.1 are different rules and this row keeps them apart."),
    ] {
        assert!(
            got.contains(claim),
            "G_GRID_ZERO_MAX_TRACK_IS_ITS_MINIMUM: expected `{claim}` (Chrome-measured) — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// R1  drop the remap (the pre-tick state)
//       -> m2, m3, m4, m5, m6 and m9 collapse to 0; m1, m7 and m8 stay green, which is what makes
//          the defect the CONTENT-derived base rather than the flooring rule.
// R2  map the maximum to `auto()` instead
//       -> m2 reads 100px — an `auto` maximum absorbs the container's free space.
// R3  apply `fit_content` for ANY fixed maximum, not only zero
//       -> m8 reads 60px/60 — the growth a non-zero limit exists to permit is lost.
