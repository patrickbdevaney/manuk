//! **G_PSEUDO_CASCADE** — `::before` / `::after` cascade correctly, and keep doing so now that
//! their rules are indexed once per document instead of re-walked per element.
//!
//! # Why this gate did not exist before, and why that was the problem
//!
//! Generated content carries a large share of the visible web — icons, quotation marks, counters,
//! dividers, the little chevrons on every disclosure widget — and `manuk-page` had **no gate on it
//! at all**. That is how `cascade_pseudo` was able to be, for its whole life, the one matcher that
//! never received the fix `RuleIndex` applied to every other rule: it ran a full recursive descent
//! over every rule in every stylesheet, **twice per element**, re-evaluating every `@media` query
//! against a device that had not changed, to find the handful of rules carrying a pseudo.
//!
//! Measured on a wix.com snapshot (10,424 nodes, 1.8 MB of CSS in 68 `<style>` blocks) with
//! `MANUK_CASCADE_PROFILE=1`: **9.0 s of each 19.5 s cascade — 46%.** The cascade runs 8× on that
//! page load, so it was ~72 s of a 165 s load. After hoisting the rules into a `PseudoIndex`:
//! **1.63 s** (5.5×), cascade 19.5 s → 11.3 s, and the end-to-end page load **164.7 s → 101.8 s**.
//!
//! # What this gate actually protects
//!
//! Not the speed — the **semantics the speed-up assumes**. Hoisting rules out of the sheet tree is
//! only sound if the resulting cascade is identical, and what the gate is RED-proven against is:
//!
//! - **Conditional groups.** `@media`, `@supports` and `@layer` are now descended once at
//!   index-build time rather than per element. A rule inside one must still reach the cascade
//!   (RED-proven by dropping the `@layer` arm → `#inlayer` loses its content), and a `@media`
//!   block that does *not* match must still be excluded (RED-proven by ignoring the evaluation →
//!   `#nomedia` gains content the author excluded).
//!
//! **A correction worth recording, because it was written here as a claim and then falsified.**
//! This gate first asserted that the index's global `order` counter is what preserves source
//! order at equal specificity. Two RED patches say otherwise: neither *stopping the counter
//! advancing on non-pseudo selectors* nor *never recording an order at all* changes any result.
//! The reason is that rules are collected in source order and `winners.sort_by_key` is a **stable**
//! sort, so source order survives on its own; the explicit counter is belt-and-braces. The `#ord`
//! case below is still asserted — the tie-break is real and must hold — but it is honest to say
//! the gate does not currently have a patch that breaks it, rather than to imply a guard it has
//! not earned.

use manuk_css::query_selector_all;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><head><style>
  /* 1. The plain case. */
  #plain::before { content: "PLAIN-BEFORE"; }
  #plain::after  { content: "PLAIN-AFTER"; }

  /* 2. SOURCE ORDER at equal specificity — one class each, so the LATER rule must win.
        Between them sits a non-pseudo selector: the order counter has to advance over it, or
        the two pseudo rules end up adjacent in the index and the tie can resolve either way. */
  .ord::before { content: "LOSER"; }
  .ord         { color: rgb(1, 2, 3); }
  .ord::before { content: "WINNER"; }

  /* 3. SPECIFICITY beats source order: the id rule is earlier but must still win. */
  #spec::before { content: "ID-WINS"; }
  .spec::before { content: "CLASS-LOSES"; }

  /* 4. Inside a conditional group that DOES apply. The viewport below is 1000px wide. */
  @media (min-width: 500px) {
    #inmedia::before { content: "MEDIA-APPLIED"; }
  }
  /* 5. ...and one that does NOT. This element must end with no generated content. */
  @media (min-width: 5000px) {
    #nomedia::before { content: "MEDIA-SHOULD-NOT-APPLY"; }
  }
  /* 6. @supports and @layer are descended at build time too. */
  @supports (display: grid) {
    #insupports::before { content: "SUPPORTS-APPLIED"; }
  }
  @layer base {
    #inlayer::before { content: "LAYER-APPLIED"; }
  }

  /* 7. An element matched by NO pseudo rule must come out with none — the index returning
        something for everyone would be just as wrong as returning nothing. */
</style></head>
<body>
  <p id="plain">p</p>
  <p id="ord" class="ord">o</p>
  <p id="spec" class="spec">s</p>
  <p id="inmedia">m</p>
  <p id="nomedia">n</p>
  <p id="insupports">u</p>
  <p id="inlayer">l</p>
  <p id="bare">b</p>
</body></html>"##;

/// The `content` string the cascade settled on for `#id`'s `::before`, if any.
fn before_of(page: &manuk_page::Page, sel: &str) -> Option<String> {
    let root = page.dom().root();
    let n = *query_selector_all(page.dom(), root, sel).first()?;
    page.styles_of(n)?.before.as_ref()?.content.clone()
}

fn after_of(page: &manuk_page::Page, sel: &str) -> Option<String> {
    let root = page.dom().root();
    let n = *query_selector_all(page.dom(), root, sel).first()?;
    page.styles_of(n)?.after.as_ref()?.content.clone()
}

#[test]
fn a_generated_content_cascades_by_specificity_then_source_order_through_conditional_groups() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://pseudo.test/", &fonts, 1000.0);

    for (sel, want, why) in [
        (
            "#plain",
            "PLAIN-BEFORE",
            "the simplest possible `::before` produced no generated content. Every icon, chevron \
             and quotation mark on the web is this rule",
        ),
        (
            "#ord",
            "WINNER",
            "two `::before` rules tie on specificity and the LATER one must win. A reset and its \
             override are usually both one class deep, so this is the common shape in real CSS, \
             not an edge case. Getting `LOSER` means the index stopped collecting in source order \
             or the winner sort stopped being stable — those two together are what preserve the \
             tie-break, NOT the explicit order counter (see the module note)",
        ),
        (
            "#spec",
            "ID-WINS",
            "specificity must outrank source order: the `#id` rule is written FIRST and must \
             still beat the later `.class` rule",
        ),
        (
            "#inmedia",
            "MEDIA-APPLIED",
            "a `::before` inside a MATCHING `@media` block never reached the cascade. Media \
             queries are now evaluated once at index-build time instead of per element; if that \
             evaluation is wrong or skipped, every responsive pseudo silently disappears",
        ),
        (
            "#insupports",
            "SUPPORTS-APPLIED",
            "a `::before` inside `@supports` was dropped — progressive-enhancement blocks are \
             where modern sites put the real design",
        ),
        (
            "#inlayer",
            "LAYER-APPLIED",
            "a `::before` inside `@layer` was dropped — design systems ship whole sheets inside \
             cascade layers",
        ),
    ] {
        let got = before_of(&page, sel);
        assert_eq!(
            got.as_deref(),
            Some(want),
            "G_PSEUDO_CASCADE: {sel}::before expected {want:?}, got {got:?}.\n\n  {why}."
        );
    }

    assert_eq!(
        after_of(&page, "#plain").as_deref(),
        Some("PLAIN-AFTER"),
        "G_PSEUDO_CASCADE: `::after` is bucketed separately from `::before`. If it is empty while \
         `::before` works, the two buckets have been conflated or one is never filled."
    );

    // The negative half. An index that hands every element the same rules would satisfy every
    // assertion above and still be completely wrong.
    assert_eq!(
        before_of(&page, "#nomedia"),
        None,
        "G_PSEUDO_CASCADE: a `::before` inside a NON-matching `@media (min-width: 5000px)` was \
         applied at a 1000px viewport. Generated content appearing where the author excluded it \
         is worse than it going missing — it puts text on the page that no stylesheet asked for."
    );
    assert_eq!(
        before_of(&page, "#bare"),
        None,
        "G_PSEUDO_CASCADE: an element matched by NO pseudo rule came out with generated content. \
         The index is returning candidates that could not have matched."
    );
}
