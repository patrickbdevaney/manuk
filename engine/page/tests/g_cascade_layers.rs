//! # G_CASCADE_LAYERS — a layer exists to LOSE, and ours won
//!
//! `@layer` (Baseline 2022) is how a page keeps a framework's styles overridable: put the framework
//! in a layer, write your own rules unlayered, and **the unlayered rules win regardless of document
//! order.** That last clause is the entire feature — a layer whose rules beat the page's own rules is
//! worse than no layer at all, because the author wrote it specifically to avoid specificity wars.
//!
//! Surface audit #50 (t787) measured us flattening layers into document order:
//!
//! ```text
//!   #h { width: 100px }  ·  @layer L { #h { width: 333px } }     Chrome 100   ours 333
//! ```
//!
//! ## Every expectation here is MEASURED, not recalled
//!
//! `google-chrome --headless=new --dump-dom` on the same fixture this test loads:
//!
//! ```text
//!   a=300   b=300   c=100   d=210   e=100
//! ```
//!
//! and each letter is a different clause of CSS Cascade 5 §6.4.4:
//!
//! * **`a` — the STATEMENT form fixes the order before either block exists.** `@layer reset, theme;`
//!   then a `theme` block *followed by* a `reset` block: `theme` wins at 300 even though its block
//!   came first. An engine that ranked layers by first BLOCK gets this backwards and reads 111 — and
//!   this is the common idiom, written at the top of the sheet precisely so the blocks below can
//!   appear in any order.
//! * **`b` — a later layer beats an earlier one** (300, not 111).
//! * **`c` — UNLAYERED BEATS LAYERED, regardless of order** (100, not 333). The audit's finding.
//! * **`d` — a declaration that exists ONLY in a layer still applies** (210). The complement, and the
//!   half a fix aimed only at `c` would break: "layers lose" must not become "layers are ignored".
//! * **`e` — an anonymous `@layer { … }` is a layer too** (100, not 250).
//!
//! ## How this goes RED
//!
//! - **Drop the layer term from the winner sort** → `c` and `e` read 333 and 250: the pre-t790 bug.
//! - **Drop the `CssRule::LayerStatement` arm** → `a` reads 111, and only `a`.
//! - **Give every layered rule rank 0 instead of counting up** → `b` reads 111.
//! - **Skip layered rules entirely** ("layers lose" read as "layers are ignored") → `d` reads 100.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
@layer reset, theme;
@layer theme { #a { width: 300px } }
@layer reset { #a { width: 111px } }
@layer one { #b { width: 111px } }
@layer two { #b { width: 300px } }
#c { width: 100px }
@layer L { #c { width: 333px } }
@layer M { #d { width: 210px } }
#e { width: 100px }
@layer { #e { width: 250px } }
div { height: 20px }
</style></head><body>
<div id="a"></div><div id="b"></div><div id="c"></div><div id="d"></div><div id="e"></div>
</body></html>"##;

fn width_of(page: &manuk_page::Page, sel: &str) -> f32 {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    page.root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel}"))
        .width
}

fn assert_w(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let got = width_of(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_CASCADE_LAYERS: `{sel}` expected {want}px (MEASURED in Chrome), got {got}.\n  {why}"
    );
}

#[test]
fn g_cascade_layers() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://layers.test/", &fonts, 1200.0);

    assert_w(
        &page,
        "#a",
        300.0,
        "`@layer reset, theme;` fixes the ORDER before either block exists, so `theme` wins even \
         though the `reset` block is written after it. Ranking layers by first BLOCK reads 111 here \
         — and the statement form at the top of a sheet is the idiom this feature is used through",
    );
    assert_w(
        &page,
        "#b",
        300.0,
        "a later layer beats an earlier one — `two` over `one`",
    );
    assert_w(
        &page,
        "#c",
        100.0,
        "UNLAYERED BEATS LAYERED regardless of document order. This is the whole point of a layer: \
         the author moved those rules into it so their own would win without a specificity war",
    );
    assert_w(
        &page,
        "#d",
        210.0,
        "…and a declaration that exists ONLY in a layer still APPLIES. `layers lose` must not \
         become `layers are ignored` — that is the half a fix aimed only at #c would break",
    );
    assert_w(
        &page,
        "#e",
        100.0,
        "an anonymous `@layer { … }` is a layer too, and loses to the unlayered rule above it",
    );
}
