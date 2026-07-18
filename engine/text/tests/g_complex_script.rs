//! **G_COMPLEX_SCRIPT — the shaper is told WHICH SCRIPT it is shaping.**
//!
//! swash's `ShaperBuilder` defaults `script` to `Script::Latin`, and we never called `.script()`.
//! The script is what **selects the OpenType feature set**, so every run on the web was shaped with
//! Latin's: no `init`/`medi`/`fina` joining, no `akhn`/`half`/`pres` conjunct formation, no matra
//! reordering. Arabic came out as five disconnected isolated letterforms; Devanagari as one glyph
//! per codepoint with conjuncts unformed.
//!
//! **This is why it survived so long: nothing was missing.** No `.notdef`, no tofu, no error, a
//! plausible-looking width — the per-glyph *fallback* worked correctly and picked the right face,
//! so the text rendered as real letters that happen to be wrong. To anyone who does not read the
//! script it looks fine, which is the worst shape a rendering bug takes. The probe that found it
//! (`probe_script_fallback.rs`) had to compare glyph COUNT against char count to see it at all.
//!
//! ⚠ These assertions need real system fonts for each script. Absent, they skip **loudly** (the
//! crate's existing precedent — see the metric assertions in `lib.rs`) rather than passing quietly:
//! a gate that silently no-ops on a machine without Noto is counted as coverage and is vacuous.

use manuk_text::{FontContext, FontFamily, FontKey};

fn key() -> FontKey {
    FontKey {
        family: FontFamily::SansSerif,
        bold: false,
        italic: false,
    }
}

/// Did we actually resolve a non-primary face for this sample? If not, the script's font is not
/// installed and there is nothing to assert about its shaping.
fn has_font_for(fonts: &FontContext, sample: &str, primary: Option<u32>) -> bool {
    let run = fonts.shape(sample, key(), 16.0);
    !run.glyphs.is_empty()
        && run.glyphs.iter().all(|g| g.glyph_id != 0)
        && run.glyphs.iter().any(|g| Some(g.face) != primary)
}

#[test]
fn a_complex_script_is_shaped_with_its_own_script_not_latin() {
    let fonts = FontContext::new();
    let k = key();
    let primary = fonts.shape("A", k, 16.0).glyphs.first().map(|g| g.face);

    // ── DEVANAGARI: conjuncts collapse codepoints into fewer glyphs. ──────────────────────────
    //
    // `नमस्ते` is 6 codepoints. `स` + virama + `त` forms the conjunct स्त, and the `े` matra
    // reorders — so a correct shaper emits FEWER glyphs than codepoints. Shaped as Latin it is a
    // flat 1:1 mapping, 6 glyphs, with the virama rendered as a visible dangling mark.
    let deva = "नमस्ते";
    if has_font_for(&fonts, deva, primary) {
        let run = fonts.shape(deva, k, 16.0);
        let chars = deva.chars().count();
        assert!(
            run.glyphs.len() < chars,
            "G_COMPLEX_SCRIPT: Devanagari `{deva}` shaped to {} glyphs for {chars} codepoints — a \
             1:1 mapping means the conjunct never formed and the virama is rendering as a visible \
             dangling mark. The shaper was told `Script::Latin`, which has no `akhn`/`half`/`pres`.",
            run.glyphs.len()
        );
    } else {
        eprintln!("G_COMPLEX_SCRIPT: no Devanagari font installed; skipping that assertion");
    }

    // ── ARABIC: a letter's form depends on its NEIGHBOURS. ────────────────────────────────────
    //
    // The strongest available statement of "joining happened", and it needs no font-specific
    // constants: shape the whole word, then shape one interior letter ALONE. In joined text that
    // letter takes a medial form — a different glyph id from its isolated form. Shaped as Latin,
    // every letter keeps its isolated form and the two ids are identical.
    let arabic = "مرحبا";
    if has_font_for(&fonts, arabic, primary) {
        let word = fonts.shape(arabic, k, 16.0);
        let interior: char = arabic.chars().nth(3).expect("sample has 5 letters");
        let alone = fonts.shape(&interior.to_string(), k, 16.0);
        let alone_id = alone.glyphs.first().map(|g| g.glyph_id);
        let in_word: Vec<u16> = word.glyphs.iter().map(|g| g.glyph_id).collect();
        assert!(
            alone_id.is_some_and(|id| !in_word.contains(&id)),
            "G_COMPLEX_SCRIPT: Arabic `{arabic}` — the interior letter `{interior}` kept its \
             ISOLATED glyph id {alone_id:?} inside the word (word glyphs: {in_word:?}). Arabic \
             letters join: an interior letter must take a medial form, a different glyph. \
             Identical ids mean `init`/`medi`/`fina` never ran, and the word renders as \
             disconnected letterforms — real glyphs, correct font, unreadable text."
        );
    } else {
        eprintln!("G_COMPLEX_SCRIPT: no Arabic font installed; skipping that assertion");
    }

    // ── AND THE SCRIPTS THAT ALREADY WORKED MUST NOT REGRESS. ─────────────────────────────────
    //
    // Script segmentation splits runs that face segmentation alone did not. The risk it introduces
    // is over-splitting — a run cut per-character shapes nothing correctly and loses kerning. CJK
    // is genuinely 1:1, so its glyph count is a stable invariant; Latin must be untouched.
    for (label, sample, expect) in [("latin", "Hello", 5), ("cjk", "你好世界", 4)] {
        let run = fonts.shape(sample, k, 16.0);
        if run.glyphs.is_empty() {
            eprintln!("G_COMPLEX_SCRIPT: no font for {label}; skipping");
            continue;
        }
        assert_eq!(
            run.glyphs.len(),
            expect,
            "G_COMPLEX_SCRIPT: {label} `{sample}` shaped to {} glyphs, expected {expect} — script \
             segmentation must not over-split a run that was already correct",
            run.glyphs.len()
        );
        assert!(
            run.glyphs.iter().all(|g| g.glyph_id != 0),
            "G_COMPLEX_SCRIPT: {label} `{sample}` produced .notdef — tofu, the per-glyph fallback \
             stopped resolving a face that covers it"
        );
    }

    // ── A MIXED RUN: the neutral characters between scripts must not cut a word. ──────────────
    //
    // Spaces and punctuation are `Script::Common` and carry no script. If they opened a new run,
    // an Arabic word split at its own space would stop joining across the cut — reintroducing the
    // exact bug, but only in running text, where it is hardest to notice.
    let mixed = "hi مرحبا 你好";
    let run = fonts.shape(mixed, k, 16.0);
    assert!(
        run.width > 0.0 && run.glyphs.iter().all(|g| g.glyph_id != 0),
        "G_COMPLEX_SCRIPT: mixed-script line `{mixed}` produced .notdef or zero width — \
         segmenting by (face, script) must still cover every character"
    );
}
