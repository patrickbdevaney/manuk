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

    /* ── `:has()` — Stylo DISCARDS these rules; we match them with our own engine (tick 42). */
    .card:has(.probe)              { color: blue }
    .card:has(.probe) .has-desc    { display: flex }
    .card:has(> .probe) .has-child { display: flex }
    .row:has(+ .after)             { display: grid }
    .card:has(.nope) .has-nomatch  { display: flex }   /* must NOT match — there is no .nope */

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
      <div class="probe"></div>
      <div class="has-desc"></div>
      <div class="has-child"></div>
      <div class="has-nomatch"></div>
      <div class="row"></div>
      <div class="after"></div>
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

    // ── (1b) **`:has()`** — Stylo's *servo* build hardcodes `parse_has() -> false`, so these rules fail
    //         to parse and error-recovery discards the WHOLE rule: the declarations never reach the
    //         cascade. 13% of the corpus. We match them with our own selector engine instead of
    //         vendoring Stylo — see STATUS.md, "a borrowed engine is a means, not a constraint".
    assert_eq!(
        display_of("has-desc"),
        "flex",
        "G_SELECTOR: `.card:has(.probe) .has-desc` did not match. `:has()` with a DESCENDANT argument is \
         by far the commonest form, and Stylo drops the rule entirely rather than merely failing to \
         match it."
    );
    assert_eq!(
        display_of("has-child"),
        "flex",
        "G_SELECTOR: `:has(> .probe)` (child combinator) did not match."
    );
    assert_eq!(
        display_of("row"),
        "grid",
        "G_SELECTOR: `.row:has(+ .after)` (next-sibling combinator) did not match. The leading combinator \
         decides the SEARCH SPACE, and searching the subtree for a sibling selector would be both wrong \
         and slow."
    );
    // **And a `:has()` that should NOT match must not match.** A supplement that applies its rules
    // indiscriminately would be a far worse bug than the missing feature: it would restyle the page.
    assert_eq!(
        display_of("has-nomatch"),
        "block",
        "G_SELECTOR: `.card:has(.nope) .has-nomatch` MATCHED — but there is no `.nope` anywhere. The \
         supplement is applying `:has()` rules without checking them, which restyles the page."
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

/// **The attribute-selector case-sensitivity flag (`[attr=val i]` / `[attr=val s]`) and the namespace
/// prefix (`[*|attr]`, `[|attr]`).** Selectors §6.3.
///
/// This was the single largest matching gap in `css/selectors`: the value's `i`/`s` flag was *stripped
/// and discarded*, so `[foo='bar' i]` matched `foo="BAR"` case-**sensitively** and returned nothing —
/// and a namespaced attribute name (`*|foo`) was carried into the match verbatim, matching no attribute.
/// ~117 subtests hung off exactly these two mechanisms. Probe: `foo="BAR"` on a real element.
#[test]
fn attribute_selector_case_flag_and_namespace() {
    const H: &str = r#"<!doctype html><html><body>
        <div id="t" foo="BAR" baz="quux"></div>
      </body></html>"#;
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(H, "https://sel.test/", &fonts, 800.0);
    let root = page.dom().root();
    let matches = |sel: &str| !manuk_css::query_selector_all(page.dom(), root, sel).is_empty();

    // ── The `i` flag makes value matching ASCII case-insensitive — the mechanism that was missing.
    for sel in [
        "[foo='bar' i]",  // quoted, spaced
        "[foo='bar'i]",   // quoted, abutting
        "[foo=bar i]",    // unquoted
        "[foo='bar' I]",  // the flag itself is case-insensitive
        "[foo~='bar' i]", // …and it applies to every operator, not just `=`
        "[foo^='ba' i]",
        "[foo$='ar' i]",
        "[foo*='a' i]",
    ] {
        assert!(
            matches(sel),
            "G_SELECTOR: `{sel}` did not match foo=\"BAR\" — the `i` (ASCII case-insensitive) flag is \
             being dropped instead of applied. This is the largest matching gap in css/selectors."
        );
    }

    // ── Default and `s` are case-SENSITIVE — the flag must not leak case-insensitivity into plain
    //    matching, or half of every attribute test starts passing for the wrong reason.
    assert!(
        !matches("[foo='bar']"),
        "G_SELECTOR: `[foo='bar']` matched foo=\"BAR\" — attribute values are case-sensitive by default."
    );
    assert!(
        !matches("[foo='bar' s]"),
        "G_SELECTOR: `[foo='bar' s]` matched foo=\"BAR\" — the `s` flag forces case-SENSITIVE matching."
    );
    assert!(
        matches("[foo='BAR']") && matches("[baz='quux' s]"),
        "G_SELECTOR: an exact-case value stopped matching — the flag parser ate part of the value."
    );

    // ── The namespace prefix resolves to the local attribute name (HTML: everything is null-namespace).
    for sel in ["[*|foo='BAR']", "[|foo='BAR']", "[*|foo]", "[|foo]"] {
        assert!(
            matches(sel),
            "G_SELECTOR: `{sel}` did not match the `foo` attribute — the `*|`/`|` namespace prefix is \
             being carried into the attribute name instead of stripped."
        );
    }
}
