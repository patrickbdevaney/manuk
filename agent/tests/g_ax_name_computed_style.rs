//! **G_AX_NAME_COMPUTED_STYLE — the accessible name is a function of the COMPUTED STYLE, and the
//! walk had never been given one.**
//!
//! Check #128's STEER #2 was *"thread the COMPUTED STYLE into `accessible_name` the way t1097
//! threaded `GeneratedText` — one job closes block-level name spacing, `text-transform`, and
//! class-driven `display:none` (9 subtests)."* This is that job, and it is exactly nine:
//!
//! ```text
//!   accname   423/484 (87.4%)  →  432/484 (89.3%)   +9, and ZERO newly failing
//!     button/heading/link name from content for each child (no space, display:block)        ×3
//!     button/heading/link name from content for each child (no space, display:inline-block) ×3
//!     heading name from content with text-transform:uppercase / lowercase / capitalize       ×3
//!   wai-aria  399/434 (91.9%)  CONTROL, unchanged
//!   html-aam  310/335 (92.5%)  CONTROL, unchanged
//! ```
//!
//! ## ⭐⭐⭐ The same markup names two different things, and only a stylesheet separates them
//!
//! ```html
//!   <button><span>one</span><span>two</span><span>three</span></button>
//! ```
//!
//! is `"onetwothree"` when the spans are inline and `"one two three"` the moment CSS makes them
//! `display:block`. accname's *Computed Name from Content* appends a separator around a child that
//! is not an inline box — so a name walk that reads only the DOM **cannot** be right, whatever else
//! it gets correct. The same is true of `text-transform`: a heading styled `uppercase` is named
//! `"CALL US"`, because the name is the text a user is *read*, not the text an author typed.
//!
//! ⚠ **`inline-block` separates TOO, and that row is what decides the predicate.** The WPT fixture
//! asserts `"one two three"` for `display:inline-block` spans as well as `display:block` ones: the
//! rule is *"not an inline box"*, and an inline-block is an **atomic inline** — a block box that
//! participates in a line rather than an inline box. A predicate written as "is it block?" passes
//! three of these six rows.
//!
//! ⚠ `capitalize` upper-cases the first typographic letter of each word and **leaves the rest as
//! authored** — `"Call us"` → `"Call Us"`, not `"Call US"`. It is not `to_uppercase` on the first
//! character of a `split_whitespace`.
//!
//! ## ⚠⚠⚠ BOTH ENTRANCES, ASSERTED AGAINST EACH OTHER — this is I3, sharpened by check #128
//!
//! The name walk is reached through **two doors**: the AX tree builder (what a live page's agent
//! reads) and the bare `accessible_name` behind `test_driver.get_computed_label()` (what the
//! conformance suite reads). This file has been caught by that split three times — t1097's generated
//! content, t1350's case fold, t1355's name entry — so this gate does not test one door and trust
//! the other: **every row asserts that the two agree, and that the agreed value is Chrome's.** A fix
//! wired to one entrance fails here even though its own suite number moves.
//!
//! PROVEN RED by four mutations — see the module tail.

use manuk_a11y::{
    accessible_name_generated, empty_name_ctx, name_styles, role_of, A11yNode, GeneratedAlt,
};
use manuk_text::FontContext;

/// The WPT fixture's shape, reduced to the rows that need a computed style. `.block`/`.iblock` are
/// the classes `accname/name/comp_name_from_content.html` uses, spelled the same way.
const HTML: &str = r##"<!doctype html><html><head><style>
  .block > span { display: block; }
  .iblock > span { display: inline-block; }
</style></head><body>
  <button id="b_inline">      <span>one</span><span>two</span><span>three</span></button>
  <button id="b_block"  class="block"><span>one</span><span>two</span><span>three</span></button>
  <button id="b_iblock" class="iblock"><span>one</span><span>two</span><span>three</span></button>
  <h3 id="h_block"  class="block"><span>one</span><span>two</span><span>three</span></h3>
  <a href="#" id="a_block" class="block"><span>one</span><span>two</span><span>three</span></a>
  <h1 id="t_none"       style="text-transform:none">Call us</h1>
  <h1 id="t_upper"      style="text-transform:uppercase">Call us</h1>
  <h1 id="t_lower"      style="text-transform:lowercase">Call us</h1>
  <h1 id="t_cap"        style="text-transform:capitalize">Call us</h1>
  <h1 id="t_hidden">Visible <span style="display:none">gone</span></h1>
</body></html>"##;

fn node_id(page: &manuk_page::Page, id: &str) -> manuk_dom::NodeId {
    let dom = page.dom();
    dom.get_element_by_id(dom.root(), id)
        .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"))
}

/// The name as the **AX TREE** publishes it — the door a live page's agent reads.
fn tree_name(tree: &A11yNode, want_node: manuk_dom::NodeId) -> String {
    tree.iter()
        .find(|n| n.node == want_node)
        .unwrap_or_else(|| panic!("VACUOUS: node {want_node:?} is not in the a11y tree"))
        .name
        .clone()
}

#[test]
fn the_accessible_name_reads_the_computed_style_through_both_entrances() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://axn.test/", &fonts, 800.0);
    let dom = page.dom();
    let styles = name_styles(dom, page.styles_map());
    let tree = page.a11y_tree();

    // ── VACUITY. The style map must actually carry the two facts, or every row below is a
    // comparison of two identically-styleless answers.
    assert!(
        !styles.is_empty(),
        "VACUOUS: the NameStyles map is empty, so nothing below can be a test of it"
    );
    {
        let span = dom
            .descendants(node_id(&page, "b_block"))
            .find(|&n| dom.tag_name(n) == Some("span"))
            .expect("VACUOUS: #b_block has no <span>");
        assert!(
            manuk_a11y::name_separates(&styles, span),
            "VACUOUS: the `.block > span` rule did not reach the cascade, so the spacing rows below \
             would pass against an engine that ignores display entirely"
        );
        let inline_span = dom
            .descendants(node_id(&page, "b_inline"))
            .find(|&n| dom.tag_name(n) == Some("span"))
            .expect("VACUOUS: #b_inline has no <span>");
        assert!(
            !manuk_a11y::name_separates(&styles, inline_span),
            "VACUOUS: an INLINE span also separates, so the control row proves nothing and the \
             predicate is not reading `display` at all"
        );
    }

    // (id, the name Chrome computes, what the row is for)
    let rows: &[(&str, &str, &str)] = &[
        ("b_inline", "onetwothree", "CONTROL — inline children are NOT separated; this is the row a blanket separator breaks"),
        ("b_block", "one two three", "a display:block child contributes a space on each side"),
        ("b_iblock", "one two three", "an inline-block child does TOO — it is an atomic inline, not an inline box"),
        ("h_block", "one two three", "the same rule on a heading, whose name-from-content path is separate"),
        ("a_block", "one two three", "and on a link"),
        ("t_none", "Call us", "CONTROL — text-transform:none leaves the name as authored"),
        ("t_upper", "CALL US", "text-transform applies to the name, because the name is what a user is READ"),
        ("t_lower", "call us", "…and every keyword, not just the loud one"),
        ("t_cap", "Call Us", "capitalize upper-cases the first letter of each word and LEAVES THE REST — not `Call US`"),
        ("t_hidden", "Visible", "CONTROL — display:none still prunes, and threading style must not resurrect it"),
    ];

    for (id, want, why) in rows {
        let n = node_id(&page, id);
        let role = role_of(dom, n).unwrap_or_else(|| panic!("VACUOUS: #{id} maps to no ARIA role"));
        let generated = manuk_layout::generated_text(dom, page.styles_map());
        let alt = manuk_layout::generated_alt_text(dom, page.styles_map());
        // t1371 collapsed these three facts into `NameCtx`, so this composition is now the same one
        // line at both entrances instead of an argument list that can drift between them.
        let ctx = empty_name_ctx(&generated, &alt, &styles);

        // DOOR 1 — the bare name, behind `test_driver.get_computed_label()`.
        let bare = accessible_name_generated(dom, n, &role, &ctx);
        // DOOR 2 — the AX tree, which a live page's agent reads.
        let via_tree = tree_name(&tree, n);

        assert_eq!(
            bare, *want,
            "G_AX_NAME_COMPUTED_STYLE #{id} (bare name): Chrome computes {want:?}, got {bare:?}.\n  \
             {why}"
        );
        assert_eq!(
            via_tree, *want,
            "G_AX_NAME_COMPUTED_STYLE #{id} (AX TREE): Chrome computes {want:?}, got {via_tree:?}. \
             The bare name is {bare:?} — if those two differ, the fix reached ONE of the two \
             entrances, which is the t1097/t1350/t1355 shape this gate exists to refuse.\n  {why}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  `separates_name` returns false always (the pre-tick behaviour)
//       -> trips the VACUITY assert first, not a row, because `name_separates` is the same
//          predicate. That is honest and it is stated rather than dressed up as a row failure: the
//          vacuity check and the rule share an implementation, so N1 proves the fixture reaches the
//          cascade and N2 is the mutation that proves the RULE.
// N2  `separates_name` written as `matches!(d, Display::Block)` ("is it block?")
//       -> only `b_iblock` fails — the row that decides the predicate.
// N3  `transform_name` is the identity (the pre-tick behaviour)
//       -> the three transform rows read "Call us"; `t_none` stays green.
// N4  `capitalize` implemented as upper-casing the first char of each whitespace word and
//     upper-casing the remainder -> `t_cap` reads "Call US" instead of "Call Us".
// N5  thread `NameStyles` into the BARE entrance only, leaving `Page::a11y_tree` passing an empty
//     map -> every row's `bare` assertion passes and every `AX TREE` assertion fails. This is the
//     mutation that makes the two-door structure of the gate load-bearing rather than decorative.
