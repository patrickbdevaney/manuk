//! **G_SUPPORTS_LAYER — `@supports` and `@layer` blocks are not deleted, and `@supports` is
//! ANSWERED rather than assumed.**
//!
//! Tick 273 fixed `@media` and wrote down that `@supports` and `@layer` were the same defect with a
//! different at-keyword: the minimal cascade skipped them, deleting every rule inside. Under the
//! Stylo cascade that is invisible for mainstream properties — Stylo re-parses the source itself —
//! but the twelve properties `cascade_via_stylo` recovers from this cascade (`visibility`,
//! `background-image`, `mask-image`, `border-style`, `text-shadow`, `object-fit`, …) were exempt
//! from every `@supports` and `@layer` block on the web.
//!
//! `@supports (display: grid) { … }` is how the modern web ships a layout with a fallback, and
//! `@layer` is how design systems order their cascade. A theme's gradients, icon masks and
//! dividers commonly live inside both.
//!
//! ## Why the condition is answered by TRYING the declaration
//!
//! The tempting implementation is a list of supported property names. That is a second source of
//! truth — the exact failure mode this repository has now hit four times — and it goes stale the
//! moment a property is implemented. Instead the declaration is parsed, applied to a default
//! `ComputedStyle`, and checked for whether anything moved.
//!
//! **The probe is conservative by construction.** A value equal to the initial value reads as
//! unsupported, so its block does not apply — which is precisely the old behaviour. It can be as
//! wrong as before, never newly wrong.
//!
//! ## `@supports` must be able to say NO
//!
//! An `@supports` block the browser cannot satisfy is not decoration: the author wrote a fallback
//! for exactly that case, and applying both is worse than applying neither. `notSupported` and
//! `negation` carry that claim, and they are what a "descend into everything" implementation fails.
//!
//! ## The RED probe (run, not imagined)
//!
//! Restoring `skip_at_rule` for `@supports` flips `supported`, `nested` and `negation`; restoring
//! it for `@layer` flips `layered`. Making `supports_condition_matches` return `true`
//! unconditionally — the plausible wrong fix — flips `notSupported` and `negation` while leaving
//! every positive claim green.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  @supports (display: grid)            { #supported { visibility: hidden } }
  @supports (wibble-property: 3px)     { #notSupported { visibility: hidden } }
  @supports not (wibble-property: 3px) { #negation { visibility: hidden } }
  @supports (display: grid) and (position: sticky) { #nested { visibility: hidden } }
  @supports (wibble-property: 3) or (display: grid) { #either { visibility: hidden } }
  @layer base { #layered { visibility: hidden } }
  @layer a, b;
  @media screen { @supports (display: grid) { #both { visibility: hidden } } }
  @media print  { @supports (display: grid) { #printOnly { visibility: hidden } } }
</style></head><body>
  <div id="supported"></div><div id="notSupported"></div><div id="negation"></div>
  <div id="nested"></div><div id="either"></div><div id="layered"></div>
  <div id="both"></div><div id="printOnly"></div>
</body></html>"##;

#[test]
fn supports_and_layer_blocks_apply_and_supports_can_say_no() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://sup.test/", &fonts, 1280.0);
    let dom = page.dom();
    let hidden = |sel: &str| {
        let n = manuk_css::query_selector_all(dom, dom.root(), sel)[0];
        page.styles_of(n).map(|s| s.visibility) != Some(manuk_css::Visibility::Visible)
    };

    for (id, want, why) in [
        (
            "supported",
            true,
            "`@supports (display: grid)` — grid IS implemented, so the block applies. The whole \
             block used to be deleted",
        ),
        (
            "notSupported",
            false,
            "an unimplemented property must be FALSE. The author wrote a fallback for this case, \
             and applying both the fallback and the enhancement is worse than applying neither",
        ),
        (
            "negation",
            true,
            "`not (unsupported)` is TRUE — this is how the web ships a rule that must apply only \
             to browsers lacking a feature, and it is the claim a 'descend into everything' \
             implementation gets backwards",
        ),
        ("nested", true, "`and` of two supported conditions"),
        (
            "either",
            true,
            "`or` where only the second holds — the operators are evaluated, not skipped",
        ),
        (
            "layered",
            true,
            "`@layer base { … }` must not delete its contents. Layer ORDER is still approximate; \
             deleting the rules outright was not approximate, it was absent",
        ),
        (
            "both",
            true,
            "`@supports` nested inside a matching `@media` — the two at-rules compose",
        ),
        (
            "printOnly",
            false,
            "…and a supported `@supports` inside a NON-matching `@media` must still not apply, so \
             descent did not lose the media condition",
        ),
    ] {
        assert_eq!(
            hidden(&format!("#{id}")),
            want,
            "G_SUPPORTS_LAYER {id}: {why}."
        );
    }
}
