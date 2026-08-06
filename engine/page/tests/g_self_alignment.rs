//! # G_SELF_ALIGNMENT — `align-self` reached taffy and `justify-self` did not
//!
//! A grid item asking for `justify-self: end` sat at the **start** of its track: **x=0 where Chrome
//! puts it at 140** in a 200px column. `align-self` — the same rule one axis over — was mapped
//! correctly the whole time, which is what made the pair look handled.
//!
//! ```text
//!                                                    Chrome        before        after
//!     align-self:center on a flex item             [  0,  36]   [  0,  36]   unchanged
//!     align-self:flex-end                          [  0, 152]   [  0, 152]   unchanged
//!     align-self:flex-start (container centres)    [  0, 178]   [  0, 178]   unchanged
//!     align-self:stretch (container centres)    [0,264,60,80]      same      unchanged
//!     align-self:end in a GRID                     [  0, 456]   [  0, 456]   unchanged
//!     justify-self:end in a GRID                   [140, 350]   [  0, 350]   [140, 350]
//! ```
//!
//! ## Where it lived
//!
//! `taffy_tree.rs` builds taffy's `Style` from our `ComputedStyle`, and the line below it had no
//! twin:
//!
//! ```rust
//!    align_items: Some(map_align(cs.align_items)),
//!    align_self:  cs.align_self.map(map_align),
//!    justify_content: map_justify(cs.justify_content),
//!    // ...and no `justify_self` at all
//! ```
//!
//! There was no `justify_self` on `ComputedStyle` either, so nothing parsed it and nothing mapped it
//! from Stylo. **The property was absent at all three layers while its own axis-twin was complete at
//! all three** — the kind of gap that survives because the neighbouring line looks like coverage.
//!
//! Found by surface audit #38: `justify-self` was the one item on the Safari 26.x list with a
//! non-zero corpus price (**1.8%**) that a probe showed we got wrong, and `align-self` at **8.2%**
//! was the same probe's control — it passed 5/5 and is asserted here so a future edit to the shared
//! `map_align` cannot move it silently.
//!
//! ## How this goes RED
//!
//! - **Drop the `justify_self` line from `taffy_tree.rs`** → `#a5` reads x=0 against Chrome's 140.
//!   The original defect, and the only row that moves.
//! - **Map `justify_self` from `align_self`** (a plausible copy-paste) → `#a5` still fails, because
//!   the fixture's grid row sets no `align-self` — the two axes are asserted on rows that do not
//!   share a value for exactly this reason.
//! - **Give `justify-self: auto` a value instead of `None`** → `#a6`'s grid item stops honouring the
//!   container and the default-alignment rows move.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font-family:sans-serif;font-size:16px}
div{margin:6px 0}
i{display:block;font-style:normal}
</style></head><body>
<div id="wa1" style="display:flex;height:80px"><i id="a1" style="width:60px;height:20px;align-self:center">a</i><i style="width:60px;height:60px">b</i></div>
<div id="wa2" style="display:flex;height:80px"><i id="a2" style="width:60px;height:20px;align-self:flex-end">a</i><i style="width:60px;height:60px">b</i></div>
<div id="wa3" style="display:flex;height:80px;align-items:center"><i id="a3" style="width:60px;height:20px;align-self:flex-start">a</i><i style="width:60px;height:60px">b</i></div>
<div id="wa4" style="display:flex;height:80px;align-items:center"><i id="a4" style="width:60px;align-self:stretch">a</i><i style="width:60px;height:60px">b</i></div>
<div id="wa5" style="display:grid;grid-template-columns:200px;height:40px"><i id="a5" style="width:60px;justify-self:end">a</i></div>
<div id="wa6" style="display:grid;grid-template-columns:200px;height:80px"><i id="a6" style="width:60px;height:20px;align-self:end">a</i></div>
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
fn g_self_alignment() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://sa.test/", &fonts, 1200.0);
    let r = |sel: &str| rect_of(&page, sel);
    // Every row is its own container, so each assertion is stated as the item's offset from that
    // container — a row above going wrong cannot shift a row below into or out of agreement.
    let dy = |sel: &str, w: &str| r(sel).y - r(w).y;
    let dx = |sel: &str, w: &str| r(sel).x - r(w).x;

    // ── THE DEFECT: the inline axis, in a grid.
    assert!(
        (dx("#a5", "#wa5") - 140.0).abs() < 1.1,
        "G_SELF_ALIGNMENT: `justify-self: end` puts a 60px item at {} inside a 200px grid track, and \
         Chrome puts it at 140. Reading 0 means the property never reached taffy — there was no \
         `justify_self` on ComputedStyle, nothing parsed it, and `taffy_tree.rs` had `align_self` \
         with no twin.",
        dx("#a5", "#wa5")
    );

    // ── THE CONTROL AXIS: `align-self` was correct before this change, in BOTH formatting contexts,
    //    and it shares `map_align` with the new line — so an edit there must not move it.
    assert!(
        (dy("#a1", "#wa1") - 30.0).abs() < 1.1,
        "G_SELF_ALIGNMENT: `align-self:center` on a 20px item in an 80px flex row must sit 30px \
         down, not {}.",
        dy("#a1", "#wa1")
    );
    assert!(
        (dy("#a2", "#wa2") - 60.0).abs() < 1.1,
        "G_SELF_ALIGNMENT: `align-self:flex-end` must sit 60px down, not {}.",
        dy("#a2", "#wa2")
    );
    assert!(
        (dy("#a3", "#wa3") - 0.0).abs() < 1.1,
        "G_SELF_ALIGNMENT: `align-self:flex-start` must OVERRIDE the container's \
         `align-items:center` and sit at 0, not {} — this row is the one that proves the ITEM's \
         value wins over the container's.",
        dy("#a3", "#wa3")
    );
    assert!(
        (r("#a4").height - 80.0).abs() < 1.1,
        "G_SELF_ALIGNMENT: `align-self:stretch` must fill the 80px row (height {}), overriding the \
         container's `align-items:center`.",
        r("#a4").height
    );
    assert!(
        (dy("#a6", "#wa6") - 60.0).abs() < 1.1,
        "G_SELF_ALIGNMENT: `align-self:end` in a GRID must sit 60px down, not {} — `align-self` was \
         already correct in grid as well as flex, which is exactly why the missing `justify-self` \
         went unnoticed.",
        dy("#a6", "#wa6")
    );
}
