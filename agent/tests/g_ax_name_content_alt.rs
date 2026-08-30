//! **G_AX_NAME_CONTENT_ALT — the half of `content` that is ANNOUNCED reaches the accessible name.**
//!
//! t1369 made the two halves of `content: "drawn" / "announced"` *separable* — it stopped the
//! announced half being painted, on both cascades. This is the other end of that: the announced half
//! now **is** the name.
//!
//! ```text
//!   accname   432/484 (89.3%)  →  438/484 (90.5%)   +6, and ZERO newly failing
//!     button/heading/link name from fallback content with ::before and ::after            ×3
//!     button/heading/link name from fallback content mixing attr() and strings …          ×3
//! ```
//!
//! ## ⭐ `Some("")` is a real answer, and that is the whole design
//!
//! `content: "★" / ""` means *draw a star, announce nothing*. So the choice is three-way, not
//! `unwrap_or(rendered)` on a string:
//!
//! ```text
//!   no `/` in the declaration   ->  the name falls back to the RENDERED text
//!   `/ "alt"`                   ->  the name is "alt"
//!   `/ ""`                      ->  the name is EMPTY — and must NOT fall back
//! ```
//!
//! Collapsing the last two — by storing only non-empty alt strings, say — silently turns every
//! "decorative, do not announce" pseudo back into an announced one, which is the exact request the
//! author wrote the empty alt to make. That is mutation N2.
//!
//! ## ⭐⭐⭐ AND THE FOURTH FACT BECAME A CONTEXT STRUCT, BECAUSE t1365 SAID IT WOULD
//!
//! Three facts had been threaded through this walk one parameter at a time — t1097's
//! `GeneratedText`, t1355's `NameIndex` widening, t1365's `NameStyles` — and **each one left a
//! caller behind**, twice in the same unit test, invisibly, because `manuk-a11y` is a suite in no
//! wall (surface audit #78). t1365's own note read: *"a fourth fact should become a context struct
//! rather than a fourth parameter."* This is the fourth fact, and it did.
//!
//! `NameCtx { generated, alt, styles }` replaces two parameters across eleven signatures. The win is
//! not tidiness: **adding a fifth is now a one-line change to the struct and its two construction
//! sites**, instead of an edit to twenty call sites where missing one compiles fine on every path
//! but the one that matters.
//!
//! ## ⚠⚠ BOTH ENTRANCES, ASSERTED AGAINST EACH OTHER
//!
//! Same structure as t1365's gate and for the same reason: the walk is reached through the AX tree
//! builder (what a live agent reads) and the bare `accessible_name` behind
//! `test_driver.get_computed_label()` (what the conformance suite reads). Every row asserts that
//! **the two agree and that the agreed value is Chrome's**, so a fix wired to one door fails here
//! even though its own suite number moves by the full +6.
//!
//! ## ⚠ NAMED, MEASURED, NOT BUILT — `attr()` in `content` on the OTHER cascade
//!
//! `#b4` in the fixture is `content: "x " / "start " attr(data-alt) " end "`. Chrome names it
//! `"start MID end label"`; the **shipping (Stylo) path agrees** — which is exactly why the three
//! `name from fallback content mixing attr() and strings` rows are among the six this tick fixed.
//! `MinimalCascade` gives `"start end label"`, dropping the `attr()` term.
//!
//! The cause is structural rather than an oversight: `ContentPart` has no `Attr` variant **by
//! design** — its own doc says an `attr()` is *"already resolved against the element (that one CAN
//! be resolved in the cascade — the attribute is right there on the element)"* — and Stylo's mapper
//! does resolve it, while `parse_content_parts` is a free function with no element in hand. Closing
//! it means either threading the element into that parser or giving `ContentPart` a term layout
//! resolves, which is a design decision and not a line change.
//!
//! It is therefore **left in the fixture and out of the asserted set**: asserting Chrome's answer
//! would land a RED gate, and asserting MinimalCascade's would pin the bug (the t1004 shape). The
//! day it is fixed, `("b4", "start MID end label", …)` joins the rows above. `attr(` prices at
//! **14 of 39** corpus sites — the highest row in t1369's pref sweep — so this is a ranked item,
//! not a curiosity.
//!
//! PROVEN RED by three mutations — see the module tail.

use manuk_a11y::{accessible_name_generated, empty_name_ctx, name_styles, role_of, A11yNode};
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  #b1::before{content:"drawn " / "announced ";}
  #b2::before{content:"drawn ";}
  #b3::before{content:"star " / "";}
  #b4::before{content:"x " / "start " attr(data-alt) " end ";}
</style></head><body>
  <button id="b1">label</button>
  <button id="b2">label</button>
  <button id="b3">label</button>
  <button id="b4" data-alt="MID">label</button>
</body></html>"##;

fn node_id(page: &manuk_page::Page, id: &str) -> manuk_dom::NodeId {
    let dom = page.dom();
    dom.get_element_by_id(dom.root(), id)
        .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"))
}

fn tree_name(tree: &A11yNode, want: manuk_dom::NodeId) -> String {
    tree.iter()
        .find(|n| n.node == want)
        .unwrap_or_else(|| panic!("VACUOUS: node {want:?} is not in the a11y tree"))
        .name
        .clone()
}

#[test]
fn the_announced_half_of_content_is_the_name_through_both_entrances() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://axca.test/", &fonts, 800.0);
    let dom = page.dom();
    let generated = manuk_layout::generated_text(dom, page.styles_map());
    let alt = manuk_layout::generated_alt_text(dom, page.styles_map());
    let styles = name_styles(dom, page.styles_map());
    let ctx = empty_name_ctx(&generated, &alt, &styles);
    let tree = page.a11y_tree();

    // ── VACUITY. The generated text must actually be reaching the walk, or every row below is a
    // comparison of four bare "label"s — and the ALT map must be non-empty, or `#b1` would pass by
    // accident on any implementation that simply dropped generated content.
    assert!(
        !generated.is_empty(),
        "VACUOUS: no ::before text was materialised at all"
    );
    assert!(
        !alt.is_empty(),
        "VACUOUS: the alt map is empty, so nothing below tests the alt half"
    );
    // `#b4` is not asserted (see the header) but it must still be REACHED, or the note about it is
    // about a fixture row that does not exist.
    {
        let n4 = node_id(&page, "b4");
        let name4 = tree_name(&tree, n4);
        assert!(
            name4.contains("label"),
            "VACUOUS: #b4 produced no name at all ({name4:?}), so the NAMED-MEASURED-NOT-BUILT note \
             in this module's header is not about anything"
        );
    }

    // (id, the name Chrome computes, what the row is for)
    let rows: &[(&str, &str, &str)] = &[
        (
            "b1",
            "announced label",
            "the ALT half is the name, and the drawn half is not",
        ),
        (
            "b2",
            "drawn label",
            "CONTROL — no `/`, so the name falls back to the RENDERED text",
        ),
        (
            "b3",
            "label",
            "an EMPTY alt announces NOTHING and must not fall back to `star `",
        ),
        // ⚠ `#b4` — `attr()` INSIDE the alt half — is measured below rather than asserted here, and
        // the reason is worth stating: it works on the SHIPPING path and not on this one.
    ];

    for (id, want, why) in rows {
        let n = node_id(&page, id);
        let role = role_of(dom, n).unwrap_or_else(|| panic!("VACUOUS: #{id} maps to no ARIA role"));
        let bare = accessible_name_generated(dom, n, &role, &ctx);
        let via_tree = tree_name(&tree, n);
        assert_eq!(
            bare, *want,
            "G_AX_NAME_CONTENT_ALT #{id} (bare name): Chrome computes {want:?}, got {bare:?}.\n  {why}"
        );
        assert_eq!(
            via_tree, *want,
            "G_AX_NAME_CONTENT_ALT #{id} (AX TREE): Chrome computes {want:?}, got {via_tree:?}. The \
             bare name is {bare:?} — if those differ, the fix reached ONE of the two entrances, \
             which is the t1097/t1350/t1355 shape these gates exist to refuse.\n  {why}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  `NameCtx::pseudo_names` returns the rendered text always (the pre-tick behaviour)
//       -> #b1 reads "drawn label", #b3 reads "star label", #b4 reads "x label"; the CONTROL row
//          #b2 stays green, which is what says this is about the ALT half and not about generated
//          content in general.
// N2  treat `Some("")` as absent — `alt.filter(|s| !s.is_empty()).unwrap_or(rendered)`
//       -> only #b3 fails, reading "star label". This is the row that makes the three-way choice
//          load-bearing: an empty alt is a REQUEST, not a missing value.
// N3  build the tree with an empty `GeneratedAlt` while the bare entrance gets the real one
//       -> every `bare` assertion passes and every `AX TREE` assertion fails. The two-door
//          structure, load-bearing rather than decorative.
