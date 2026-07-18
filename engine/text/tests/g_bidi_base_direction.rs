//! **G_BIDI_BASE — an RTL paragraph is ordered from the right.**
//!
//! The Unicode Bidi Algorithm resolves every run against a paragraph **base level**, and
//! `FontContext::shape` hard-coded that base to LTR. So `direction: rtl` and `dir="rtl"` — how the
//! entire Arabic, Hebrew, Persian and Urdu web declares itself — changed nothing: every character
//! was present, correctly shaped (after tick 214) and **in the wrong order**.
//!
//! The visible symptoms, none of which look like "the text is backwards":
//!
//! - a sentence's trailing period lands on the **wrong end** of the line;
//! - an embedded Latin word or a number inside Arabic text is placed on the wrong side of its
//!   neighbours — so a price, a date or a product name lands somewhere else in the sentence;
//! - the line starts from the left, so short lines hug the wrong margin.
//!
//! ⚠ **HTML's initial value is `ltr`, not content detection.** An unmarked Arabic paragraph is LTR
//! in Chrome too, and this gate pins that: inferring RTL from content would be a *structural
//! divergence from Chromium*, which the north star calls a bug even when it looks more "correct".

use manuk_text::{FontContext, FontFamily, FontKey};

fn key() -> FontKey {
    FontKey {
        family: FontFamily::SansSerif,
        bold: false,
        italic: false,
    }
}

/// Glyph x-positions in the order the glyphs were emitted. Bidi reordering shows up here and
/// nowhere else — the glyph *ids* are identical either way, which is exactly why this was invisible.
fn xs(run: &manuk_text::ShapedRun) -> Vec<f32> {
    run.glyphs.iter().map(|g| g.x).collect()
}

#[test]
fn an_rtl_base_reorders_the_line_and_an_ltr_base_does_not() {
    let fonts = FontContext::new();
    let k = key();

    // A mixed line is the only honest test: pure-RTL text reorders trivially, and pure-LTR text
    // must not move at all. `ABC` inside Arabic is the case every real page hits — a brand name, a
    // product code, a URL — and it is where a wrong base level is actually visible.
    let mixed = "مرحبا ABC";
    let ltr = fonts.shape_bidi(mixed, k, 16.0, false);
    let rtl = fonts.shape_bidi(mixed, k, 16.0, true);

    if ltr.glyphs.is_empty() || rtl.glyphs.is_empty() {
        eprintln!("G_BIDI_BASE: no Arabic font installed; skipping");
        return;
    }

    // Same text, same font, same size — so any difference is ORDER, which is the whole claim.
    assert_eq!(
        ltr.glyphs.len(),
        rtl.glyphs.len(),
        "G_BIDI_BASE: the base direction changed the GLYPH COUNT ({} vs {}). It must only change \
         where glyphs are placed, never which ones are produced — a count change means bidi is \
         corrupting the run rather than reordering it.",
        ltr.glyphs.len(),
        rtl.glyphs.len()
    );
    // Width is *nearly* base-independent, and the residual is real rather than sloppy: the two
    // bases split the line into different bidi runs, so the space between `مرحبا` and `ABC` is
    // shaped as part of a different run and picks up a slightly different advance. Measured at
    // 0.89px on a 70px line (~1.3%). Shaping is per-run in every browser, so this is inherent, not
    // a defect — but it must stay SMALL, because layout measures direction-agnostically (`measure`
    // pins the base to LTR) while paint shapes with the real base. A large divergence here would
    // mean painted text overflowing the box layout reserved for it.
    let tol = 0.03 * ltr.width.max(1.0);
    assert!(
        (ltr.width - rtl.width).abs() < tol,
        "G_BIDI_BASE: the base direction changed the run WIDTH by more than {tol:.2}px \
         ({:.2} vs {:.2}). Bidi reorders runs, it does not resize them. Layout measures without \
         knowing the direction it will be painted in, so a real width divergence is text \
         overflowing the box that was reserved for it.",
        ltr.width,
        rtl.width
    );
    assert_ne!(
        xs(&ltr),
        xs(&rtl),
        "G_BIDI_BASE: `{mixed}` shaped IDENTICALLY under an LTR and an RTL base. The paragraph's \
         base level is being ignored, so `direction: rtl` / `dir=\"rtl\"` render exactly like LTR: \
         the Latin run `ABC` sits on the wrong side of the Arabic, and a trailing period lands on \
         the wrong end of the line. Every glyph is present and correctly shaped, and the line \
         reads backwards — which is why nothing else in the engine notices."
    );

    // ── PURE-LTR TEXT MUST BE BYTE-IDENTICAL UNDER BOTH BASES. ────────────────────────────────
    //
    // The risk this change introduces is that RTL support quietly perturbs the 99% case. Latin has
    // no RTL runs to reorder, so an RTL base must leave it exactly where an LTR base did.
    let latin = fonts.shape_bidi("Hello world", k, 16.0, false);
    let latin_rtl_base = fonts.shape_bidi("Hello world", k, 16.0, true);
    assert_eq!(
        xs(&latin),
        xs(&latin_rtl_base),
        "G_BIDI_BASE: pure-LTR text moved under an RTL base. There are no RTL runs to reorder, so \
         the visual order must be untouched — otherwise enabling RTL anywhere on a page disturbs \
         all of its Latin text."
    );

    // ── THE CACHE MUST NOT SERVE ONE BASE'S ORDER TO THE OTHER. ───────────────────────────────
    //
    // `RunKey` carries the base direction. Without it, the second shape of the same string is a
    // cache HIT returning the first one's ordering — correctly-shaped glyphs in the wrong places,
    // and only on the second paragraph, which is as hard a bug as this file could hide.
    let rtl_again = fonts.shape_bidi(mixed, k, 16.0, true);
    let ltr_again = fonts.shape_bidi(mixed, k, 16.0, false);
    assert_eq!(
        xs(&rtl_again),
        xs(&rtl),
        "G_BIDI_BASE: re-shaping under an RTL base returned different positions — the shaped-run \
         cache is keyed without the base direction"
    );
    assert_eq!(
        xs(&ltr_again),
        xs(&ltr),
        "G_BIDI_BASE: an LTR re-shape returned the RTL ordering — the cache served one base's \
         order to the other"
    );
}
