//! **G_SVG_AUTO_SIZING — an `<svg viewBox="0 0 100 25">` with no width/height laid out at 100×25:
//! its own coordinate numbers, read as if they were pixels.**
//!
//! `viewBox` is an intrinsic **RATIO**, never an intrinsic **SIZE** (SVG2 §8.2 + the CSS-Images
//! §5.3.2 default sizing algorithm). An outermost `<svg>` with `width:auto` fills its containing
//! block and takes its height from the ratio. Chrome-measured on one fixture, and the two container
//! widths are the point — they are what separate "fills the containing block" from "some constant":
//!
//! ```text
//!                                            CHROME    BEFORE    AFTER
//!   viewBox 100x25, in a 400px block         400x100   100x25    400x100
//!   viewBox 100x25, in a 250px block         250x63    100x25    250x63
//!   the <path> filling that viewBox          400x100   100x25    400x100
//!   no viewBox at all                        300x150   100x100   300x150
//!   viewBox + height="10"                     40x10    100x10     40x10
//!   ── the other half, pre-existing ─────────────────────────────────────
//!   an unsized <canvas> as a FLEX ITEM       300x150     0x150    300x150
//!   a ratio'd <svg> as a FLEX ITEM           540x540    16x16*    540x540
//! ```
//!
//! ⚠⚠ **The starred row is why the flex half had to land in the same tick.** A replaced element has
//! no children, so the flex/grid measure seam — which sizes an item by laying its subtree out and
//! reading how far the content reached — measured **zero**, and an unsized `<canvas>` flex item was
//! 0px wide before any of this. The svg column of that seam was hidden behind the very bug being
//! fixed: reading the viewBox as pixels gave a 16-unit icon a 16×16 box, which *looks* like an icon,
//! so replacing the accident with the spec answer alone would have made the nav-bar case **worse**.
//! Measured, not assumed — it showed up as four extra reading-order violations on `www.ikea.com`,
//! `<span>` ⇄ `<svg>` inside a flex `<a>`.
//!
//! ⚠⚠ **AND THOSE FOUR ARE STILL THERE — the flex half made the fixture Chrome-exact and did NOT
//! clear them.** `www.ikea.com` reads 19 → 23 out-of-sequence sibling pairs, reproducible twice on
//! each side, against `shape` 51.43 → 51.72 and coverage 97.08 → 100.0 on the same tree. A first
//! draft of this comment claimed the count came back; it was written before the re-measurement and
//! it was wrong. The cost is real, it is named, and it is the top item on the follow-on list — the
//! remaining flex/inline placement of a ratio'd replaced element is not what this gate proves.
//!
//! ⚠⚠ **The size came in through a second channel, behind a comment that said it would not.** The
//! inline-svg raster cache is merged into `Page::images` so the painter can find it, and
//! `apply_natural_sizes` reads that same map — so usvg's `Tree::size()`, which falls back to the
//! viewBox when the dimension attributes are absent, arrived as an intrinsic size. The code that
//! merges the cache says in as many words *"Inline svgs are deliberately NOT natural-sized: the
//! measured replaced-sizing model owns their geometry"* — and it was true of the function it was
//! written next to and false of the map.
//!
//! ⚠⚠ **The block path had this right and a unit test proved it — under `MinimalCascade`.**
//! `an_unsized_svg_gets_the_default_object_size` (engine/layout) asserts exactly the 400px case and
//! passes, because the unit-test cascade never runs the natural-size pass at all. The shipping
//! Stylo path did. That is the two-cascades trap: **a green unit test is evidence about the cascade
//! it ran under**, so this gate runs the real one.
//!
//! ⚠⚠ **This gate reads the FINAL LAYOUT RECTS, not `getBoundingClientRect` from a `load`
//! handler — and that is not a stylistic choice.** The first draft asked the page itself, and it
//! passed with the fix REVERTED: the leak is in the post-load subresource pass, so at load-event
//! time the boxes are already right and go wrong afterwards. A gate that measures before the bug
//! happens is a gate that cannot fail. Found by RED-proving, twice.
//!
//! ⚠ **NAMED, MEASURED, NOT FIXED:** a `<use href="#sym">` still reports no geometry (Chrome
//! 20×20) and `<symbol>`/`<defs>` content still gets a box it should not — both are the SVG
//! *referencing* model, and both are their own change.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body style="margin:0">
 <div style="width:400px"><svg id="a" viewBox="0 0 100 25"><path id="ap" d="M0 0 H100 V25 H0 Z"/></svg></div>
 <div style="width:250px"><svg id="b" viewBox="0 0 100 25"></svg></div>
 <div style="width:400px"><svg id="c"></svg></div>
 <div style="width:400px"><svg id="d" viewBox="0 0 100 25" height="10"></svg></div>
 <div style="width:400px"><svg id="e" viewBox="0 0 100 25" style="width:60px"></svg></div>
 <div style="width:400px;display:flex"><canvas id="fc"></canvas></div>
 <div style="width:600px;display:flex;align-items:center"><span id="fs">Products</span><svg id="fi" viewBox="0 0 16 16"><path d="M2 5 L8 11 L14 5 Z"/></svg></div>
</body></html>"#;

/// `id=WxH` for each id, read from the layout tree the painter and the fidelity sweep both read.
fn sizes(page: &manuk_page::Page, ids: &[&str]) -> String {
    let rects = page.root_box.node_rects(page.dom());
    let root = page.dom().root();
    ids.iter()
        .map(|id| {
            let hits = manuk_css::query_selector_all(page.dom(), root, &format!("#{id}"));
            match hits.first().and_then(|n| rects.get(n)) {
                Some(r) => format!("{id}={}x{}", r.width.round(), r.height.round()),
                None => format!("{id}=NO-BOX"),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn an_auto_sized_svg_fills_its_container_and_takes_its_height_from_the_viewbox() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    // ⚠ `finish_loading` is not optional here: the inline-svg raster cache — the map the bug came
    // through — is merged into `Page::images` by the SUBRESOURCE pass, which is also why the
    // assertion reads the final rects rather than asking the page during `load`.
    let page = rt.block_on(async {
        let mut p =
            manuk_page::Page::load_async(HTML, "https://svgsize.test/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let got = sizes(&page, &["a", "ap", "b", "c", "d", "e", "fc", "fs", "fi"]);
    println!("SVG-AUTO-SIZING {got}");
    let has = |s: &str| got.contains(s);

    // (1) **THE MUTATION THAT MATTERS: two different containers.** RED: let the inline-svg raster
    // back into `apply_natural_sizes` → BOTH read `100x25`, the viewBox's own numbers. A single
    // container could be satisfied by any constant that happened to match; two cannot, and 400 vs
    // 250 is the assertion that the width comes from the CONTAINING BLOCK.
    assert!(
        has("a=400x100") && has("b=250x63"),
        "an <svg viewBox> with no width/height must FILL its containing block and take its height \
         from the ratio — 400x100 in a 400px block and 250x63 in a 250px one, Chrome-measured. \
         Reading the viewBox as an intrinsic SIZE gives 100x25 in both — got {got:?}"
    );

    // (2) **The child inherits the scale**, which is why this is a fidelity fix and not a trivia
    // fix: a `<path>` filling that viewBox is 400x100 too, and at 100x25 every path, `<g>`, `<rect>`
    // and `<circle>` in the tree is measured against a canvas four times too small.
    assert!(
        has("ap=400x100"),
        "a <path> filling the viewBox must scale with the svg's used size — got {got:?}"
    );

    // (3) **THE CONTROL AGAINST OVER-CORRECTION.** An svg with NO viewBox has no ratio and no
    // intrinsic size, so it is the default object size — 300x150, NOT the container width. A fix
    // that simply made every auto svg fill its container satisfies (1) and (2) and fails here.
    assert!(
        has("c=300x150"),
        "an <svg> with no viewBox has no intrinsic ratio and must fall back to the DEFAULT OBJECT \
         SIZE 300x150, not to the container width — got {got:?}"
    );

    // (4) **The ratio runs the other way too.** A definite height derives the width through the
    // ratio (CSS2 §10.3.2), and an author CSS width still wins outright.
    assert!(
        has("d=40x10") && has("e=60x15"),
        "height=10 with a 4:1 viewBox is 40x10, and an author width:60px is 60x15 — got {got:?}"
    );

    // (5) **THE OTHER HALF, and it was a MISSING BOX.** A replaced element has no children, so the
    // flex/grid measure seam — which sizes an item by laying its subtree out and reading how far
    // the content reached — measured ZERO and an unsized `<canvas>` flex item came out 0px wide.
    // RED: drop `replaced_default_size` from `measure_intrinsic` → `fc=0x150`. This is separately
    // reachable from (1): it was broken for `<canvas>` and `<video>` before any svg change.
    assert!(
        has("fc=300x150"),
        "an unsized <canvas> as a FLEX ITEM must be 300x150 — a replaced element has no children, \
         so measuring its content extent reports a zero WIDTH and the box vanishes — got {got:?}"
    );

    // (6) **THE NAV-BAR SHAPE, and it is the one that cost real fidelity.** A flex row with a label
    // and an icon: Chrome gives the icon the space the label leaves (600 - 56 = 544) and the label
    // keeps its own 56px. RED: hand the max-content probe the default object WIDTH instead of the
    // available one → `fi=300x300`, and the label is displaced. That displacement is exactly the
    // `<span>` ⇄ `<svg>` shape this cost four reading-order pairs on www.ikea.com — a cost that
    // this half made the FIXTURE exact without clearing on the live site. See the module header.
    //
    // ⚠ 544, not 540: the first draft asserted a number measured on a *nearly* identical fixture
    // that carried a `gap:4px`. Chrome was re-run on THIS markup rather than the expectation
    // adjusted — a gate whose number came from a different fixture is a gate testing a memory.
    assert!(
        has("fi=544x544") && has("fs=56x18"),
        "a ratio'd <svg> as a flex item takes the space its siblings leave — 544x544 beside a 56px \
         label in a 600px row, Chrome-measured — got {got:?}"
    );
}
