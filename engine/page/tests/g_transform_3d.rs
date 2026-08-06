//! # G_TRANSFORM_3D — `translate3d(x,y,0)` is a 2D translate, and we were dropping it on the floor
//!
//! `transform: translate3d(20px,10px,0)` left the element at its **untransformed position** — the
//! largest error the property can produce. It is not an exotic spelling: `translate3d` is *the*
//! idiom for putting an element on its own compositor layer, which is how every animation library,
//! carousel, drawer and sticky header on the modern web writes a plain translation.
//!
//! ```text
//!   a 100x40 box                          Chrome        before        after
//!     translate3d(20px,10px,0)          [ 20, 1070]   [  0, 1060]   [ 20, 1070]
//!     scale3d(2,2,1)                    [-50, 1110]   [  0, 1130]   [-50, 1110]
//!     rotate3d(0,0,1,45deg)             [0.5, 1170]   [  0, 1200]   [0.5, 1170]
//!     matrix3d(… 30,15,0,1)             [ 30, 1355]   [  0, 1340]   [ 30, 1355]
//!     translate3d(20px,10px,0) scale(2) [-30, 1470]   [-50, 1460]   [-30, 1470]
//!   ── already correct, and they are the CONTROLS ──
//!     rotateZ(90deg)                    [ 30, 1240]   [ 30, 1240]   unmoved
//!     translateZ(50px)  (no 2D effect)  [  0, 1410]   [  0, 1410]   unmoved
//! ```
//!
//! ## Where it lived, and why the comment above it hid it
//!
//! `stylo_map.rs` maps Stylo's computed transform list onto our affine ops, and its `_ => {}` arm
//! carried this note: *"3D/perspective skipped — our paint model is 2D"*. **That is true of a
//! genuine 3D effect and false of `translate3d(x, y, 0)`**, which has no 3D component at all. The
//! justification read as a decision, so nobody re-checked what it was actually discarding — and
//! `rotateZ` worked the whole time, which made the family look handled.
//!
//! With no `perspective` in force, `z` contributes nothing to the on-screen position, so the x/y
//! terms of each 3D function **are** its rendered effect: this is an exact projection, not an
//! approximation. `rotate3d` is taken **only about the z axis** for the opposite reason — a rotation
//! about x or y foreshortens, which a 2D pipeline cannot express, and inventing one would be a wrong
//! answer of the right type.
//!
//! ⚠ The same omission existed in the `MinimalCascade` parser (`parse_transform`'s `_ => {}`) and is
//! fixed there too, so the JS-less / headless fallback path agrees with the shipping one.
//!
//! ## How this goes RED
//!
//! - **Restore `_ => {}` in `stylo_map.rs`** (drop the four 3D arms) → every 3D row returns to its
//!   untransformed position: `#y01` reads x=0 against Chrome's 20. The original defect.
//! - **Map `Rotate3D` unconditionally** (ignore the axis check) → `#y08`, a rotation about the X
//!   axis, becomes a z-rotation: 99x99 where Chrome leaves the box 100x40.
//! - **Take `m11,m12,m13,m14…` from `Matrix3D`** instead of the 2D projection's
//!   `m11 m12 m21 m22 m41 m42` → `#y05` loses its translation and reads x=0.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font-family:sans-serif;font-size:16px}
.w{width:400px;height:60px;margin:10px 0}
.b{width:100px;height:40px}
</style></head><body>
<div class="w"><div class="b" id="y01" style="transform:translate3d(20px,10px,0)">bx</div></div>
<div class="w"><div class="b" id="y02" style="transform:scale3d(2,2,1)">bx</div></div>
<div class="w"><div class="b" id="y03" style="transform:rotate3d(0,0,1,45deg)">bx</div></div>
<div class="w"><div class="b" id="y04" style="transform:rotateZ(90deg)">bx</div></div>
<div class="w"><div class="b" id="y05" style="transform:matrix3d(1,0,0,0, 0,1,0,0, 0,0,1,0, 30,15,0,1)">bx</div></div>
<div class="w"><div class="b" id="y06" style="transform:translateZ(50px)">bx</div></div>
<div class="w"><div class="b" id="y07" style="transform:translate3d(20px,10px,0) scale(2)">bx</div></div>
<div class="w"><div class="b" id="y08" style="transform:rotate3d(1,0,0,45deg)">bx</div></div>
<div class="w"><div class="b" id="c01" style="transform:translate(20px,10px)">bx</div></div>
<div class="w"><div class="b" id="c02" style="transform:scale(2)">bx</div></div>
</body></html>"##;

fn rect_of(page: &manuk_page::Page, sel: &str) -> manuk_layout::Rect {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    *page
        .root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel}"))
}

#[test]
fn g_transform_3d() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://t3.test/", &fonts, 1200.0);
    // Every row sits in its own 400x60 wrapper, so the box's UNTRANSFORMED x is 0 and its y is the
    // wrapper's. Asserting the offset FROM the wrapper makes each row independent of the ones above
    // it — a dropped transform earlier in the file cannot cascade into a later row's expectation.
    let dx = |sel: &str| rect_of(&page, sel).x;
    let sz = |sel: &str| {
        let r = rect_of(&page, sel);
        (r.width, r.height)
    };

    // ── THE DEFECT: the whole 3D family was discarded, so the box never moved.
    assert!(
        (dx("#y01") - 20.0).abs() < 1.1,
        "G_TRANSFORM_3D: `translate3d(20px,10px,0)` puts the box at x={} where Chrome gives 20. \
         Reading 0 means the function was DROPPED and the element is at its untransformed position \
         — the largest error this property can produce, on the idiom every animation library uses \
         to get a compositor layer.",
        dx("#y01")
    );
    let (w2, h2) = sz("#y02");
    assert!(
        (w2 - 200.0).abs() < 1.1 && (h2 - 80.0).abs() < 1.1,
        "G_TRANSFORM_3D: `scale3d(2,2,1)` gives {w2} x {h2} where Chrome gives 200 x 80. Reading \
         100 x 40 means the function was dropped."
    );
    let (w3, h3) = sz("#y03");
    assert!(
        (w3 - 99.0).abs() < 1.6 && (h3 - 99.0).abs() < 1.6,
        "G_TRANSFORM_3D: `rotate3d(0,0,1,45deg)` gives {w3} x {h3} where Chrome gives 99 x 99 — a \
         z-axis rotate3d IS `rotate()`, and 100 x 40 means it was dropped."
    );
    assert!(
        (dx("#y05") - 30.0).abs() < 1.1,
        "G_TRANSFORM_3D: `matrix3d` with a 30px x-translation puts the box at x={} where Chrome \
         gives 30. The 2D projection of a 4x4 takes m11 m12 m21 m22 m41 m42 — indices 0 1 4 5 12 \
         13; reading 0 means the translation columns were missed or the function was dropped.",
        dx("#y05")
    );
    assert!(
        (dx("#y07") - -30.0).abs() < 1.1,
        "G_TRANSFORM_3D: `translate3d(20px,10px,0) scale(2)` puts the box at x={} where Chrome \
         gives -30. This row exists because a dropped function in a LIST is silent: the scale still \
         applies, so the box looks transformed while sitting 20px wrong.",
        dx("#y07")
    );

    // ── THE AXIS CHECK, which is the one place this must NOT act.
    let (w8, h8) = sz("#y08");
    assert!(
        (w8 - 100.0).abs() < 1.1 && (h8 - 40.0).abs() < 1.1,
        "G_TRANSFORM_3D: `rotate3d(1,0,0,45deg)` — a rotation about the X axis — gives {w8} x {h8} \
         and Chrome leaves the box 100 x 40 in this 2D projection. Reading 99 x 99 means the axis \
         check was dropped and an X rotation was treated as a Z rotation, which is a wrong answer \
         of the right type."
    );

    // ── THE CONTROLS: already correct before this change, and they must not move.
    assert!(
        (dx("#y04") - 30.0).abs() < 1.1,
        "G_TRANSFORM_3D: `rotateZ(90deg)` was ALWAYS mapped (that is why the family looked handled) \
         — it must still give x=30, not {}.",
        dx("#y04")
    );
    assert!(
        (dx("#y06") - 0.0).abs() < 1.1,
        "G_TRANSFORM_3D: `translateZ(50px)` has no 2D effect without a perspective context — x must \
         stay 0, not {}. It is matched explicitly so its omission is a decision, not the `_` arm \
         that hid the bug.",
        dx("#y06")
    );
    assert!(
        (dx("#c01") - 20.0).abs() < 1.1 && (sz("#c02").0 - 200.0).abs() < 1.1,
        "G_TRANSFORM_3D: the plain 2D `translate` and `scale` controls moved — this change is \
         additive to the 3D spellings and must leave the 2D ones byte-identical."
    );
}
