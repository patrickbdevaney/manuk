//! **G_SCROLL_SNAP_HORIZONTAL — a real horizontal carousel scrolls, and snaps, on the x axis.**
//!
//! The t266 scroll-snap tick gated the VERTICAL axis and recorded an honest residue: "an
//! inline-block row yields NO horizontal scroll range in layout today (max_x = 0) — real
//! horizontal carousels do not yet scroll at all, let alone snap." Interop 2026 then named
//! scroll snap a focus area (surface audit #14), which raised the residue's price. The tick-408
//! re-probe found the residue STALE-PESSIMISTIC — the standing rule's sixth confirmed instance:
//! both modern carousel shapes (`white-space:nowrap` + inline-blocks, and `display:flex` +
//! `overflow-x:auto`) now report full horizontal geometry (scrollWidth 500 / clientWidth 200)
//! and accept `scrollLeft`. Layout work since t266 (replaced sizing, inline-block extents)
//! closed the gap as a side effect; nothing was ever pinned, so the map kept saying "broken".
//!
//! This gate pins the capability the residue said was impossible:
//!   - horizontal geometry on BOTH carousel shapes (scrollWidth/clientWidth truthful),
//!   - `scrollLeft` writes clamp to `scrollWidth - clientWidth` (the last-slide-reachable rule),
//!   - `scroll-snap-type: x mandatory` LANDS a mid-slide scroll on the nearest snap point —
//!     the same single-chokepoint snap the vertical gate proves, on the axis it never covered.
//!
//! RED, demonstrated at authoring time: asserting `snap:100` with the snap styles removed reads
//! `snap:130` (no snap applied), and asserting clamped `sl` with a 1e9 write reads the raw value
//! if clamping ever regresses.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="nowrap" style="width:200px; overflow-x:auto; white-space:nowrap">
  <div style="display:inline-block;width:100px;height:50px">1</div><div style="display:inline-block;width:100px;height:50px">2</div><div style="display:inline-block;width:100px;height:50px">3</div><div style="display:inline-block;width:100px;height:50px">4</div><div style="display:inline-block;width:100px;height:50px">5</div>
</div>
<div id="snapper" style="width:200px; overflow-x:auto; display:flex; scroll-snap-type: x mandatory">
  <div style="flex:0 0 100px;height:50px;scroll-snap-align:start">a</div><div style="flex:0 0 100px;height:50px;scroll-snap-align:start">b</div><div style="flex:0 0 100px;height:50px;scroll-snap-align:start">c</div><div style="flex:0 0 100px;height:50px;scroll-snap-align:start">d</div><div style="flex:0 0 100px;height:50px;scroll-snap-align:start">e</div>
</div>
<div id="out">-</div>
<script>
var r = [];
var n = document.getElementById('nowrap'), s = document.getElementById('snapper');
r.push('nowrap-sw:' + n.scrollWidth);
r.push('nowrap-cw:' + n.clientWidth);
n.scrollLeft = 1e9;
r.push('nowrap-clamp:' + n.scrollLeft);          // scrollWidth - clientWidth = 300
r.push('flex-sw:' + s.scrollWidth);
s.scrollLeft = 130;                               // mid-slide: nearest x-snap point is 100
r.push('snap:' + s.scrollLeft);
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn a_horizontal_carousel_scrolls_and_snaps() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://carousel.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SCROLL-SNAP-HORIZONTAL RESULT: {got}");

    for claim in [
        "nowrap-sw:500", // the inline-block row's true content width — the axis t266 said was 0
        "nowrap-cw:200", // the padding-box window
        "nowrap-clamp:300", // writes clamp to scrollWidth - clientWidth (last slide reachable)
        "flex-sw:500",   // the flex-row carousel shape reports the same truth
        "snap:100",      // x-mandatory snap LANDS a mid-slide scroll on the nearest point
    ] {
        assert!(
            got.contains(claim),
            "G_SCROLL_SNAP_HORIZONTAL: expected `{claim}`\n  got: {got}\n\n  \
             A horizontal carousel (nowrap inline-blocks, or flex + overflow-x) must report \
             truthful scrollWidth/clientWidth, clamp scrollLeft to the scrollable range, and \
             land x-mandatory scrolls on the nearest snap point."
        );
    }
}
