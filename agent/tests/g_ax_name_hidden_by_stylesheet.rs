//! **G_AX_NAME_HIDDEN_BY_STYLESHEET — a name fragment hidden by a CSS RULE is not announced.**
//!
//! Surface audit #79's ranked #1 was *"a gate that constructs its own input cannot discover that the
//! producer is broken — sweep the gates that build an `A11yNode` by hand and ask what each is
//! therefore blind to."* This gate is that sweep's first finding, and the finding is a real
//! shipping divergence, not a gate defect.
//!
//! ## THE BLIND SPOT, AND IT WAS SHARED BY WPT ITSELF
//!
//! accname §4.3 step 2A prunes a hidden node from the name. The engine's prune read
//! `inline_visibility` — the element's own **inline `style=` attribute** — whose doc-comment said so
//! and named the gap: *"a `display:none` applied by a CLASS is still missed."* It was missed for
//! real, and nothing in the tree could see it:
//!
//! ```text
//!   every hidden-node fixture in WPT accname/name/comp_labelledby_hidden_nodes.html
//!                                                        style="display: none"    inline
//!   G_AX_NAME_COMPUTED_STYLE's own `t_hidden` CONTROL row style="display:none"    inline
//!   the five a11y gates in agent/tests/ that build their input with manuk_html::parse
//!                                                        NO CASCADE AT ALL
//! ```
//!
//! ⭐⭐⭐ **A RULE WITH TWO SOURCES, WHERE THE WEAKER SOURCE IS THE ONE EVERY TEST USES, IS
//! INVISIBLE TO THE WHOLE SUITE.** `.sr-only`-style visibility toggles are authored in stylesheets
//! on the real web and inline in conformance fixtures, so a conformance-shaped test can be at 100%
//! on a mechanism that is wrong on every page.
//!
//! ## THE FIX — the map was already in the context
//!
//! t1365 threaded `NameStyles` (per-node computed `display` + `text-transform`) into the walk so a
//! non-inline child could contribute a separator. `display: none` was in that same map the entire
//! time and the prune never asked it. `node_visibility` now prefers the computed pair and falls back
//! to the inline reader when there is no style map (a `manuk_html::parse` fixture, a unit test).
//! `NameStyles` gained `visibility` for the other half of the rule.
//!
//! ## THE BATTERY — Chrome-measured through CDP `Accessibility.getFullAXTree`
//! (`--headless=new --force-renderer-accessibility`)
//!
//! ```text
//!                                                         chrome    before      after
//!  b1  .h{display:none}            STYLESHEET             "Save"  "Save SECRET" "Save"
//!  b2  style="display:none"        inline        CONTROL  "Save"  "Save"        "Save"
//!  b3  .h{visibility:hidden}       STYLESHEET             "Save"  "Save SECRET" "Save"
//!  b4  style="visibility:hidden"   inline        CONTROL  "Save"  "Save"        "Save"
//!  b5  aria-labelledby → a display:none span (stylesheet) "foo bar"  same       same
//!  b6  stylesheet none, child display:inline               "Save"  "Save SHOWN" "Save"
//!  b7  the `hidden` ATTRIBUTE                             "Save"  "Save"        "Save"
//!  b8  stylesheet visibility:hidden, child visible   "Save SHOWN"  "Save SHOWN" same
//!  b9  aria-hidden                               CONTROL  "Save"  "Save"        "Save"
//! b10  class none + inline display:inline        CONTROL  "Save SHOWN"   same    same
//! ```
//!
//! ⭐ **`b7` LOOKED LIKE THE DOM-READER CONTROL AND IS NOT — this gate's own vacuity assert caught
//! it.** The UA sheet carries `[hidden] { display: none }`, so the `hidden` attribute IS a computed
//! `display: none`; b7 is a control for the two sources AGREEING. `b9` (`aria-hidden`, which no
//! stylesheet can express) is the control for the DOM reader alone, and `b10` — a class that says
//! `none` beaten by an inline `display:inline` — is the control that separates *reading the computed
//! value* from *"either source says none"*, which is the wrong fix an implementer reaches for first.
//!
//! ⭐ **`b6` and `b8` are the pair that make this a RULE and not a second predicate.** `display:none`
//! PRUNES — a `display:inline` child inside it is still gone. `visibility:hidden` does NOT — it is
//! the one hiding mechanism a descendant can UNDO, and `visibility:visible` inside it is announced.
//! Reading the computed value gets both right for free, because `visibility` is inherited and the
//! cascade has already resolved the undo; the inline reader had to flow a flag down by hand.
//!
//! ⭐ **`b5` was already the right answer for the wrong reason, and it is asserted so it stays
//! right.** A referenced node that is itself hidden is EXEMPT (§4.3 step 2A) — its text is what the
//! author pointed at. Before this tick the engine could not see that the span was hidden at all, so
//! it walked it as a visible node and arrived at the same string. Two errors that cancel, which is a
//! thing this project has been caught by before.
//!
//! ⚠ Chrome's CDP `name.value` for `b3`/`b4` is `"Save "` with a TRAILING SPACE — the space from
//! `"Save "` survives when the following fragment contributes nothing. accname §4.3 step 2 trims the
//! total, and `normalize` does, so the rows assert the trimmed string. Named here rather than left
//! as an unexplained mismatch between this file and the CDP dump it was measured from.
//!
//! ⚠ Both DOORS are asserted on every row — the bare `accessible_name_generated` behind
//! `test_driver.get_computed_label()` and the AX TREE a live page's agent reads — because a fact
//! wired to one entrance is the shape this file has been caught by four times (t1097, t1350, t1355,
//! t1365).

use manuk_a11y::{accessible_name_generated, empty_name_ctx, name_styles, role_of, A11yNode};

const HTML: &str = r##"<!doctype html><html><head><style>
  .sheet-none { display: none }
  .sheet-vis  { visibility: hidden }
</style></head><body>
<button id="b1">Save <span class="sheet-none">SECRET</span></button>
<button id="b2">Save <span style="display:none">SECRET</span></button>
<button id="b3">Save <span class="sheet-vis">SECRET</span></button>
<button id="b4">Save <span style="visibility:hidden">SECRET</span></button>
<button id="b5" aria-labelledby="l5">x</button>
<span id="l5" class="sheet-none">foo <span>bar</span></span>
<button id="b6">Save <span class="sheet-none"><span style="display:inline">SHOWN</span></span></button>
<button id="b7">Save <span hidden>SECRET</span></button>
<button id="b8">Save <span class="sheet-vis"><span style="visibility:visible">SHOWN</span></span></button>
<button id="b9">Save <span aria-hidden="true">SECRET</span></button>
<button id="b10">Save <span class="sheet-none" style="display:inline">SHOWN</span></button>
</body></html>"##;

fn node_id(page: &manuk_page::Page, id: &str) -> manuk_dom::NodeId {
    let dom = page.dom();
    dom.get_element_by_id(dom.root(), id)
        .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"))
}

/// The name as the **AX TREE** publishes it — the door a live page's agent reads.
fn tree_name(tree: &A11yNode, want: manuk_dom::NodeId) -> String {
    tree.iter()
        .find(|n| n.node == want)
        .unwrap_or_else(|| panic!("VACUOUS: node {want:?} is not in the a11y tree"))
        .name
        .clone()
}

#[test]
fn a_name_fragment_hidden_by_a_stylesheet_is_not_announced() {
    let fonts = manuk_text::FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ax.test/", &fonts, 1200.0);
    let dom = page.dom();
    let styles = name_styles(dom, page.styles_map());
    let tree = page.a11y_tree();

    // ── VACUITY: the two CLASS rules must actually have reached the cascade. Without this, every
    //    row below could pass on an engine that applies no stylesheet at all — the `.sheet-none`
    //    span would simply be an ordinary visible span with the wrong text, which is a different
    //    bug wearing the same failure.
    let span_of = |id: &str| {
        dom.descendants(node_id(&page, id))
            .find(|&n| dom.tag_name(n) == Some("span"))
            .unwrap_or_else(|| panic!("VACUOUS: #{id} has no <span>"))
    };
    assert!(
        manuk_a11y::name_display_none(&styles, span_of("b1")),
        "VACUOUS: `.sheet-none {{ display: none }}` did not reach the cascade, so b1/b5/b6 test \
         nothing about hiding"
    );
    assert!(
        manuk_a11y::name_visibility_hidden(&styles, span_of("b3")),
        "VACUOUS: `.sheet-vis {{ visibility: hidden }}` did not reach the cascade, so b3/b8 test \
         nothing about hiding"
    );
    // ⭐ **`b7` IS NOT THE DOM-READER CONTROL IT LOOKS LIKE, AND THE FIRST RUN OF THIS GATE SAID
    //    SO.** The vacuity assert here was originally `!name_display_none(b7's span)` — *"the
    //    `hidden` attribute row must be hidden by the ATTRIBUTE, not by a stray display:none"* — and
    //    it fired. The UA sheet carries `[hidden] { display: none }`, so `hidden` IS a computed
    //    `display: none`. b7 is therefore a control for *the two sources agreeing*, and `b9`
    //    (`aria-hidden`, which no stylesheet can express) is the control for the DOM reader alone.
    assert!(
        manuk_a11y::name_display_none(&styles, span_of("b7")),
        "VACUOUS: the UA sheet's `[hidden] {{ display: none }}` did not reach the cascade, so b7 is \
         not the two-sources-agree row this gate claims it is"
    );
    assert!(
        !manuk_a11y::name_display_none(&styles, span_of("b9"))
            && !manuk_a11y::name_visibility_hidden(&styles, span_of("b9")),
        "VACUOUS: #b9's span must be hidden by `aria-hidden` ALONE — if a style hides it too, it is \
         not a control for the DOM reader"
    );
    assert!(
        !manuk_a11y::name_display_none(&styles, span_of("b10")),
        "VACUOUS: #b10's inline `display:inline` must WIN over the class — otherwise the row cannot \
         tell a computed read from an either-source read"
    );

    // (id, the name Chrome computes, what the row decides)
    let rows: &[(&str, &str, &str)] = &[
        ("b1", "Save", "THE DEFECT — a fragment hidden by a STYLESHEET rule was announced; the prune read the inline style attribute only"),
        ("b2", "Save", "CONTROL — the inline spelling, which is the only one WPT and this engine's own gates ever used"),
        ("b3", "Save", "the `visibility` half of the same defect, also by stylesheet"),
        ("b4", "Save", "CONTROL — the inline `visibility` spelling"),
        ("b5", "foo bar", "a hidden REFERENCE is EXEMPT (§4.3 2A) — its text is what the author pointed at, and this was previously right for the wrong reason"),
        ("b6", "Save", "`display:none` PRUNES: a `display:inline` child inside a hidden wrapper is still gone"),
        ("b7", "Save", "CONTROL — the `hidden` attribute, which is read off the DOM and must keep working"),
        ("b8", "Save SHOWN", "`visibility:hidden` does NOT prune: `visibility:visible` inside it is announced, and the computed value already carries that undo"),
        ("b9", "Save", "CONTROL for the DOM reader — `aria-hidden` is not a style and no cascade can express it"),
        ("b10", "Save SHOWN", "CONTROL — an inline `display:inline` OVERRIDES the class, so the walk must read the COMPUTED value and not `either source says none`"),
    ];

    for (id, want, why) in rows {
        let n = node_id(&page, id);
        let role = role_of(dom, n).unwrap_or_else(|| panic!("VACUOUS: #{id} maps to no ARIA role"));
        let generated = manuk_layout::generated_text(dom, page.styles_map());
        let alt = manuk_layout::generated_alt_text(dom, page.styles_map());
        let ctx = empty_name_ctx(&generated, &alt, &styles);

        let bare = accessible_name_generated(dom, n, &role, &ctx);
        let via_tree = tree_name(&tree, n);

        assert_eq!(
            bare, *want,
            "G_AX_NAME_HIDDEN_BY_STYLESHEET #{id} (bare name): Chrome computes {want:?}, got \
             {bare:?}.\n  {why}"
        );
        assert_eq!(
            via_tree, *want,
            "G_AX_NAME_HIDDEN_BY_STYLESHEET #{id} (AX TREE): Chrome computes {want:?}, got \
             {via_tree:?}. The bare name is {bare:?} — if those two differ, the fix reached ONE of \
             the two entrances.\n  {why}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  `node_visibility` returns `inline_visibility(dom, n)` unconditionally (the pre-tick behaviour)
//       -> b1, b3 and b6 read "Save SECRET" / "Save SECRET" / "Save SHOWN". The three stylesheet
//          rows fail and all four CONTROLS stay green, which is what identifies the mechanism as
//          "which source is consulted" rather than "the prune is broken".
// N2  `node_visibility` reports `display: none` but always `Some(true)` for visibility
//       -> only b3 fails. The two halves of the rule are separately load-bearing.
// N3  make `display: none` flow down as a flag instead of returning (i.e. treat it like visibility)
//       -> b6 reads "Save SHOWN": a `display:inline` child would undo a `display:none` ancestor.
