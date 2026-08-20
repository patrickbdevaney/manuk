//! **G_CASCADE_AMPLIFICATION — a script that appends 2,000 nodes must not cost 2,000 cascades.**
//!
//! `CONSTITUTION.MD` PART VI.2's H0.1 row calls incremental relayout *"the single highest-leverage
//! architectural decision in the renderer"* and states its mechanism as a RATE:
//!
//! > *"without incrementality every DOM mutation is O(document), so any page that builds content in
//! > a loop — every SPA, every feed, every table render — pays `mutations × nodes`."*
//!
//! ⭐ **t1324 measured that rate on the real web for the first time, and it is not 1.0 — it is
//! between 0.0005 and 0.0052.**
//!
//! ```text
//!    site                    LAYOUTS  CASCADES     MUT   NODES   CASC/MUT
//!    www.fragrantica.com          20        20   36431   14663     0.0005
//!    oilprice.com                 45        45    9913    3765     0.0045
//!    ticket.jfa.jp                19        19    3678    2069     0.0052
//!    en.wikipedia.org/…           14        14    5919    2536     0.0024
//!    news.ycombinator.com          8         6    2450    1293     0.0024
//! ```
//!
//! `fragrantica.com` performs **36,431 DOM mutations and pays 20 layouts.** The coalescing is
//! already there — mutations do not each drive a pass — so the sentence above is true of the
//! FORCED-REFLOW shape (a script that reads `getComputedStyle`/`offsetHeight` between writes, which
//! is what `css/selectors/invalidation/has-complexity.html` is) and **not** of "any page that builds
//! content in a loop".
//!
//! This gate exists to keep it that way, because the property is currently emergent rather than
//! designed and nothing was asserting it. **To watch it go RED:** make `Page` re-cascade or
//! re-lay-out once per DOM mutation.
//!
//! ⚠ **And the clock half alone would reward the LEAK.** "Don't cascade" is trivially satisfied by
//! never cascading, which leaves the appended nodes unstyled — a faster number and a blank page. So
//! the second half asserts the 2,000th appended element actually has its authored colour: the
//! cascade must have happened, once, and covered everything.

use manuk_text::FontContext;

/// Appends 2,000 elements in a tight loop with **no interleaved reads** — the shape the
/// constitution's sentence describes. A forced-reflow shape is a different gate.
const N: usize = 2000;

#[test]
fn appending_two_thousand_nodes_does_not_cost_two_thousand_cascades() {
    use std::sync::atomic::Ordering;

    let html = format!(
        r#"<!doctype html><html><head><style>
          .item {{ color: rgb(1, 2, 3) }}
        </style></head><body>
          <div id="host"></div>
          <div id="out">-</div>
          <script>
            var h = document.getElementById('host');
            for (var i = 0; i < {N}; i++) {{
              var d = document.createElement('div');
              d.className = 'item';
              d.textContent = 'row ' + i;
              h.appendChild(d);
            }}
            globalThis.__report = function () {{
              var last = h.lastElementChild;
              document.getElementById('out').textContent =
                'kids:' + h.children.length +
                ' color:' + (last ? getComputedStyle(last).color : 'NONE');
            }};
          </script>
        </body></html>"#
    );

    let fonts = FontContext::new();
    #[cfg(feature = "stylo")]
    manuk_css::stylo_engine::CASCADES.store(0, Ordering::Relaxed);
    manuk_layout::LAYOUTS.store(0, Ordering::Relaxed);

    let mut page = manuk_page::Page::load(&html, "https://amp.test/", &fonts, 800.0);
    page.eval_for_test("globalThis.__report && __report()");

    let layouts = manuk_layout::LAYOUTS.load(Ordering::Relaxed);
    #[cfg(feature = "stylo")]
    let cascades = manuk_css::stylo_engine::CASCADES.load(Ordering::Relaxed);
    #[cfg(not(feature = "stylo"))]
    let cascades = 0usize;

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("  amplification: {N} appends → CASCADES {cascades} · LAYOUTS {layouts} · {got}");

    // ── The correctness half FIRST, so a gate that measured nothing cannot pass on speed.
    assert!(
        got.contains(&format!("kids:{N}")),
        "G_CASCADE_AMPLIFICATION: the loop did not build the tree — the cascade count below would \
         be a number about nothing.\n  got: {got}"
    );
    assert!(
        got.contains("color:rgb(1, 2, 3)"),
        "G_CASCADE_AMPLIFICATION: the {N}th appended element does not have its authored colour, so \
         the cascade never covered it. A low cascade count bought by NOT STYLING the new nodes is \
         the memory-leak trade in a different currency: a faster number and a blank page.\n  \
         got: {got}"
    );

    // ── …then the rate. The bar is two orders of magnitude below `N`, which is where the real web
    //    sits (`fragrantica` pays 20 for 36,431). Only per-mutation work can cross it.
    const BAR: usize = 25;
    assert!(
        cascades <= BAR && layouts <= BAR,
        "G_CASCADE_AMPLIFICATION: {N} appends cost {cascades} cascades and {layouts} layouts \
         against a bar of {BAR}. Something has started doing full-document work PER MUTATION — the \
         `mutations × nodes` shape CONSTITUTION.MD PART VI.2 names, which the real web measured at \
         0.0005–0.0052 cascades per mutation and which this gate exists to hold."
    );
}
