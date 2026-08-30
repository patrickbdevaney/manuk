//! **G_GENERATED_CONTENT_EDGE_SPACE — a space at the edge of a line is not drawn, and `content`'s
//! spaces were.**
//!
//! CSS Text 3 §4.1.3 removes a line's leading and trailing white space, and it does not care that
//! the space came from `content`. Ordinary text gets this for free here: it is split on white space
//! into words plus `PendingGap`s, so a space at a line edge is simply never drawn. **Generated
//! content took the other route** — t1107 emitted it as one unbreakable word with its spaces baked
//! into the string, so `content: " before "` carried its outer spaces as WIDTH and a line whose
//! first or last item was a pseudo came out one space too wide at each end.
//!
//! ```text
//!                                                        Chrome    before    after
//!   ::before " before " + "label" + ::after " after "     173.41   192.66   173.39
//!   ::before " before " + "label"                         115.61   125.23   115.59
//!   "label" + ::after " after "                           105.97   115.59   105.96
//!   ::before "before" + "label" + ::after "after"         154.14   154.12   154.12   CONTROL
//!   "label" with no pseudos at all                         48.17    48.16    48.16   CONTROL
//! ```
//!
//! Exactly one space per edge, every time.
//!
//! ## ⭐ The interior space is KEPT, and that is the whole difference
//!
//! Chrome's 12 characters for row 2 are `before` + **one space** + `label`: the space *between* the
//! pseudo and the text survives as the ordinary inter-word gap, and only the one at the line edge
//! goes. So the fix is not "trim generated content" — it is **hand the edge spaces to the gap
//! machinery**, which already knows what a line edge is. Trimming without re-emitting the gap
//! deletes the interior space too and reads 11 characters where Chrome reads 12.
//!
//! ⚠ t1107 baked these spaces in **deliberately** — *"the generated text is emitted as ONE
//! unbreakable word with its spaces baked in, because Chrome bills a trailing collapsible space
//! into the preceding inline's rect"* — and that reasoning still holds for the spaces **inside** the
//! string, which are untouched. `lead_ws`/`trail_ws` were already being read off the string for
//! their break opportunities; they now carry the space as well. The `.hlist` separator gate t1107
//! landed is green across this change and is the control that says so.
//!
//! PROVEN RED by two mutations — see the module tail.

use manuk_dom::NodeId;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font-family:monospace;font-size:16px;line-height:24px}
.w{width:400px;margin:0 0 8px 0}
#e1::before{content:" before ";} #e1::after{content:" after ";}
#e2::before{content:" before ";}
#e3::after{content:" after ";}
#e4::before{content:"before";} #e4::after{content:"after";}
</style></head><body>
<div class="w"><span id="e1">label</span></div>
<div class="w"><span id="e2">label</span></div>
<div class="w"><span id="e3">label</span></div>
<div class="w"><span id="e4">label</span></div>
<div class="w"><span id="e5">label</span></div>
</body></html>"##;

fn by_id(page: &manuk_page::Page, id: &str) -> NodeId {
    let dom = page.dom();
    dom.descendants(dom.root())
        .find(|&n| {
            dom.element(n)
                .and_then(|e| e.attr("id"))
                .is_some_and(|v| v == id)
        })
        .unwrap_or_else(|| panic!("VACUOUS: no element with id={id:?}"))
}

fn width(page: &manuk_page::Page, id: &str) -> f32 {
    page.root_box
        .node_rects(page.dom())
        .get(&by_id(page, id))
        .unwrap_or_else(|| panic!("VACUOUS: no box for id={id:?}"))
        .width
}

#[test]
fn a_pseudo_s_edge_space_is_a_gap_not_a_glyph() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ges.test/", &fonts, 1200.0);
    let near = |g: f32, w: f32| (g - w).abs() < 1.5;

    // ── VACUITY. The bare span must be the narrowest, or the pseudos are not reaching layout at
    // all and all five rows are the same measurement.
    let bare = width(&page, "e5");
    assert!(
        near(bare, 48.17),
        "VACUOUS: the span with no pseudos measures {bare}, not 48.17 — the fixture is not laying \
         out as expected and nothing below tests `content`"
    );
    assert!(
        width(&page, "e4") > bare + 90.0,
        "VACUOUS: the space-free pseudo row is not wider than the bare span, so `content` is not \
         being materialised and every row below would pass on the same number"
    );

    let rows: &[(&str, f32, &str)] = &[
        ("e1", 173.41, "both edges: 20 characters of text, 18 drawn — one space off each end"),
        ("e2", 115.61, "the LEADING edge only. 12 characters, not 13 — and not 11: the space BETWEEN the pseudo and the text survives as the inter-word gap"),
        ("e3", 105.97, "the TRAILING edge only, 11 characters not 12"),
        ("e4", 154.14, "CONTROL — pseudo content with no outer spaces is untouched"),
        ("e5", 48.17, "CONTROL — no pseudos at all"),
    ];
    for (id, want, why) in rows {
        let got = width(&page, id);
        assert!(
            near(got, *want),
            "G_GENERATED_CONTENT_EDGE_SPACE #{id}: Chrome renders this {want} wide, got {got}.\n  \
             {why}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  remove ONLY the trim, leaving the gaps in place
//       -> e1 reads 211.92 (22 characters): the edge spaces are back inside the word AND the gap
//          machinery still adds its own, so they are counted twice. ⚠ This is not the pre-tick
//          state — the pre-tick code had neither the trim nor the gaps — and the ledger says so
//          rather than quoting the pre-tick number for a mutation that did not produce it. What it
//          proves is the trim's necessity given the gaps.
// N2  trim the word but do NOT re-emit the space as a gap
//       -> e1 reads 154.125 (16 characters): the INTERIOR spaces are deleted along with the edge
//          ones, and the row collapses to the same width as the space-free control. This is the
//          mutation that makes "hand it to the gap machinery" load-bearing rather than a phrasing.
//
// ⚠ Neither mutation reproduces the pre-tick widths exactly, because the fix is a PAIR (trim +
// re-emit) and each mutation removes one half. The pre-tick numbers are in the table above, taken
// from the tree before the change rather than from a mutation of the tree after it.
