//! # G_ABSPOS_IN_VERTICAL_CB — an out-of-flow box is NOT part of the subtree that gets transposed
//!
//! t1345 gave the engine vertical writing modes by TRANSPOSING a subtree: `writing_mode::plan`
//! walks the box tree, and every box under a `vertical-rl`/`vertical-lr` element has its style
//! rewritten in place (`transpose_in_place`) so the *existing* horizontal layout algorithm, run
//! unchanged, produces vertical geometry that a final `map_subtree` rotates back into physical
//! coordinates. That is a coordinate change, and it is correct for everything that participates in
//! the vertical block flow.
//!
//! An absolutely-positioned box does not participate in it. CSS Position §3 resolves it against a
//! CONTAINING BLOCK — the padding box of the nearest positioned ancestor — and CSS Writing Modes
//! §7.2 says its `top`/`right`/`bottom`/`left` are read in the containing block's *physical* axes
//! no matter what mode the box itself is in, while `inset-block-*`/`inset-inline-*` are read in the
//! writing mode of the CONTAINING BLOCK. Transposing such a box swaps its `width` with its `height`
//! and permutes its inset sides, then lays it out against a containing block whose rect is already
//! physical — the rotation is applied to a box that was never rotated, so it is applied ONCE TOO
//! MANY. The box comes out at ninety degrees, and `right`/`bottom` land on the wrong edges entirely.
//!
//! ## Chrome-measured (`--headless=new --dump-dom` + `getBoundingClientRect`)
//!
//! Both containers are `position:relative; width:200px; height:100px`; every child is
//! `position:absolute; width:5px; height:6px`. Rects are PARENT-RELATIVE, `id@x,y wxh`:
//!
//! ```text
//!   container                       child declaration                      Chrome
//!   .cb  vertical-rl                #a1  top:10px; left:20px               a1@20,10 5x6
//!   .cb  vertical-rl                #a2  top:10px; right:20px              a2@175,10 5x6
//!   .cb  vertical-rl                #a3  bottom:20px; left:20px            a3@20,74 5x6
//!   .cb  vertical-rl                #a4  inset:10px auto auto 20px         a4@20,10 5x6
//!   .cb2 vertical-lr + direction:rtl #b1 top:10px; left:20px               b1@20,10 5x6
//!   .cb2 vertical-lr + direction:rtl #b2 inset-block-start:10px;
//!                                        inset-inline-start:20px           b2@10,74 5x6
//! ```
//!
//! Read the rows against each other and the whole rule is in them:
//!
//! - **`a1` is at 20,10 — exactly where a horizontal container would put it.** `top`/`left` are
//!   physical and the containing block is physical, so the writing mode of the container changes
//!   NOTHING about this box. It is the row that says "do not transpose me".
//! - **`a2` at x=175 and `a3` at y=74** are `200-20-5` and `100-20-6`: `right` and `bottom` resolve
//!   against the containing block's physical right and bottom edges. Under the transposition these
//!   two sides get permuted into each other, so this pair is what separates "the box was rotated"
//!   from "the box happened to be square".
//! - **`a4` equals `a1`** — the `inset` shorthand is four physical sides, not a logical one.
//! ## The row that is NOT here — over-constrained insets (t1360 NEXT)
//!
//! `#a5 { inset: 10px 20px 30px 40px }` in `.cb` states BOTH `left` and `right`, which is
//! over-constrained. Chrome puts it at `175,10`; ours puts it at `40,10`. CSS 2.1 §10.3.7's
//! *"ignore `right` for `ltr`"* is really *"ignore the inset-INLINE-end side"*, and §10.6.4's
//! block-axis rule ignores the block-END side — in `vertical-rl` the block axis runs right-to-left,
//! so the side to drop is `left` and the answer is `200-20-5 = 175`. Ours drops `right`
//! unconditionally. That is a resolution-axis defect, a different mechanism from the exemption this
//! gate carries, so the row is recorded here rather than asserted:
//!
//! ```text
//!   .cb  vertical-rl   #a5  inset:10px 20px 30px 40px    Chrome a5@175,10 5x6    ours a5@40,10
//! ```
//!
//! - **`c1` and `d1` are the rows with NO insets**, where the box falls at its *static position* —
//!   "where it would have been in the flow". That position is computed inside the transposed
//!   subtree, so unlike every row above it must be mapped BACK: `vertical-rl` stacks blocks
//!   leftwards, so after a 30px in-flow block the next block-start edge is x=170 and a 5px-wide box
//!   grows leftward from it to x=165; `vertical-lr` stacks rightwards and the same box lands at
//!   x=30. The pair is the only thing in this gate that exercises `map_static_positions`, and the
//!   two directions disagree by 135px, so no single constant satisfies both.
//! - **`e1a`/`e1b` are the rows that say an out-of-flow box is exempt from the transposition
//!   WITHOUT its contents being exempt.** `writing-mode` inherits, so #e1's children are still in
//!   `vertical-rl`: their `width` is a BLOCK size, they stack leftward from #e1's right edge
//!   (40-8=32, then 32-12=20), and both sit at y=0. #e1 is therefore an orthogonal ROOT — its own
//!   box physical, its subtree transposed and mapped back — which is a different statement from
//!   "skip it", and an empty box cannot tell the two apart. Every other row here has no children,
//!   so before this pair the gate was satisfied by dropping `roots.insert` entirely.
//! - **`b2` is the mirror of `a1` and the only row that DOES move.** `vertical-lr` runs its block
//!   axis left-to-right, so `inset-block-start:20px`… no: `inset-block-start:10px` is `left:10px`.
//!   `direction:rtl` in a vertical mode runs the inline axis BOTTOM-TO-TOP, so `inset-inline-start`
//!   is a distance from the BOTTOM edge: `100-20-6 = 74`. Getting `b2` right while `b1` stays at
//!   `20,10` is what proves the logical insets are resolved in the CONTAINING BLOCK's mode rather
//!   than being ignored or being resolved in the box's own.
//!
//! ## How each assertion goes RED
//!
//! - **Delete the `out_of_flow` branch in `writing_mode::plan`** (make it always
//!   `transpose_in_place`) — the pre-tick state — and `a1` lands at `10,20` with a `6x5` rect: both
//!   the position and the SIZE come back transposed. `a2` goes to `x=0` and `a3` to `y=0`.
//! - **Keep the branch but drop `roots.insert(node, pm)`** (leave the box untransposed and NOT a
//!   root) and the box lays out correctly in its own axes but is never mapped back through
//!   `VerticalRun`, so it is positioned against the container's untransposed origin.
//! - **Delete `Layout::map_static_positions`** and a box with `auto` insets keeps the static
//!   position recorded in the transposed subtree's coordinates, which is the `b`-row shape.
//! - **Remove the `parent_is_flex_or_grid` exclusion** and the gate stays green while
//!   `css/css-grid/alignment/grid-*-axis-alignment-positioned-items-*` loses 38 subtests upstream —
//!   which is why that exclusion is asserted separately, in `g_abspos_grid_item_vertical.rs`.

use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><style>
  html, body { margin: 0; padding: 0; }
  .cb  { position: relative; writing-mode: vertical-rl; width: 200px; height: 100px; }
  .cb2 { position: relative; writing-mode: vertical-lr; direction: rtl;
         width: 200px; height: 100px; }
  .cb > div, .cb2 > div { position: absolute; width: 5px; height: 6px; }
  #a1 { top: 10px; left: 20px }
  #a2 { top: 10px; right: 20px }
  #a3 { bottom: 20px; left: 20px }
  #a4 { inset: 10px auto auto 20px }
  #a7 { inset-block-start: 10px; inset-inline-start: 20px }
  #b1 { top: 10px; left: 20px }
  #b2 { inset-block-start: 10px; inset-inline-start: 20px }
  .cb3 { position: relative; writing-mode: vertical-rl; width: 200px; height: 100px }
  .cb4 { position: relative; writing-mode: vertical-lr; width: 200px; height: 100px }
  .flow { width: 30px; height: 40px }
  #c1, #d1 { position: absolute; width: 5px; height: 6px }
  #e1 { position: absolute; top: 10px; left: 20px; width: 40px; height: 50px }
  .in { width: 8px; height: 9px }
  .in2 { width: 12px; height: 9px }
</style></head><body>
<div class="cb" id="cb"><div id="a1"></div><div id="a2"></div><div id="a3"></div>
  <div id="a4"></div><div id="a7"></div></div>
<div class="cb2" id="cb2"><div id="b1"></div><div id="b2"></div></div>
<div class="cb3" id="cb3"><div class="flow"></div><div id="c1"></div></div>
<div class="cb4" id="cb4"><div class="flow"></div><div id="d1"></div></div>
<div class="cb" id="cbe"><div id="e1"><div class="in" id="e1a"></div>
  <div class="in2" id="e1b"></div></div></div>
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
        .unwrap_or_else(|| panic!("no box for {sel} — the element has no geometry at all"))
}

#[test]
fn g_abspos_in_vertical_cb_is_physical() {
    // ⚠⚠ **ONE `#[test]` PER `Page`-BUILDING BINARY (t1342) — DO NOT ADD A SECOND.** SpiderMonkey
    // allows one JS thread per process; a second scripted test in this binary silently runs no
    // script or SIGSEGVs, and the symptom shows up in the OTHER test.
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://wm-abspos.test/", &fonts, 1200.0);
    let r = |sel: &str| rect_of(&page, sel);

    // ── THE CONTROL. The containing block itself is stated in physical pixels and must survive the
    // transposition unchanged; if it does not, every child offset below is measured off a moved
    // origin and the rest of this gate is comparing against the wrong thing.
    let (cb, cb2) = (r("#cb"), r("#cb2"));
    for (name, c) in [("#cb", cb), ("#cb2", cb2)] {
        assert!(
            (c.width - 200.0).abs() < 1.01 && (c.height - 100.0).abs() < 1.01,
            "G_ABSPOS_IN_VERTICAL_CB: the containing block {name} is {:.2}x{:.2}; the page states \
             `width:200px; height:100px` explicitly, so both must survive the vertical \
             transposition. A 100x200 here means the CONTAINER was rotated, and the child \
             assertions below cannot distinguish that from the bug they are looking for.",
            c.width,
            c.height
        );
    }

    // Every child states `width:5px; height:6px`. 5 and 6 are deliberately DIFFERENT so a
    // transposed box is visible in its SIZE alone, before any position is read.
    for id in [
        "#a1", "#a2", "#a3", "#a4", "#a7", "#b1", "#b2", "#c1", "#d1",
    ] {
        let k = r(id);
        assert!(
            (k.width - 5.0).abs() < 0.51 && (k.height - 6.0).abs() < 0.51,
            "G_ABSPOS_IN_VERTICAL_CB: {id} is {:.2}x{:.2}, not 5x6. `width` and `height` are \
             PHYSICAL on an absolutely-positioned box regardless of its containing block's writing \
             mode (CSS Writing Modes §7.2). 6x5 is the transposition applied to a box that never \
             entered the vertical flow — the pre-tick state.",
            k.width,
            k.height
        );
    }

    // ── 1. PHYSICAL INSETS IGNORE THE CONTAINER'S WRITING MODE. Chrome: a1@20,10.
    let a1 = r("#a1");
    assert!(
        ((a1.x - cb.x) - 20.0).abs() < 0.51 && ((a1.y - cb.y) - 10.0).abs() < 0.51,
        "G_ABSPOS_IN_VERTICAL_CB: `top:10px; left:20px` in a `vertical-rl` containing block put \
         #a1 at {:.2},{:.2}; it must be at 20,10 — literally the same place a horizontal container \
         would put it, because both the insets and the containing block are physical. 10,20 is the \
         transposition applied once too many (Chrome: a1@20,10).",
        a1.x - cb.x,
        a1.y - cb.y
    );

    // ── 2. `right` AND `bottom` RESOLVE AGAINST THE CONTAINING BLOCK'S PHYSICAL FAR EDGES.
    //    These are the rows the transposition permutes into each other, so they are what separates
    //    "correct" from "square by luck". Chrome: a2@175,10 and a3@20,74.
    let a2 = r("#a2");
    assert!(
        ((a2.x - cb.x) - 175.0).abs() < 0.51 && ((a2.y - cb.y) - 10.0).abs() < 0.51,
        "G_ABSPOS_IN_VERTICAL_CB: `right:20px` in a 200px-wide `vertical-rl` containing block put \
         #a2 at {:.2},{:.2}; `right` is the distance from the containing block's PHYSICAL right \
         edge, so it must be at 200-20-5 = 175. x=0 means `right` was permuted onto another side \
         by the transposition (Chrome: a2@175,10).",
        a2.x - cb.x,
        a2.y - cb.y
    );
    let a3 = r("#a3");
    assert!(
        ((a3.x - cb.x) - 20.0).abs() < 0.51 && ((a3.y - cb.y) - 74.0).abs() < 0.51,
        "G_ABSPOS_IN_VERTICAL_CB: `bottom:20px` in a 100px-tall `vertical-rl` containing block put \
         #a3 at {:.2},{:.2}; it must be at 100-20-6 = 74. y=0 means `bottom` was permuted onto the \
         block-start side, which in `vertical-rl` is the container's RIGHT edge (Chrome: a3@20,74).",
        a3.x - cb.x,
        a3.y - cb.y
    );

    // ── 3. THE `inset` SHORTHAND IS FOUR PHYSICAL SIDES, not a logical one. It must land exactly
    //    on top of #a1, which states the same thing as longhands. Chrome: a4@20,10.
    let a4 = r("#a4");
    assert!(
        ((a4.x - cb.x) - 20.0).abs() < 0.51 && ((a4.y - cb.y) - 10.0).abs() < 0.51,
        "G_ABSPOS_IN_VERTICAL_CB: `inset:10px auto auto 20px` put #a4 at {:.2},{:.2} while the \
         equivalent `top`/`left` longhands put #a1 at {:.2},{:.2}. `inset` expands to the four \
         PHYSICAL sides; a divergence between these two rows means the shorthand is being treated \
         as logical (Chrome: both @20,10).",
        a4.x - cb.x,
        a4.y - cb.y,
        a1.x - cb.x,
        a1.y - cb.y
    );

    // ── 3b. LOGICAL INSETS IN `vertical-rl`. The block axis runs RIGHT-TO-LEFT, so
    //    `inset-block-start` is a distance from the container's RIGHT edge; the inline axis (ltr)
    //    runs top-to-bottom, so `inset-inline-start` is `top`. Chrome: a7@185,20. Paired with #b2
    //    below — the same two declarations in `vertical-lr; direction:rtl` — these two rows are
    //    what force the mapping to READ both the mode and the direction rather than pick one.
    let a7 = r("#a7");
    assert!(
        ((a7.x - cb.x) - 185.0).abs() < 0.51 && ((a7.y - cb.y) - 20.0).abs() < 0.51,
        "G_ABSPOS_IN_VERTICAL_CB: `inset-block-start:10px; inset-inline-start:20px` in a \
         `vertical-rl` containing block put #a7 at {:.2},{:.2}; it must be at 185,20. `vertical-rl` \
         runs the BLOCK axis right-to-left, so `inset-block-start` is `right:10px` = 200-10-5 = 185, \
         and its inline axis runs top-to-bottom, so `inset-inline-start` is `top:20px`. 20,10 means \
         the logical insets were read as physical; 10,20 means they were read in the horizontal \
         mode (Chrome: a7@185,20).",
        a7.x - cb.x,
        a7.y - cb.y
    );

    // ── 4. THE MIRROR: LOGICAL INSETS *ARE* RESOLVED, IN THE CONTAINING BLOCK'S MODE.
    //    Without this row the gate is satisfied by an engine that simply ignores the writing mode
    //    for every out-of-flow box — which would be wrong in the other direction. `#b1` states
    //    physical insets in the SAME container and must not move, so the pair localises it.
    let (b1, b2) = (r("#b1"), r("#b2"));
    assert!(
        ((b1.x - cb2.x) - 20.0).abs() < 0.51 && ((b1.y - cb2.y) - 10.0).abs() < 0.51,
        "G_ABSPOS_IN_VERTICAL_CB: `top:10px; left:20px` in a `vertical-lr; direction:rtl` block put \
         #b1 at {:.2},{:.2}; physical insets are physical in EVERY mode, including RTL ones, so it \
         must be at 20,10 exactly as in the `vertical-rl` container (Chrome: b1@20,10).",
        b1.x - cb2.x,
        b1.y - cb2.y
    );
    assert!(
        ((b2.x - cb2.x) - 10.0).abs() < 0.51 && ((b2.y - cb2.y) - 74.0).abs() < 0.51,
        "G_ABSPOS_IN_VERTICAL_CB: `inset-block-start:10px; inset-inline-start:20px` in a \
         `vertical-lr; direction:rtl` containing block put #b2 at {:.2},{:.2}; it must be at 10,74. \
         `vertical-lr` runs the BLOCK axis left-to-right, so `inset-block-start` is `left:10px`; \
         `direction:rtl` in a vertical mode runs the INLINE axis bottom-to-top, so \
         `inset-inline-start` is a distance from the BOTTOM edge: 100-20-6 = 74. 20,10 means the \
         logical insets were treated as physical — the failure mode that skipping the whole \
         transposition for out-of-flow boxes would introduce, which is why #b1 above pins the \
         physical case in this SAME container (Chrome: b2@10,74).",
        b2.x - cb2.x,
        b2.y - cb2.y
    );

    // ── 5. THE STATIC POSITION IS COMPUTED IN THE TRANSPOSED SUBTREE AND MUST BE MAPPED BACK.
    //    Every row above states an inset, and an inset is resolved against a containing block whose
    //    rect is already physical — so none of them can see whether the *static* position (CSS
    //    Position §3, "where the box would have been in the flow") survived the coordinate change.
    //    These two do, and they are the only rows here that exercise `map_static_positions`.
    //    The two directions land 135px apart, so a stub that returns any single constant fails one.
    let (cb3, c1) = (r("#cb3"), r("#c1"));
    assert!(
        ((c1.x - cb3.x) - 165.0).abs() < 1.01 && ((c1.y - cb3.y) - 0.0).abs() < 1.01,
        "G_ABSPOS_IN_VERTICAL_CB: an inset-less absolute box after a 30px in-flow block in a \
         `vertical-rl` container is at {:.2},{:.2}; it must be at 165,0. The in-flow block consumes \
         30px of the BLOCK axis from the container's right edge, leaving the next block-start edge \
         at x=170, and `vertical-rl` grows leftward so a 5px-wide box runs 165..170. A value in the \
         low tens is the static position left in the TRANSPOSED subtree's coordinates and never \
         mapped back — delete `Layout::map_static_positions` and this is the row that goes red \
         (Chrome: c1@165,0).",
        c1.x - cb3.x,
        c1.y - cb3.y
    );
    let (cb4, d1) = (r("#cb4"), r("#d1"));
    assert!(
        ((d1.x - cb4.x) - 30.0).abs() < 1.01 && ((d1.y - cb4.y) - 0.0).abs() < 1.01,
        "G_ABSPOS_IN_VERTICAL_CB: the same inset-less box in a `vertical-lr` container is at \
         {:.2},{:.2}; it must be at 30,0 — `vertical-lr` stacks blocks RIGHTWARDS, so the static \
         position sits just past the 30px in-flow block rather than 135px away from it as in the \
         `vertical-rl` row above. Matching that row's 165 here means the mapping ignores the \
         direction and applies `vertical-rl`'s origin to both (Chrome: d1@30,0).",
        d1.x - cb4.x,
        d1.y - cb4.y
    );

    // ── 6. EXEMPT, NOT SKIPPED: THE BOX IS PHYSICAL AND ITS SUBTREE IS STILL VERTICAL.
    //    `writing-mode` inherits, so an out-of-flow box in a vertical container is an ORTHOGONAL
    //    ROOT — its own margin box laid out in the containing block's physical axes, its contents
    //    laid out transposed and mapped back through their own `VerticalRun`. Every row above uses
    //    an EMPTY box, which cannot distinguish that from simply not transposing anything, and the
    //    mutation that drops `roots.insert(node, pm)` passed this gate until these two rows existed.
    let e1 = r("#e1");
    let (e1a, e1b) = (r("#e1a"), r("#e1b"));
    assert!(
        (e1.width - 40.0).abs() < 0.51 && (e1.height - 50.0).abs() < 0.51,
        "G_ABSPOS_IN_VERTICAL_CB: #e1 is {:.2}x{:.2}, not 40x50 — the out-of-flow box's own \
         geometry must be physical before its children can be read against it (Chrome: 40x50).",
        e1.width,
        e1.height
    );
    assert!(
        ((e1a.x - e1.x) - 32.0).abs() < 1.01
            && ((e1a.y - e1.y) - 0.0).abs() < 1.01
            && (e1a.width - 8.0).abs() < 0.51,
        "G_ABSPOS_IN_VERTICAL_CB: the first in-flow child of an out-of-flow box in a `vertical-rl` \
         container is at {:.2},{:.2} sized {:.2}x{:.2}; it must be 8x9 at 32,0. `writing-mode` \
         INHERITS across `position:absolute`, so this child is still in vertical flow: its \
         `width:8px` is a BLOCK size and it hugs #e1's right edge at 40-8 = 32. Landing at 0,0 at \
         full width means the subtree was exempted along with the box — #e1 must be an orthogonal \
         ROOT, not a hole in the transposition (Chrome: e1a@32,0 8x9).",
        e1a.x - e1.x,
        e1a.y - e1.y,
        e1a.width,
        e1a.height
    );
    assert!(
        ((e1b.x - e1.x) - 20.0).abs() < 1.01 && ((e1b.y - e1.y) - 0.0).abs() < 1.01,
        "G_ABSPOS_IN_VERTICAL_CB: the SECOND in-flow child is at {:.2},{:.2}; it must be at 20,0 — \
         one 12px block further LEFT than its 8px-wide sibling at 32. Both children sharing an x, \
         or stacking downward instead, means the subtree was laid out in horizontal flow and only \
         its origin was moved (Chrome: e1b@20,0 12x9).",
        e1b.x - e1.x,
        e1b.y - e1.y
    );
}
