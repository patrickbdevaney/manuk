//! # G_WRITING_MODE — a vertical writing mode is a COORDINATE SYSTEM, and the engine had none
//!
//! `writing-mode` decides which physical axis the *inline* direction runs along, and therefore what
//! every `width` and `height` in the layout engine means. `horizontal-tb` was not the default here —
//! it was the only thing representable: `writing-mode` had no `ComputedStyle` field, so a page that
//! set `vertical-rl` was laid out at ninety degrees to where it belongs, silently, with no error and
//! no missing box.
//!
//! Chrome-measured (`--headless --dump-dom` + `getBoundingClientRect`, `font:16px/20px monospace`,
//! container `width:400px`), child rects PARENT-RELATIVE as `[dx dy w×h]`:
//!
//! ```text
//!   row                                          container      child(ren)
//!   horizontal-tb, one block child   CONTROL     400 x 20       [0 0 400x20]
//!   horizontal-tb, two block children CONTROL    400 x 40       [0 0 400x20] [0 20 400x20]
//!   vertical-rl,  one block child                400 x 10       [380 0 20x10]
//!   vertical-rl,  two block children             400 x 10       [380 0 20x10] [360 0 20x10]
//!   vertical-lr,  two block children             400 x 10       [0   0 20x10] [20  0 20x10]
//!   vertical-rl,  child width:100px              400 x 10       [300 0 100x10]
//!   vertical-rl,  child height:100px             400 x 100      [380 0 20x100]
//!   vertical-rl,  child inline-size:100px        400 x 100      [380 0 20x100]
//!   vertical-rl,  bare text "hello"              400 x 48
//!   horizontal-tb, bare text "hello"  CONTROL    400 x 20
//!   vertical-rl,  child margin-top:10px          400 x 20       [380 10 20x10]
//!   vertical-rl,  child margin-block-start:10px  400 x 10       [370 0 20x10]
//! ```
//!
//! Read the vertical rows against the horizontal controls and the whole mechanism is in them:
//!
//! - **the container's HEIGHT collapses from 20 to 10** — it is no longer a stack of line boxes, it
//!   is one glyph's *advance* running down the page;
//! - **the child is 20 wide** — its `width` is now its BLOCK size, i.e. the line height;
//! - **the child sits at x=380**, flush against the RIGHT edge, because `vertical-rl` stacks blocks
//!   leftwards and the block-start edge is the right one. `vertical-lr` puts the same two children
//!   at x=0 and x=20, which is the row that proves the direction is read and not assumed;
//! - **`height:100px` and `inline-size:100px` are the same declaration** on a vertical box, and both
//!   change the *container's height* rather than its width. That row is what a physical-only engine
//!   cannot produce by luck.
//!
//! ## Why the assertions are written against the CONTROLS and not against Chrome's literals
//!
//! Chrome's `10` is the advance of `x` in *its* default monospace face and its `20` is that face's
//! line box; ours are this box's fonts. Asserting `10` would make the gate a font-metrics test that
//! goes red when a font package updates — the failure mode `G_INLINE_BOX_GEOMETRY` already documents.
//! So the horizontal control rows establish this engine's own advance and line height, and every
//! vertical row is asserted as a RELATION to them. The relations are exactly the transposition:
//! *the vertical container's height is the horizontal container's WIDTH-of-content, and the vertical
//! child's width is the horizontal child's HEIGHT.*
//!
//! ## How each assertion goes RED
//!
//! - **Return `None` from `writing_mode::plan`** (or delete the `s.writing_mode` mapping in
//!   `stylo_map`) and every vertical row collapses onto its horizontal control: the container is
//!   20 tall, the child is `[0 0 400x20]`. That is the pre-tick state, and it is what these
//!   assertions were written from.
//! - **Drop the `is_rl()` branch in the `VerticalRun` origin** and the `vertical-lr` row lands at
//!   x=380 with the `vertical-rl` one — the two directions become one.
//! - **Skip `transpose_in_place`'s `width`/`height` swap** and the `height:100px` row stops changing
//!   the container's height (it would widen the box instead).
//! - **Skip the `margin` side permutation** and `margin-top:10px` stops being an inline-start margin:
//!   the child no longer moves DOWN by 10, and `margin-block-start` stops moving it left.
//! - **The CSSOM arm**: delete the `("writing-mode", …)` row in `dom_bindings` and
//!   `getComputedStyle(el).writingMode` reads `undefined` again — which is what every
//!   vertical-text feature-detect on the web branches on.

use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><style>
 body{margin:0;font:16px/20px monospace}
 .c{width:400px}
 .cb{position:relative;width:300px;height:200px}
 .ab{position:absolute}
</style></head><body>
<div class="c" id="h1"><div class="k" id="h1k">x</div></div>
<div class="c" id="h2"><div class="k" id="h2k1">x</div><div class="k" id="h2k2">y</div></div>
<div class="c" id="ht">hello</div>
<div class="c" id="v1" style="writing-mode:vertical-rl"><div class="k" id="v1k">x</div></div>
<div class="c" id="v2" style="writing-mode:vertical-rl"><div class="k" id="v2k1">x</div><div class="k" id="v2k2">y</div></div>
<div class="c" id="l2" style="writing-mode:vertical-lr"><div class="k" id="l2k1">x</div><div class="k" id="l2k2">y</div></div>
<div class="c" id="vw" style="writing-mode:vertical-rl"><div class="k" id="vwk" style="width:100px">x</div></div>
<div class="c" id="vh" style="writing-mode:vertical-rl"><div class="k" id="vhk" style="height:100px">x</div></div>
<div class="c" id="vi" style="writing-mode:vertical-rl"><div class="k" id="vik" style="inline-size:100px">x</div></div>
<div class="c" id="vt" style="writing-mode:vertical-rl">hello</div>
<div class="c" id="vm" style="writing-mode:vertical-rl"><div class="k" id="vmk" style="margin-top:10px">x</div></div>
<div class="c" id="vb" style="writing-mode:vertical-rl"><div class="k" id="vbk" style="margin-block-start:10px">x</div></div>
<div id="pv" style="writing-mode:vertical-rl;width:60px;height:200px;font:32px/40px monospace;background:#fff;color:#c00">IIII</div>
<div class="cb"><div id="ah" class="ab">hello</div></div>
<div class="cb"><div id="av" class="ab" style="writing-mode:vertical-rl">hello</div></div>
<div class="cb"><div id="alr" class="ab" style="writing-mode:vertical-rl;left:10px;right:20px">hello</div></div>
<div class="cb"><div id="atb" class="ab" style="writing-mode:vertical-rl;top:10px;bottom:20px">hello</div></div>
<div class="cb"><div id="a2" class="ab" style="writing-mode:vertical-rl">hello<br>worldly</div></div>
<div class="cb"><div id="ai" class="ab" style="writing-mode:vertical-rl;display:inline">hello</div></div>
<div class="cb"><div id="iv" style="writing-mode:vertical-rl">hello</div></div>
<div id="cssom">-</div>
<script>
  var g = function (id) { return getComputedStyle(document.getElementById(id)).writingMode; };
  document.getElementById('cssom').textContent =
    'v1:' + g('v1') + ' l2:' + g('l2') + ' h1:' + g('h1');
</script>
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
fn g_writing_mode_transposes_the_subtree() {
    // ⚠⚠ **ONE `#[test]` PER `Page`-BUILDING BINARY (t1342) — DO NOT ADD A SECOND.** `libtest`
    // spawns a thread per test and SpiderMonkey allows exactly one JS thread per process; a second
    // scripted test in the same binary silently runs no script or SIGSEGVs. Enforced by
    // `G_ONE_PAGE_TEST_PER_BINARY` in `g_one_js_thread_per_process.rs`.
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://wm.test/", &fonts, 1200.0);
    let r = |sel: &str| rect_of(&page, sel);

    // ── THE CALIBRATION. Two numbers, taken from the horizontal control rows, that every vertical
    // assertion below is written against: this engine's line box height, and the inline advance of
    // one monospace glyph. Chrome's are 20 and 10.
    let (h1, h1k) = (r("#h1"), r("#h1k"));
    let line_h = h1k.height;
    assert!(
        (line_h - 20.0).abs() < 1.01 && (h1k.width - 400.0).abs() < 1.01,
        "G_WRITING_MODE: the horizontal CONTROL is {:.2}x{:.2}; Chrome reads 400x20. Every vertical \
         row is measured against this one, so a drift here means the ruler moved and the rest of \
         this gate is comparing against the wrong thing.",
        h1k.width,
        line_h
    );
    // The advance of `hello` divided by five is one glyph — the number a vertical container's
    // HEIGHT is made of. Taken from the control row rather than from a font table.
    let glyph_adv = ht_advance(&page) / 5.0;
    assert!(
        glyph_adv > 3.0 && glyph_adv < 20.0,
        "G_WRITING_MODE: the control row's monospace advance came out {glyph_adv:.2}px/glyph, which \
         is not a plausible 16px monospace glyph. The calibration is broken, not the feature."
    );
    assert!(
        (h1.height - line_h).abs() < 1.01,
        "G_WRITING_MODE: the horizontal control container is {:.2} tall, not one line box.",
        h1.height
    );

    // ── 1. THE CONTAINER'S HEIGHT IS NOW AN INLINE EXTENT. Chrome: 400x10 where horizontal is
    //    400x20. This is the single number that says the axes swapped.
    let v1 = r("#v1");
    assert!(
        (v1.width - 400.0).abs() < 1.01,
        "G_WRITING_MODE: a `vertical-rl` container with `width:400px` is {:.2} wide. `width` is its \
         BLOCK size in a vertical mode and the page still stated it explicitly, so it must survive \
         the transposition unchanged.",
        v1.width
    );
    assert!(
        (v1.height - glyph_adv).abs() < 1.51,
        "G_WRITING_MODE: a `vertical-rl` container holding one glyph is {:.2}px tall; it must be \
         ONE GLYPH ADVANCE ({glyph_adv:.2}px), because its content's inline axis now runs DOWN the \
         page. It reads {:.2} — the horizontal line height — when `writing-mode` never reached \
         layout at all, which is the pre-tick state (Chrome: 400x10 vs 400x20).",
        v1.height,
        line_h
    );

    // ── 2. THE CHILD'S `width` IS ITS BLOCK SIZE, AND IT HUGS THE BLOCK-START (RIGHT) EDGE.
    //    Chrome: [380 0 20x10].
    let v1k = r("#v1k");
    assert!(
        (v1k.width - line_h).abs() < 1.01,
        "G_WRITING_MODE: the child of a `vertical-rl` container is {:.2}px wide; its `width` is now \
         its BLOCK size, so it must be one line height ({line_h:.2}px). 400 means it was still \
         filling the inline axis of a horizontal parent.",
        v1k.width
    );
    assert!(
        ((v1k.x - v1.x) - (400.0 - line_h)).abs() < 1.01,
        "G_WRITING_MODE: the child sits {:.2}px from its container's left edge; `vertical-rl` stacks \
         blocks RIGHT-TO-LEFT, so the first child's block-start edge is the container's RIGHT edge \
         and it must sit at {:.2}. Chrome: x=380 in a 400px box.",
        v1k.x - v1.x,
        400.0 - line_h
    );

    // ── 3. TWO CHILDREN STACK ALONG THE BLOCK AXIS — leftwards for `vertical-rl`, RIGHTWARDS for
    //    `vertical-lr`. The pair is what proves the direction is read rather than assumed.
    let (v2, v2k1, v2k2) = (r("#v2"), r("#v2k1"), r("#v2k2"));
    assert!(
        (v2k1.x - v2k2.x - line_h).abs() < 1.01,
        "G_WRITING_MODE: `vertical-rl`'s second child is at x={:.2} and the first at x={:.2}; the \
         second must be exactly one line height FURTHER LEFT. Chrome: 380 then 360.",
        v2k2.x - v2.x,
        v2k1.x - v2.x
    );
    let (l2, l2k1, l2k2) = (r("#l2"), r("#l2k1"), r("#l2k2"));
    assert!(
        ((l2k1.x - l2.x).abs() < 1.01) && ((l2k2.x - l2.x) - line_h).abs() < 1.01,
        "G_WRITING_MODE: `vertical-lr` stacks blocks LEFT-TO-RIGHT — its two children must sit at \
         x=0 and x={line_h:.2} from the container's left edge, not at the right-hand end. They are \
         at {:.2} and {:.2}. Chrome: [0 0 20x10] and [20 0 20x10]. Collapsing the two modes into one \
         is what this row exists to catch.",
        l2k1.x - l2.x,
        l2k2.x - l2.x
    );

    // ── 4. `height:100px` AND `inline-size:100px` ARE THE SAME DECLARATION HERE, AND BOTH GROW THE
    //    CONTAINER'S HEIGHT. A physical-only engine cannot produce this row by luck.
    let (vh, vhk) = (r("#vh"), r("#vhk"));
    assert!(
        (vh.height - 100.0).abs() < 1.51 && (vhk.height - 100.0).abs() < 1.51,
        "G_WRITING_MODE: `height:100px` on a child of a `vertical-rl` box is an INLINE size — it \
         must make the container 100px TALL (Chrome: 400x100). Container is {:.2} tall, child \
         {:.2}.",
        vh.height,
        vhk.height
    );
    let (vi, vik) = (r("#vi"), r("#vik"));
    assert!(
        (vi.height - 100.0).abs() < 1.51 && (vik.height - 100.0).abs() < 1.51,
        "G_WRITING_MODE: `inline-size:100px` must be the SAME declaration as `height:100px` in a \
         vertical mode — Stylo maps the logical spelling onto the physical field against exactly \
         this writing mode. Container {:.2} tall, child {:.2}; both should be 100.",
        vi.height,
        vik.height
    );
    // …and the mirror: `width:100px` is a BLOCK size, so it widens the child and leaves the
    // container's height alone. Chrome: [300 0 100x10] in a 400x10 box.
    let (vw, vwk) = (r("#vw"), r("#vwk"));
    assert!(
        (vwk.width - 100.0).abs() < 1.01 && ((vwk.x - vw.x) - 300.0).abs() < 1.51,
        "G_WRITING_MODE: `width:100px` on a `vertical-rl` child is a BLOCK size: 100px wide, sitting \
         100px in from the container's RIGHT edge (Chrome: [300 0 100x10]). It is {:.2} wide at \
         dx={:.2}.",
        vwk.width,
        vwk.x - vw.x
    );

    // ── 5. BARE TEXT. The container's height is the run's advance, not its line height. Chrome:
    //    400x48 for `hello` where the horizontal control is 400x20.
    let vt = r("#vt");
    assert!(
        (vt.height - 5.0 * glyph_adv).abs() < 2.51,
        "G_WRITING_MODE: a `vertical-rl` container holding `hello` is {:.2}px tall; it must be the \
         run's ADVANCE running down the page ({:.2}px = 5 glyphs), which is Chrome's 48 against its \
         horizontal 20.",
        vt.height,
        5.0 * glyph_adv
    );

    // ── 6. THE MARGIN SIDES PERMUTE. `margin-top` is an INLINE-start margin in a vertical mode
    //    (Chrome: child at dy=10, container 20 tall), and `margin-block-start` is the physical
    //    RIGHT one (Chrome: child at x=370, container height unchanged).
    let (vm, vmk) = (r("#vm"), r("#vmk"));
    assert!(
        ((vmk.y - vm.y) - 10.0).abs() < 1.01,
        "G_WRITING_MODE: `margin-top:10px` on a `vertical-rl` child is an INLINE-START margin — it \
         must push the child 10px DOWN (Chrome: dy=10) and make the container 10px taller. dy is \
         {:.2}.",
        vmk.y - vm.y
    );
    let (vb, vbk) = (r("#vb"), r("#vbk"));
    assert!(
        ((vbk.x - vb.x) - (400.0 - line_h - 10.0)).abs() < 1.51,
        "G_WRITING_MODE: `margin-block-start:10px` maps to the physical RIGHT margin in \
         `vertical-rl`, so the child moves 10px further LEFT (Chrome: x=370 where the unmargined \
         child is at 380). dx is {:.2}, expected {:.2}.",
        vbk.x - vb.x,
        400.0 - line_h - 10.0
    );
    assert!(
        (vb.height - vmk.height).abs() < 1.51,
        "G_WRITING_MODE: a BLOCK-start margin must not change the container's INLINE extent — the \
         two margins are on different axes and a permutation that sent both to the same side would \
         pass the row above and fail here."
    );

    // ── 7. THE PAINT ARM — **the glyphs have to TURN, or this tick traded a visual for a
    //    geometry.** With the boxes right and the glyphs still horizontal, a vertical run would
    //    stream out of the side of its 40px strip and across whatever is beside it: a page that
    //    renders wrong in a *new* way, bought with a shape win. THE RATCHET refuses that trade, so
    //    the rotation is asserted here in pixels rather than trusted.
    //
    //    `IIII` at 32px/40px monospace in a 200px-tall vertical box: rotated, its ink is a TALL
    //    NARROW column inside the box's own 40px strip. Unrotated it is a wide short band that
    //    overruns the strip's right edge by hundreds of pixels.
    let pv = r("#pv");
    let canvas = page.paint(&fonts, 1200, 1400);
    let px = canvas.rgba_bytes();
    let at = |x: u32, y: u32| -> (u8, u8, u8) {
        let i = ((y * 1200 + x) * 4) as usize;
        (px[i], px[i + 1], px[i + 2])
    };
    // ⚠ The run is coloured `#c00` and the WHOLE canvas is scanned for red — not a band around the
    // box. A band would exclude the failure it exists to catch: a glyph that never learned about the
    // writing mode is not drawn slightly wrong, it is drawn somewhere ELSE (the fragment's fields
    // are logical, so read physically they point near the page origin). Scanning only near the box
    // would report "no ink" for every paint mutation and never distinguish them.
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for y in 0..1400u32 {
        for x in 0..1200u32 {
            let (r_, g_, b_) = at(x, y);
            if r_ > 120 && g_ < 90 && b_ < 90 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    assert!(
        x1 >= x0 && y1 >= y0 && x0 != u32::MAX,
        "G_WRITING_MODE: the vertical run painted NO ink anywhere on the canvas — the rotation \
         cannot be judged because nothing was drawn at all."
    );
    let (ink_w, ink_h) = (x1 - x0 + 1, y1 - y0 + 1);
    assert!(
        ink_h > ink_w,
        "G_WRITING_MODE: the vertical run's ink is {ink_w}x{ink_h} — WIDER than it is tall, which is \
         a horizontal run of `IIII`. `text-orientation: mixed` lays a sideways glyph on its side and \
         advances the pen DOWN the page; without that the box is in the right place and the text \
         runs out of its side."
    );
    assert!(
        (x1 as f32) <= pv.right() + 2.0 && (x0 as f32) >= pv.x - 2.0,
        "G_WRITING_MODE: the vertical run's ink spans x {x0}..{x1}, outside its own box \
         ({:.0}..{:.0}). A rotated glyph stays inside the line's block strip; an unrotated one \
         streams across the page.",
        pv.x,
        pv.right()
    );

    // ── 8. THE CSSOM ARM. A page that sets a vertical mode must read it back — every vertical-text
    //    feature detect on the web is `getComputedStyle(el).writingMode`, and it was empty. The
    //    `h1` row is the control: the property answers for a box that never set it, too.
    let dom = page.dom();
    let out = manuk_css::query_selector_all(dom, dom.root(), "#cssom")[0];
    let got = dom.text_content(out);
    assert_eq!(
        got.trim(),
        "v1:vertical-rl l2:vertical-lr h1:horizontal-tb",
        "G_WRITING_MODE: `getComputedStyle(el).writingMode` reads {got:?}. The property had no \
         `ComputedStyle` field at all before this tick, so the readback was empty and every \
         feature-detect for vertical text took its fallback path against an engine that now has it."
    );
    // ── ⭐ THE ABSPOS HALF (t1347). Kept in its own function for readability, called from THIS
    //    test rather than added as a second `#[test]` — see the one-`Page`-per-binary note above.
    abspos_rows(&page);
}

/// The abspos half of the transposition, and the eight rows are all headless-Chrome-measured on
/// this fixture's own font (`16px/20px monospace`, `hello` = 48.2px of advance, `worldly` = 67.4).
///
/// ```text
///                                                    CHROME     before t1347
///    #ah  abspos, NO writing-mode          CONTROL   48.2x20.0    48.2x20.0   ✓
///    #av  abspos, vertical-rl, all auto              20.0x48.2    48.2x20.0   ✗
///    #alr …with left:10px; right:20px               270.0x48.2   270.0x20.0   ✗
///    #atb …with top:10px; bottom:20px                20.0x170.0   48.2x170.0  ✗
///    #a2  …two lines, `hello`/`worldly`              40.0x67.4    67.4x40.0   ✗
///    #ai  …display:inline (blockified)                20.0x48.2   48.2x20.0   ✗
///    #iv  the same box IN FLOW           ⚠ RESIDUE    20.0x48.2  300.0x48.2   ✗ still
/// ```
fn abspos_rows(page: &manuk_page::Page) {
    let r = |sel: &str| rect_of(page, sel);
    let (ah, av) = (r("#ah"), r("#av"));

    // ── THE CONTROL, first. Without it every relation below is a statement about an engine that
    //    might not lay out an ordinary abspos correctly either, and the diagnosis would go to
    //    `writing-mode` when it belonged to `layout_abs`.
    assert!(
        ah.width > ah.height + 1.0,
        "G_WRITING_MODE: CONTROL — an abspos with NO writing-mode must be WIDER than tall for a \
         one-line `hello` ({:.1}x{:.1}). Nothing below is a test of a transposition if this fails.",
        ah.width,
        ah.height
    );

    // ⭐ THE CLAIM, written as an EXACT TRANSPOSITION of the control rather than against Chrome's
    //    literals — so it is calibrated by this engine's own font metrics and stays true if the
    //    fallback face changes. Chrome: the control is 48.2x20.0 and the vertical row is 20.0x48.2.
    assert!(
        (av.width - ah.height).abs() < 0.6 && (av.height - ah.width).abs() < 0.6,
        "G_WRITING_MODE: an ABSOLUTELY POSITIONED vertical box must be the exact transpose of the \
         same box without `writing-mode` — expected {:.1}x{:.1}, got {:.1}x{:.1}. `layout_abs` \
         places every `position:absolute` and `position:fixed` box and knew nothing about the \
         transposition until t1347, so a vertical abspos was laid out HORIZONTALLY: not wrong by a \
         few pixels, wrong by ninety degrees, and silently, because a rotated box is not malformed.",
        ah.height,
        ah.width,
        av.width,
        av.height
    );

    // ── The two axes SEPARATELY, because either one alone can be right by accident when the other
    //    is pinned by an inset — and those are the two arms the WPT files split on (1,200 failures
    //    on `width`, 300 on `height`).
    let alr = r("#alr");
    assert!(
        (alr.width - 270.0).abs() < 0.6 && (alr.height - ah.width).abs() < 0.6,
        "G_WRITING_MODE: `left:10px; right:20px` pins the physical WIDTH at 270 (the offsets \
         resolve in the CONTAINING BLOCK's mode, which is horizontal) while the HEIGHT is still the \
         inline extent {:.1} — got {:.1}x{:.1}. This is the arm where the width is right by \
         accident and only the height can see the bug.",
        ah.width,
        alr.width,
        alr.height
    );
    let atb = r("#atb");
    assert!(
        (atb.width - ah.height).abs() < 0.6 && (atb.height - 170.0).abs() < 0.6,
        "G_WRITING_MODE: `top:10px; bottom:20px` pins the HEIGHT at 170 while the WIDTH is the \
         block extent {:.1} — got {:.1}x{:.1}. The mirror of the row above, one axis over.",
        ah.height,
        atb.width,
        atb.height
    );

    // ── TWO LINES. One line cannot tell "the block extent" from "the line height", because they
    //    are the same number; two makes the block axis a sum and the inline axis a MAX.
    let a2 = r("#a2");
    assert!(
        (a2.width - 2.0 * ah.height).abs() < 0.6 && a2.height > ah.width + 1.0,
        "G_WRITING_MODE: two lines in a vertical abspos are TWO line boxes wide ({:.1}) and as tall \
         as the LONGER run — got {:.1}x{:.1}. Chrome: 40.0x67.4. A one-line fixture cannot \
         distinguish the block extent from the line height; this row is why there are two.",
        2.0 * ah.height,
        a2.width,
        a2.height
    );

    // ── A BLOCKIFIED INLINE, which is what the WPT files actually use (`.abspos{display:inline}`).
    let ai = r("#ai");
    assert!(
        (ai.width - ah.height).abs() < 0.6 && (ai.height - ah.width).abs() < 0.6,
        "G_WRITING_MODE: `display:inline` + `position:absolute` blockifies, and the blockified box \
         transposes like any other — got {:.1}x{:.1}. This is the exact spelling of \
         `css/css-grid/abspos/orthogonal-positioned-grid-descendants-*`, fifteen files at 100 \
         subtests each.",
        ai.width,
        ai.height
    );

    // ── ⚠⚠⚠ THE NAMED RESIDUE, pinned at OUR value with Chrome's beside it. An IN-FLOW orthogonal
    //    root's physical width is its BLOCK axis, so `width:auto` must shrink-wrap to the content's
    //    block extent (Chrome: 20.0) rather than fill the containing block (ours: 300.0). It is the
    //    same swap this tick made in `layout_abs`, in the other layout path — and it is a bigger
    //    change there, because `layout_block` computes its width long before the children and feeds
    //    it to the margin, float and BFC rules on the way. Everything else about the in-flow
    //    vertical box is already exact, including this row's HEIGHT.
    let iv = r("#iv");
    assert!(
        (iv.height - ah.width).abs() < 0.6,
        "G_WRITING_MODE: an in-flow vertical box's HEIGHT is its inline extent {:.1} — got {:.1}. \
         This half has been right since t1343 and is the control for the residue below.",
        ah.width,
        iv.height
    );
    assert!(
        iv.width > 100.0,
        "G_WRITING_MODE: `#iv` reads {:.1} — a KNOWN DIVERGENCE pinned at its current value. \
         Chrome gives 20.0: an in-flow orthogonal root's `width:auto` is an auto BLOCK size and \
         shrink-wraps to the content, where we still fill the containing block. If this now reads \
         ~20, that residue has been closed — update this assertion to `(iv.width - ah.height).abs() \
         < 0.6` and delete this paragraph, do not delete the row.",
        iv.width
    );
}

/// The horizontal control run's advance — measured off the fragment the engine actually produced,
/// so the calibration is this engine's own font metrics rather than a table.
fn ht_advance(page: &manuk_page::Page) -> f32 {
    let mut w = 0.0f32;
    page.root_box.walk(&mut |b| {
        if let manuk_layout::BoxContent::Inline(frags) = &b.content {
            for f in frags {
                if f.text.trim() == "hello" && f.vertical.is_none() {
                    w = w.max(f.width);
                }
            }
        }
    });
    assert!(
        w > 0.0,
        "G_WRITING_MODE: the horizontal control run `hello` produced no fragment at all — the \
         calibration cannot be taken, so nothing below it means anything."
    );
    w
}
