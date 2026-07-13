//! **G_SELECTOR — the cascade must not silently DROP rules.**
//!
//! `RuleIndex` — added in tick 14 as a cascade optimisation (339ms → 199ms) — walked each stylesheet's
//! rules, read each `StyleRule`'s `selectors` and `block`, and **never looked at its `rules` field.**
//! That field holds the rule's **nested** rules. Stylo parses them correctly and always has. We threw
//! every one of them away before it could match anything.
//!
//! Measured: **41% of the corpus uses CSS nesting** in its inline `<style>` blocks *alone* — external
//! stylesheets are not even scanned, so that is a **floor**, not an estimate. It is the single largest
//! cause of the two real rendering divergences the oracle found:
//!
//!   - *"we lose flex/grid on this node"* (11,324) — a nested `display: flex` never applied.
//!   - *"we show what Chrome hides"* (2,433) — a nested `display: none` never applied either, so we
//!     render menus, modals and off-screen panels that Chrome correctly hides.
//!
//! **The lesson, and it is the mirror image of the one this project keeps learning:** an optimisation
//! that makes a data structure *smaller* must be asked **what it dropped**. This one was measured for
//! speed and never once asked whether the rules it indexed were all the rules there were. A gate
//! comparing *boxes* could not see it, because the boxes it produced were internally consistent — they
//! were just consistently wrong.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head><style>
    div { display: block }

    /* ── CSS NESTING. 41% of the corpus, and every one of these rules was being DROPPED. */
    .card {
      color: red;
      & .nested-flex { display: flex }
      & .nested-hidden { display: none }
      .implicit-descendant { display: grid }
    }

    /* ── The selectors that already worked. Here so that "fixed nesting" cannot quietly break them. */
    :is(.card) .is-sel        { display: flex }
    :where(.card) .where-sel  { display: flex }
    .card:not(.zz) .not-sel   { display: flex }
    [data-k="v"] .attr-sel    { display: flex }
    [data-k^="v"] .attr-pre   { display: flex }
    .card > .a + .adj-sel     { display: flex }
    .card > .a ~ .sib-sel     { display: flex }
  </style></head><body>
    <div class="card" data-k="v">
      <div class="a"></div>
      <div class="adj-sel"></div>
      <div class="sib-sel"></div>
      <div class="nested-flex"></div>
      <div class="nested-hidden"></div>
      <div class="implicit-descendant"></div>
      <div class="is-sel"></div>
      <div class="where-sel"></div>
      <div class="not-sel"></div>
      <div class="attr-sel"></div>
      <div class="attr-pre"></div>
    </div>
  </body></html>"#;

#[test]
fn nested_rules_are_not_dropped_and_the_selectors_that_worked_still_work() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://sel.test/", &fonts, 800.0);
    let root = page.dom().root();

    let display_of = |class: &str| -> String {
        let hits = manuk_css::query_selector_all(page.dom(), root, &format!(".{class}"));
        assert!(!hits.is_empty(), "the .{class} element must exist");
        page.styles_of(hits[0])
            .map(|s| format!("{:?}", s.display).to_lowercase())
            .unwrap_or_else(|| "<NO STYLE>".into())
    };

    // ── (1) **CSS NESTING.** The whole point. Every one of these was silently dropped by `RuleIndex`.
    assert_eq!(
        display_of("nested-flex"),
        "flex",
        "G_SELECTOR: a NESTED `display: flex` did not apply.\n  \
         `RuleIndex` read each StyleRule's `selectors` and `block` and never looked at its `rules` \
         field — the nested rules. 41% of the corpus uses CSS nesting (a FLOOR: external stylesheets \
         are not even scanned), and this is the single largest cause of the oracle's \
         'we lose flex/grid on this node' divergence (11,324 nodes)."
    );
    assert_eq!(
        display_of("nested-hidden"),
        "none",
        "G_SELECTOR: a NESTED `display: none` did not apply — so we RENDER content Chrome hides.\n  \
         That is the oracle's 'we show what Chrome hides' divergence (2,433 nodes): menus, modals and \
         off-screen panels drawn on top of the page."
    );
    assert_eq!(
        display_of("implicit-descendant"),
        "grid",
        "G_SELECTOR: a nested rule with an IMPLICIT `&` (no ampersand written) did not apply. \
         `.card {{ .x {{ … }} }}` means `.card .x`, and it is by far the commonest form."
    );

    // ── (2) **And everything that already worked still works.** A fix that silently breaks the
    //        selectors the cascade already handled would be a far worse bug than the one it repaired,
    //        and it would be invisible — the rules would simply stop matching.
    for (class, want) in [
        ("is-sel", "flex"),
        ("where-sel", "flex"),
        ("not-sel", "flex"),
        ("attr-sel", "flex"),
        ("attr-pre", "flex"),
        ("adj-sel", "flex"),
        ("sib-sel", "flex"),
    ] {
        assert_eq!(
            display_of(class),
            want,
            "G_SELECTOR: `.{class}` regressed — it matched before the nesting fix and does not now. \
             A fix that breaks the selectors that already worked is worse than the bug it repaired."
        );
    }
}
