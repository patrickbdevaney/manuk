//! **G_SRCSET_SELECTION — `srcset` and `<picture>` choose the image; `src` is only the fallback.**
//!
//! The constellation carried `srcset / <picture>` as **`works`** with no gate and a receipt reading
//! `capability-probe.html`. `g_img_current_src`'s own header said the opposite — *"it does not yet do
//! srcset/`<picture>` candidate selection for the bitmap"*. Tick 581's map sweep put the two side by
//! side; tick 582 measured, and the header was right:
//!
//! ```text
//! <img srcset="wide-2x.png 2x, wide-1x.png 1x" src="fallback.png">   requested: fallback.png
//! <img srcset="w400.png 400w, w800.png 800w" sizes="100vw" ...>      requested: fallback2.png
//! <picture><source srcset="from-source.png"><img src="from-img.png"> requested: from-img.png
//! ```
//!
//! Every candidate list ignored, every time, silently.
//!
//! ## Why this is not a 2×-display nicety
//!
//! The row's `what_breaks_without_it` said *"2x displays get 1x images"*, which undersells it by a lot
//! and is probably why it sat unmeasured:
//!
//! - **WordPress emits `srcset`+`sizes` on essentially every content image**, as does every modern CMS,
//!   Next.js `<Image>`, and every image CDN. On a `w`-descriptor list the `src` is frequently the
//!   *smallest* candidate — so we were not picking a 1× image, we were picking a thumbnail and scaling
//!   it across a hero.
//! - **`<picture>` is how a site serves AVIF/WebP with a JPEG fallback.** Taking `<img src>` there is
//!   safe but wastes the modern format the author shipped.
//! - **`<img srcset>` with NO `src` is legal**, and it is what a `<picture>`-less responsive image
//!   often looks like. There we requested *nothing at all* and rendered an empty box.
//!
//! ## The claims
//!
//! Selection is `Page::subresources()` — the fetch worklist — because that is where the choice is
//! actually made; asserting `currentSrc` alone would let a fix that reports one URL and fetches
//! another pass. Viewport is 800 CSS px at DPR 1 in these tests, which fixes every expected answer.
//!
//! ⚠ **The AVIF case is the one to read.** This gate was first written expecting an
//! `<source type="image/avif">` to WIN — an expectation taken from what the web serves rather than
//! from what this engine decodes. `image` is built with `["png","jpeg","gif","webp","bmp","ico"]` and
//! AVIF is **deliberately off** (its decoder is C dav1d). So the honest answer is the opposite: skip
//! it and take the `<img>` fallback. Honouring `type` is therefore not a nicety — it is what lets a
//! site ship AVIF without us rendering an empty box, and it is the single most valuable half of
//! `<picture>` support for an engine with a deliberately narrow decoder set.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<!-- x-descriptors at DPR 1: the 1x candidate wins, NOT the first listed and NOT `src`. -->
<img id="x" srcset="wide-2x.png 2x, wide-1x.png 1x" src="fallback.png">
<!-- w-descriptors with sizes=100vw at 800px: 800w is the smallest candidate that covers it. -->
<img id="w" srcset="w400.png 400w, w800.png 800w, w1600.png 1600w" sizes="100vw" src="fallback2.png">
<!-- `<picture>`: the first <source> whose type/media match wins over the <img src>. WebP is in the
     decoder's feature list, so this one is honestly choosable. -->
<picture><source srcset="from-source.webp" type="image/webp"><img id="p" src="from-img.jpg"></picture>
<!-- AVIF is DELIBERATELY OFF in the `image` crate's feature list (the C dav1d dependency is declined),
     so an AVIF <source> must be SKIPPED and the <img> fallback used — which is exactly what `type` is
     for, and the reason a site can ship AVIF without breaking us. -->
<picture><source srcset="modern.avif" type="image/avif"><img id="skip" src="good.png"></picture>
<!-- A non-matching media query must be skipped too. -->
<picture><source srcset="print-only.png" media="print"><img id="med" src="screen.png"></picture>
<!-- srcset with NO src at all: we used to request nothing and render an empty box. -->
<img id="nosrc" srcset="only-candidate.png 1x">
<!-- The control: a plain src still works. -->
<img id="plain" src="plain.png">
</body></html>"##;

fn urls(page: &manuk_page::Page) -> Vec<String> {
    page.subresources().iter().map(|s| s.url.clone()).collect()
}

#[test]
fn srcset_and_picture_choose_the_image() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://sr.test/", &fonts, 800.0);
    let got = urls(&page);
    println!("SRCSET SELECTION -> {got:?}");
    let has = |u: &str| got.iter().any(|g| g.ends_with(u));

    for (want, why) in [
        (
            "wide-1x.png",
            "x-descriptors: at DPR 1 the `1x` candidate wins. Taking `src` here is the measured bug; \
             taking `wide-2x.png` would be the other cheap wrong fix (first listed wins)",
        ),
        (
            "w800.png",
            "w-descriptors with `sizes=100vw` in an 800px viewport: 800w is the smallest candidate \
             that covers the slot. On a `w` list the `src` is often the SMALLEST image, so ignoring \
             the list does not cost a 2x asset — it renders a thumbnail across a hero",
        ),
        (
            "from-source.webp",
            "`<picture>`: the first `<source>` whose `type` we can decode beats the `<img src>` — \
             that is the entire reason the element exists. WebP is in the decoder's feature list",
        ),
        (
            "good.png",
            "AVIF is deliberately OFF in the `image` crate's features (the C dav1d dependency is \
             declined), so an AVIF `<source>` must be SKIPPED and the `<img>` fallback used. \
             Choosing it would render NOTHING — worse than ignoring srcset entirely. This is the \
             claim that makes `type` worth honouring rather than a decoration",
        ),
        (
            "screen.png",
            "a `<source media=print>` does not match, so the `<img>` fallback is used",
        ),
        (
            "only-candidate.png",
            "`<img srcset>` with NO `src` is legal and common; we used to request nothing and render \
             an empty box",
        ),
        ("plain.png", "THE CONTROL: a plain `src` still works"),
    ] {
        assert!(has(want), "G_SRCSET_SELECTION: expected {want}\n  got: {got:?}\n\n  {why}.");
    }

    // The negative half — the fallbacks the candidate lists REPLACE must not be fetched as well.
    // Without this a fix that fetches everything would satisfy every assertion above and double the
    // page's image bytes, which is the opposite of what responsive images are for.
    for (unwanted, why) in [
        (
            "fallback.png",
            "the `src` is a FALLBACK: with a usable srcset it must not be fetched too",
        ),
        ("fallback2.png", "same, for the w-descriptor list"),
        (
            "from-img.jpg",
            "the `<picture>`'s `<img src>` is not fetched when a `<source>` matched",
        ),
        ("wide-2x.png", "the 2x candidate is not fetched at DPR 1"),
        ("w1600.png", "the oversized candidate is not fetched"),
        ("print-only.png", "a non-matching `<source>` is not fetched"),
        (
            "modern.avif",
            "an undecodable `<source>` is not fetched — we would not be able to show it",
        ),
    ] {
        assert!(
            !has(unwanted),
            "G_SRCSET_SELECTION: {unwanted} must NOT be fetched\n  got: {got:?}\n\n  {why}."
        );
    }
}
