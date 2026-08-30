//! **G_A11Y_GENERATED_NAME_ENTRY — t1097 WAS FIXED AT ONE ENTRANCE, AND THIS IS THE OTHER ONE.**
//!
//! accname §4.3 step 2F folds `::before`/`::after` text into the accessible name, and t1097 built
//! that: `manuk_layout::generated_text` produces the map, the TREE builder threads it, and
//! `g_ax_generated_name` gates it. All correct — for the path a live page's AX tree takes.
//!
//! **The other path built an EMPTY map.** `manuk_a11y::accessible_name` — the function behind
//! `test_driver.get_computed_role/label()`, which is the only way the `accname` / `wai-aria` /
//! `html-aam` suites and the agent's role+name probe can see a name — called
//! `GeneratedText::new()`. So `button::before{content:"★ "}` was announced as *"Save"* there and
//! *"★ Save"* in the tree, on a mechanism the project had already built, gated and journaled.
//!
//! ⭐ **THE TWO-ENTRANCE SHAPE APPEARED THREE TIMES IN ONE SESSION** — t1350 (`Role::parse`
//! case-folded and `role_of` did not), t1353 (the label path walked and name-from-content
//! flattened), and this. *A fix belongs at the rule; a rule reached through two doors needs both
//! doors walked, not one door tested.*
//!
//! This gate composes exactly what `host_ax_role_name` composes — the real page's styles → the real
//! generated map → the public name entry point — so it fails if either half is unwired.
//!
//! Measured: `accname` 411/484 → 423/484, `wai-aria` and `html-aam` unchanged, zero newly-failing.
//!
//! PROVEN RED by the mutation the defect WAS: pass `GeneratedText::new()` instead of the real map.

use manuk_a11y::{accessible_name, accessible_name_generated, role_of};
use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head><style>
  #save::before { content: "★ "; }
  #tab::after   { content: " (opens in a new tab)"; }
</style></head><body>
  <button id="save">Save</button>
  <a href="/x" id="tab">Docs</a>
</body></html>"#;

#[test]
fn the_name_entry_point_carries_generated_content_not_an_empty_map() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ax-generated.test/", &fonts, 800.0);
    let dom = page.dom();

    // Exactly the composition `host_ax_role_name` performs.
    let generated = manuk_layout::generated_text(dom, page.styles_map());
    // t1365 threaded the computed style into the same walk, for the same reason and through the
    // same two entrances — a non-inline child separates the name with a space, and `text-transform`
    // applies to it. `host_ax_role_name` composes both, so this gate does too.
    let name_styles = manuk_a11y::name_styles(dom, page.styles_map());
    let gen_alt = manuk_layout::generated_alt_text(dom, page.styles_map());
    let ctx = manuk_a11y::empty_name_ctx(&generated, &gen_alt, &name_styles);

    let probe = |id: &str| -> (String, String) {
        let n = dom
            .get_element_by_id(dom.root(), id)
            .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"));
        let role = role_of(dom, n).unwrap_or_else(|| panic!("VACUOUS: #{id} maps to no ARIA role"));
        (
            accessible_name_generated(dom, n, &role, &ctx),
            accessible_name(dom, n, &role),
        )
    };

    // ── VACUITY GUARD: the map must actually contain something, or every assertion below is a
    // comparison of two identical empty answers and the gate cannot fail.
    assert!(
        !generated.is_empty(),
        "VACUOUS: `generated_text` produced NO entries for a document whose stylesheet sets \
         `content` on two elements. The gate would then be comparing the same empty map twice."
    );

    // ── 1. `::before` is part of the name, in that order around the content.
    let (with_gen, without) = probe("save");
    assert_eq!(
        with_gen, "★ Save",
        "accname §4.3 step 2F: a `::before` glyph is announced BEFORE the content"
    );

    // ── 2. ⭐ AND THE ENTRY POINT THAT DROPS THE MAP GIVES A DIFFERENT ANSWER. This is the defect
    // itself, asserted as a difference rather than described in a comment: if these two ever agree,
    // the parameter has stopped being load-bearing and the wiring is dead again.
    assert_eq!(
        without, "Save",
        "the un-generated entry point is the OLD answer — kept as the contrast that makes the \
         assertion above meaningful"
    );
    assert_ne!(
        with_gen, without,
        "⭐ THE PARAMETER MUST BE LOAD-BEARING. The bug was not a wrong rule, it was a correct rule \
         handed an EMPTY map by one of its two callers — so the thing to gate is that the map \
         CHANGES the answer, which no assertion on a single call can see."
    );

    // ── 3. `::after` on a link, where the pseudo carries the only text that says what the link
    // does. This is the case that matters past conformance: an agent told to avoid opening new
    // tabs cannot know without it.
    let (a_gen, a_plain) = probe("tab");
    assert_eq!(a_gen, "Docs (opens in a new tab)");
    assert_eq!(a_plain, "Docs");
}
