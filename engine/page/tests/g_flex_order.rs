//! # G_FLEX_ORDER — `order` lays items out in order-modified document order
//!
//! Flexbox §5.4 and Grid §6.3: items are laid out in **order-modified document order**, not document
//! order. `order: -1` to pull the image above the copy, `order: 2` to send the sidebar after the
//! article — this is how a responsive layout rearranges blocks without touching the markup, and it is
//! in essentially every design system's breakpoint CSS.
//!
//! taffy has no `order` field, so the sort has to happen where the items are collected. We did not do
//! it, so every such layout rendered its blocks in source sequence. **That is a READING-ORDER defect**
//! — the jarring dimension this corpus is worst at (14.5% of in-scope sites clean at t786) — rather
//! than a missing property that degrades quietly: every pairwise comparison against Chrome across the
//! reordered items disagrees at once.
//!
//! ## Measured (`--dump-dom` + `getBoundingClientRect`), x positions in a 400px row of 100px items
//!
//! ```text
//!   n1 n2 n3   second item has order:-1        100    0  200
//!   t1 t2 t3   middle has an EXPLICIT order:0    0  100  200   ← ties keep document order
//!   m1 m2 m3   order 3 / 1 / 2                 200    0  100
//!   ga gb gc   the same, in a GRID              100    0  200
//! ```
//!
//! ## The tie is the whole specification of the sort
//!
//! Equal `order` — which is every item on most pages, since the initial value is `0` — must keep
//! DOCUMENT order. An unstable sort would shuffle ordinary flex rows for no reason, on every page,
//! which is a far worse bug than the one being fixed. `t1/t2/t3` exists to catch exactly that, and it
//! is the case a fixture written only from the failing symptom would not contain.
//!
//! ## What must NOT move
//!
//! `order` is **visual only**, by design: the DOM, the accessibility tree and sequential focus keep
//! source order. Reordering those here would turn a layout fix into an accessibility regression, so
//! this gate asserts the DOM's own order is untouched alongside the boxes'.
//!
//! ## How this goes RED
//!
//! - **Drop the sort** → `n1`, `n2`, `m1`, `m2`, `m3`, `ga`, `gb` all fail at once.
//! - **Make the sort unstable** (e.g. sort by `order` descending then reverse) → `t1/t2/t3` fail
//!   while every explicit-order case still passes.
//! - **Sort the DOM children instead of the layout items** → the boxes pass and `dom_order` fails.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.2 sans-serif}
.f{display:flex;width:400px}
.f > div{width:100px;height:10px}
#n2{order:-1}
#t2{order:0}
#g{display:grid;grid-template-columns:repeat(4,100px);width:400px}
#g > div{height:10px}
#gb{order:-1}
</style></head><body>
<div class="f"><div id="n1"></div><div id="n2"></div><div id="n3"></div></div>
<div class="f"><div id="t1"></div><div id="t2"></div><div id="t3"></div></div>
<div class="f"><div id="m1" style="order:3"></div><div id="m2" style="order:1"></div><div id="m3" style="order:2"></div></div>
<div id="g"><div id="ga"></div><div id="gb"></div><div id="gc"></div></div>
</body></html>"##;

fn x_of(page: &manuk_page::Page, sel: &str) -> f32 {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    page.root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel}"))
        .x
}

fn assert_x(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let got = x_of(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_FLEX_ORDER: `{sel}` expected x={want} (MEASURED in Chrome), got {got}.\n  {why}"
    );
}

#[test]
fn g_flex_order() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://order.test/", &fonts, 1200.0);

    // ── A negative order pulls an item to the front.
    assert_x(&page, "#n2", 0.0, "`order:-1` is laid out FIRST");
    assert_x(
        &page,
        "#n1",
        100.0,
        "…so the source-first item moves to the second slot",
    );
    assert_x(&page, "#n3", 200.0, "…and the third is unmoved");

    // ── THE TIE. Every item here has order 0 (one of them says so explicitly), so document order
    // must survive. This is the case that separates a stable sort from a shuffle.
    assert_x(
        &page,
        "#t1",
        0.0,
        "equal `order` keeps DOCUMENT order — an unstable sort shuffles these",
    );
    assert_x(
        &page,
        "#t2",
        100.0,
        "…including an item that writes the initial value out explicitly",
    );
    assert_x(&page, "#t3", 200.0, "…and the last stays last");

    // ── Several distinct orders, none of them 0.
    assert_x(&page, "#m2", 0.0, "order 1 of (3, 1, 2)");
    assert_x(&page, "#m3", 100.0, "order 2");
    assert_x(
        &page,
        "#m1",
        200.0,
        "order 3 — the source-first item lands last",
    );

    // ── GRID takes the same rule, through the same item collection.
    assert_x(
        &page,
        "#gb",
        0.0,
        "`order:-1` in a GRID container, not just a flex one",
    );
    assert_x(
        &page,
        "#ga",
        100.0,
        "…and its source-first sibling shifts right",
    );
    assert_x(&page, "#gc", 200.0, "…third unmoved");

    // ── AND THE DOM IS UNTOUCHED. `order` is visual only: the a11y tree and sequential focus read
    // source order, and an engine that reordered the tree would pass every assertion above while
    // silently rewriting what a screen reader announces.
    let dom = page.dom();
    let container = manuk_css::query_selector_all(dom, dom.root(), ".f")[0];
    let ids: Vec<String> = dom
        .children(container)
        .filter(|&c| dom.is_element(c))
        .filter_map(|c| {
            dom.element(c)
                .and_then(|e| e.attr("id"))
                .map(str::to_string)
        })
        .collect();
    assert_eq!(
        ids,
        vec!["n1", "n2", "n3"],
        "G_FLEX_ORDER: `order` must not touch the DOM — the accessibility tree and tab order read \
         SOURCE order, and reordering the tree would pass every box assertion above while rewriting \
         what a screen reader announces"
    );
}
