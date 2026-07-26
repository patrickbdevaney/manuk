//! **G_ISOLATION_PRICED — what `isolation` costs us, measured, and why it is NOT built here.**
//!
//! `isolation` is the last `UNRENDERED_LONGHANDS` row with real usage (18.0% of page loads, Blink
//! use counters). §VI.3 says price a candidate by measuring it **in this engine** before building
//! it — t590 did that for `appearance` and found a no-op, saving a misdirected tick. This is the
//! same measurement, and it came out the other way: **`isolation` is genuinely missing, and t594
//! made it matter.**
//!
//! ## The measurement
//!
//! A cyan page, a card with **no background of its own**, and a red overlay with
//! `mix-blend-mode: multiply`. What the overlay blends with is whatever is *behind the card*:
//!
//! ```text
//! plain card       red × cyan → (0,0,0)     correct: nothing isolates the blend
//! isolated card    red × cyan → (0,0,0)     WRONG: should be (255,0,0)
//! ```
//!
//! `isolation: isolate` makes the card a stacking context, so the overlay's backdrop is the card's
//! own — empty — surface, not the cyan page behind it. Multiplying against nothing leaves the
//! overlay red. We paint black.
//!
//! **This became a real defect at t594, not before.** Until `mix-blend-mode` landed there was
//! nothing for `isolation` to contain, which is exactly why the row could sit at `missing` without
//! consequence. Landing one capability turned a dormant row into a live one — worth noticing as a
//! general effect: **a capability's price is not fixed; its neighbours change it.**
//!
//! ## Why the fix is NOT in this tick
//!
//! The paint model is **flat**: one group per box, z-sorted, each composited onto the page. Blending
//! works because a group's backdrop is simply what is already on the canvas. Isolation needs the
//! isolating subtree composited into its **own** surface first — a *nested* group — and that is a
//! restructure of how groups are built and ordered, not another field on one.
//!
//! `IndexedDB MVP: keep ABSENT until done — half-built is worse` is the standing rule, and it
//! applies: a partial isolation that contained *some* blends would be harder to diagnose than one
//! that contains none. So this gate **pins the current, wrong behaviour** so the row cannot rot into
//! a vague "missing", and states what correct looks like. When nesting lands, this gate goes red and
//! that is the signal to flip it.

use manuk_text::FontContext;

const W: u32 = 300;
const H: u32 = 120;

const HTML: &str = r##"<!doctype html><html><head><style>
  body { margin: 0; background: #fff }
  .page { position: absolute; top: 0; width: 150px; height: 120px; background: #0ff }
  #a { left: 0 } #b { left: 150px }
  /* No background of its own: the overlay blends with whatever is BEHIND the card. */
  .card { position: absolute; top: 20px; left: 20px; width: 80px; height: 80px }
  #biso { isolation: isolate }
  .ov { position: absolute; top: 0; left: 0; width: 60px; height: 60px;
        background: #f00; mix-blend-mode: multiply }
</style></head><body>
<div class="page" id="a"><div class="card"><div class="ov"></div></div></div>
<div class="page" id="b"><div class="card" id="biso"><div class="ov"></div></div></div>
</body></html>"##;

fn at(b: &[u8], x: u32, y: u32) -> (u8, u8, u8) {
    let i = ((y * W + x) * 4) as usize;
    (b[i], b[i + 1], b[i + 2])
}

#[test]
fn isolation_is_measured_missing_and_the_cost_is_pinned() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://iso.test/", &fonts, W as f32);
    let canvas = page.paint(&fonts, W, H);
    let b = canvas.rgba_bytes();

    let pagebg = at(b, 130, 100);
    let plain = at(b, 50, 50);
    let isolated = at(b, 200, 50);
    println!("ISOLATION: pagebg={pagebg:?} plain={plain:?} isolated={isolated:?}");

    // 1. VACUITY GUARD — the cyan page must be there, or "the overlay blended with cyan" is a
    //    statement about nothing.
    assert_eq!(
        pagebg,
        (0, 255, 255),
        "the cyan page background must paint — it is the backdrop the whole measurement is about"
    );

    // 2. THE BLEND ITSELF WORKS (t594), which is what makes the row live.
    assert_eq!(
        plain,
        (0, 0, 0),
        "an un-isolated `mix-blend-mode: multiply` overlay must multiply with the CYAN page behind \
         the card: red × cyan = black. If this is (255,0,0) the blend regressed and the isolation \
         question is moot"
    );

    // 3. …AND ISOLATION DOES NOT CONTAIN IT. This is the pinned defect.
    assert_eq!(
        isolated,
        (0, 0, 0),
        "PINNED DEFECT: `isolation: isolate` should make the card a stacking context, so the \
         overlay's backdrop is the card's own EMPTY surface and multiply leaves it (255,0,0). We \
         paint (0,0,0) — the blend leaks past the isolating ancestor to the page behind it.\n\n\
         When this assertion FAILS with (255,0,0), isolation has landed: flip the constellation row \
         to `gated`, drop `isolation` from UNRENDERED_LONGHANDS, and replace this gate with one that \
         asserts the correct value. That is the intended way for this gate to die."
    );
}
