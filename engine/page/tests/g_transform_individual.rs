//! # G_TRANSFORM_INDIVIDUAL — `translate` / `rotate` / `scale` are PROPERTIES, and we had none of them
//!
//! CSS Transforms 2 splits the three commonest transform functions into properties of their own, so
//! that setting one does not clobber the others: `el.style.translate = '30px 15px'` leaves a
//! `rotate` the stylesheet set alone, which is why every animation library now writes them. This
//! engine matched only `"transform"` — in **both** cascades — so all three were **absent** rather
//! than wrong and the element sat at its untransformed position and its untransformed size.
//!
//! Priced on the burndown's own corpus, 171 sites fetched *with their linked stylesheets*:
//! **33/171 = 19.3%** declare at least one (`rotate:` 12.9%, `scale:` 8.8%, `translate:` 3.5%).
//!
//! Chrome-measured (headless, 1200px), a 40×20 abspos box at `left:20px; top:10px` with
//! `transform-origin: 0 0`, reported as an offset from its own 200×60 container:
//!
//! ```text
//!                                              Chrome            before
//!   #s1  translate: 30px                     (50,10) 40x20     (20,10) 40x20
//!   #s2  scale: 2                            (20,10) 80x40     (20,10) 40x20
//!   #s6  rotate: z 90deg                     ( 0,10) 20x40     (20,10) 40x20
//!   #s5  translate: 50% 100%                 (40,30) 40x20     (20,10) 40x20
//!   #o7  translate+rotate+scale together     ( 0,20) 40x80     (20,10) 40x20
//! ```
//!
//! ## The two shorthand rules are OPPOSITE, and one fixture cannot tell them apart
//!
//! A one-value `translate: 30px` leaves **y at 0**; a one-value `scale: 2` is **uniform**. Writing
//! either rule for both — the obvious simplification — passes half the rows. `#s1` and `#s2` are
//! there to make that impossible.
//!
//! ## The ORDER is fixed by the spec, not by the declarations
//!
//! §3: the matrix is `translate`, then `rotate`, then `scale`, then the `transform` list —
//! **whatever order the declarations appeared in**. So every pair is written BOTH ways and both
//! must agree: `#o1`/`#o2`, `#o3`/`#o4`, `#o5`/`#o6`. That is what makes these separate fields
//! composed at use, rather than one Vec appended to at parse time.
//!
//! ## How this goes RED
//!
//! - **Delete the `"translate" | "rotate" | "scale"` arms** in `engine/css/src/lib.rs`, or the
//!   `clone_translate/clone_rotate/clone_scale` block in `stylo_map.rs` (this gate runs the
//!   SHIPPING cascade, so the second is the one it catches): every `#s*` and `#o*` row collapses to
//!   `(20,10) 40x20`, its untransformed box. RUN, not reasoned.
//! - **Return `Cow::Borrowed(&self.transform)` unconditionally** from `effective_transform`: same
//!   collapse, with the parse still working — which is the version that would survive a unit test
//!   of the parser alone.
//! - **Emit `rotate` before `translate`** in `effective_transform` — the plausible
//!   "declaration order" reading: `#o1` reads `(0,40) 20x40` against Chrome's `(30,10) 20x40`.
//!   ⚠ I first wrote that this would take `#o2` down *"while `#o1` still passes"*, which is wrong
//!   and is left here because the class recurs: **both** rows carry both properties, so a wrong
//!   composition order moves the pair together and `#o1` — the earlier row — is what fails. A RED
//!   recipe written from the code rather than from a run is a hypothesis wearing a receipt's
//!   clothes.
//!
//! ⚠ **`rotate: x 45deg` and `rotate: y 45deg` are NOT in this gate, and the reason is a measured
//! correction to a claim this repo already carries.** `g_transform_3d.rs` says a rotation about x
//! or y *"foreshortens, which a 2D pipeline cannot express"*. Measured: with no `perspective` in
//! force it is exactly a scale on the other axis — Chrome reports `rotate: x 45deg` on a 40×20 box
//! as **40 × 14.14** (= 20·cos45°) and `rotate: y 45deg` as **28.28 × 20**. We report 40×20 for
//! both. That is a real defect and a DIFFERENT rule from this one (it belongs to `rotate3d` and
//! `rotateX/Y` too), so it is banked rather than smuggled in here.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font-family:sans-serif;font-size:16px}
.c{width:200px;height:60px;position:relative;margin-bottom:30px}
.b{position:absolute;left:20px;top:10px;width:40px;height:20px;transform-origin:0 0}
</style></head><body>
<div class="c" id="p1"><div class="b" id="z1"></div></div>
<div class="c" id="p2"><div class="b" id="z2" style="rotate:none;scale:none;translate:none"></div></div>
<div class="c" id="p5"><div class="b" id="s1" style="translate:30px"></div></div>
<div class="c" id="p6"><div class="b" id="s2" style="scale:2"></div></div>
<div class="c" id="p7"><div class="b" id="s3" style="scale:2 3"></div></div>
<div class="c" id="p8"><div class="b" id="s4" style="scale:50%"></div></div>
<div class="c" id="p9"><div class="b" id="s5" style="translate:50% 100%"></div></div>
<div class="c" id="p10"><div class="b" id="s6" style="rotate:z 90deg"></div></div>
<div class="c" id="p11"><div class="b" id="s7" style="rotate:0 0 1 90deg"></div></div>
<div class="c" id="p12"><div class="b" id="o1" style="translate:30px 0;rotate:90deg"></div></div>
<div class="c" id="p13"><div class="b" id="o2" style="rotate:90deg;translate:30px 0"></div></div>
<div class="c" id="p14"><div class="b" id="o3" style="translate:30px 0;scale:2"></div></div>
<div class="c" id="p15"><div class="b" id="o4" style="scale:2;translate:30px 0"></div></div>
<div class="c" id="p16"><div class="b" id="o5" style="translate:30px 0;transform:scale(2)"></div></div>
<div class="c" id="p17"><div class="b" id="o6" style="transform:scale(2);translate:30px 0"></div></div>
<div class="c" id="p18"><div class="b" id="o7" style="translate:20px 10px;rotate:90deg;scale:2"></div></div>
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
fn g_transform_individual() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ti.test/", &fonts, 1200.0);
    // Every row is reported as an offset from its OWN container, so a row that moves cannot
    // cascade into the expectation of the row below it.
    let row = |b: &str, c: &str| {
        let (r, p) = (rect_of(&page, b), rect_of(&page, c));
        (r.x - p.x, r.y - p.y, r.width, r.height)
    };
    let close = |got: (f32, f32, f32, f32), want: (f32, f32, f32, f32), id: &str, why: &str| {
        let d = [
            got.0 - want.0,
            got.1 - want.1,
            got.2 - want.2,
            got.3 - want.3,
        ];
        assert!(
            d.iter().all(|v| v.abs() < 1.1),
            "G_TRANSFORM_INDIVIDUAL {id}: got {got:?}, Chrome gives {want:?}. {why}"
        );
    };
    let untransformed = (20.0, 10.0, 40.0, 20.0);

    // ── THE CONTROLS FIRST. No individual property, and the explicit `none` that must CLEAR
    //    rather than be ignored.
    close(
        row("#z1", "#p1"),
        untransformed,
        "#z1",
        "no transform at all",
    );
    close(
        row("#z2", "#p2"),
        untransformed,
        "#z2",
        "`translate/rotate/scale: none` is the initial value and must leave the box alone",
    );

    // ── THE TWO SHORTHAND RULES, and they are opposite.
    close(
        row("#s1", "#p5"),
        (50.0, 10.0, 40.0, 20.0),
        "#s1",
        "a one-value `translate: 30px` leaves y at 0 — it is NOT uniform. Reading (20,10) means \
         the property was never parsed.",
    );
    close(
        row("#s2", "#p6"),
        (20.0, 10.0, 80.0, 40.0),
        "#s2",
        "a one-value `scale: 2` IS uniform — the opposite of `translate`'s rule.",
    );
    close(
        row("#s3", "#p7"),
        (20.0, 10.0, 80.0, 60.0),
        "#s3",
        "`scale: 2 3`",
    );
    close(
        row("#s4", "#p8"),
        (20.0, 10.0, 20.0, 10.0),
        "#s4",
        "`scale: 50%` — a percentage is a number, not a length",
    );
    close(
        row("#s5", "#p9"),
        (40.0, 30.0, 40.0, 20.0),
        "#s5",
        "a percentage `translate` resolves against the element's OWN border box (40x20), not the \
         containing block",
    );
    close(
        row("#s6", "#p10"),
        (0.0, 10.0, 20.0, 40.0),
        "#s6",
        "`rotate: z 90deg` — the axis keyword spelling",
    );
    close(
        row("#s7", "#p11"),
        (0.0, 10.0, 20.0, 40.0),
        "#s7",
        "`rotate: 0 0 1 90deg` — the vector spelling of the same z rotation",
    );

    // ── THE ORDER IS THE SPEC'S, NOT THE DECLARATIONS'. Each pair is the same two properties
    //    written both ways round, and Chrome gives the same answer to both.
    for (a, ca, b, cb, want, why) in [
        (
            "#o1",
            "#p12",
            "#o2",
            "#p13",
            (30.0, 10.0, 20.0, 40.0),
            "translate then rotate",
        ),
        (
            "#o3",
            "#p14",
            "#o4",
            "#p15",
            (50.0, 10.0, 80.0, 40.0),
            "translate then scale",
        ),
        (
            "#o5",
            "#p16",
            "#o6",
            "#p17",
            (50.0, 10.0, 80.0, 40.0),
            "translate then the `transform` list",
        ),
    ] {
        close(row(a, ca), want, a, why);
        close(row(b, cb), want, b, why);
        assert_eq!(
            row(a, ca),
            row(b, cb),
            "G_TRANSFORM_INDIVIDUAL: {a} and {b} declare the same two properties in the OPPOSITE \
             order and must compose identically ({why}) — CSS Transforms 2 §3 fixes the order as \
             translate, rotate, scale, transform."
        );
    }
    close(
        row("#o7", "#p18"),
        (0.0, 20.0, 40.0, 80.0),
        "#o7",
        "all three together: translate(20,10), then rotate(90deg), then scale(2), about the \
         box's top-left",
    );
}
