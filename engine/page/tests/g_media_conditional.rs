//! **G_MEDIA_CONDITIONAL — a rule inside `@media` is a rule, and it reaches every property.**
//!
//! The minimal cascade's parser skipped `@media` along with every other at-rule, which deleted
//! every rule inside it. That looked harmless under the Stylo cascade, because Stylo re-parses
//! the sheet source with its own parser and evaluates media queries correctly — so `display`,
//! `width`, `color` and the rest were fine, and a `@media` unit test written against those
//! properties passed.
//!
//! But **a dozen properties are not read from Stylo at all.** Stylo's *servo* build does not
//! expose `visibility`, `mask-image`, `background-image`/`-size`/`-position`, `border-style`,
//! `text-shadow`, `object-fit`/`-position` or `vertical-align`, so `cascade_via_stylo` recovers
//! each of them from a second, MinimalCascade pass over the same sheets. Those were exactly the
//! properties `@media` erased. The property set that fails and the property set a naive test
//! covers are disjoint, which is why this survived a passing `@media` test in the same file as
//! the bug.
//!
//! ## What it cost on the real web
//!
//! `.vector-dropdown .vector-dropdown-content { visibility: hidden }` is inside an `@media` block
//! in Wikipedia's stylesheet — as the equivalent is on essentially every site, because a closed
//! dropdown/popover/tooltip is hidden this way (animatable, unlike `display:none`) and the rule
//! that hides it lives in a responsive block. Every one of those panels computed `visible`,
//! stayed laid out at full size, painted over the page and **swallowed clicks on the content
//! underneath**.
//!
//! It is worse than one property, because the page pipeline wraps a conditional
//! `<link media="(prefers-color-scheme: dark)">` sheet in `@media …{ }` on purpose, so that the
//! cascade decides whether it applies rather than that decision being reimplemented elsewhere.
//! With `@media` skipped, that whole sheet lost the same dozen properties — every background
//! image, gradient and icon mask it defined.
//!
//! ## The RED probe (run, not imagined)
//!
//! Reverting `parse_rules_into` to `skip_at_rule` for `@media` flips `mediaVis` and `darkBg`.
//! Making `media_matches` return `true` unconditionally — the plausible wrong fix, "just apply
//! them" — flips `printVis`, `narrowVis`, `darkBg` and `nestedNo`: a print sheet and a
//! dark-scheme sheet would apply to a light screen, which is not less wrong than dropping them.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  /* MATCHING — the live-page shape: a hidden dropdown panel behind a responsive block. */
  @media screen { .dropdown .panel { visibility: hidden } }
  /* NON-matching: a print sheet, and a breakpoint this 1280px viewport is far above. */
  @media print { #printed { visibility: hidden } }
  @media (max-width: 100px) { #narrow { visibility: hidden } }
  /* The conditional-stylesheet path: `wrap_media` produces exactly this shape. */
  @media (prefers-color-scheme: dark) { #dark { background-image: url(dark.png) } }
  @media (prefers-color-scheme: light) { #light { background-image: url(light.png) } }
  /* Nesting is conjunction: the inner query must ALSO hold. */
  @media screen { @media (min-width: 10px) { #nestYes { visibility: hidden } } }
  @media screen { @media (min-width: 99999px) { #nestNo { visibility: hidden } } }
</style></head><body>
  <div class="dropdown"><div class="panel" id="panel">menu</div></div>
  <div id="printed"></div><div id="narrow"></div>
  <div id="dark"></div><div id="light"></div>
  <div id="nestYes"></div><div id="nestNo"></div>
</body></html>"##;

fn hidden(page: &manuk_page::Page, sel: &str) -> bool {
    let n = manuk_css::query_selector_all(page.dom(), page.dom().root(), sel)[0];
    page.styles_of(n).map(|s| s.visibility) != Some(manuk_css::Visibility::Visible)
}

fn has_bg(page: &manuk_page::Page, sel: &str) -> bool {
    let n = manuk_css::query_selector_all(page.dom(), page.dom().root(), sel)[0];
    page.styles_of(n)
        .is_some_and(|s| !s.background_images.is_empty())
}

#[test]
fn media_blocks_apply_when_they_match_and_only_then() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://media.test/", &fonts, 1280.0);

    for (claim, got, want, why) in [
        (
            "mediaVis",
            hidden(&page, "#panel"),
            true,
            "`visibility:hidden` inside `@media screen` must apply — this is the closed-dropdown \
             rule on essentially every site, and without it the panel paints over the page and \
             eats clicks",
        ),
        (
            "printVis",
            hidden(&page, "#printed"),
            false,
            "`@media print` must NOT apply on screen — 'descend into @media' is only correct if \
             the query is still evaluated",
        ),
        (
            "narrowVis",
            hidden(&page, "#narrow"),
            false,
            "`@media (max-width:100px)` must not apply at a 1280px viewport",
        ),
        (
            "darkBg",
            has_bg(&page, "#dark"),
            false,
            "a `prefers-color-scheme: dark` block must not apply — this is the shape the page \
             pipeline wraps a conditional `<link media>` sheet in, so getting it wrong renders \
             the whole site in the wrong theme",
        ),
        (
            "lightBg",
            has_bg(&page, "#light"),
            true,
            "…and the `light` block must apply, so the answer is a decision and not a refusal",
        ),
        (
            "nestedYes",
            hidden(&page, "#nestYes"),
            true,
            "nested `@media` applies when both queries hold",
        ),
        (
            "nestedNo",
            hidden(&page, "#nestNo"),
            false,
            "…and does not when the inner one fails — nesting is conjunction, not the outer \
             query alone",
        ),
    ] {
        assert_eq!(got, want, "G_MEDIA_CONDITIONAL {claim}: {why}");
    }
}
