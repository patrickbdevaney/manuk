//! **PROBE — does per-glyph font fallback actually fire for CJK, emoji and RTL?**
//!
//! Not a gate yet. The lever board lists "font fallback across scripts (CJK/emoji)" as a `?`, an
//! UNKNOWN — and an unknown is cheaper to probe than to assume. `FALLBACK_FAMILIES` exists and
//! names Noto CJK/emoji/Arabic/Hebrew, so the machinery is *present*; whether it *works* is a
//! different question, and the project has been wrong about exactly this four times (`localStorage`,
//! `FormData`, `position: sticky`, `IntersectionObserver` — all built, all assumed missing).
//!
//! **An absent measurement is not a negative measurement.** So: measure.
//!
//! What tofu actually looks like, mechanically: the primary face (a Latin font) has no glyph for
//! `あ`, so `glyph_id` comes back **0** — `.notdef`, the empty box the user sees. Real fallback
//! means the glyph is non-zero AND it was shaped from a *different* face than the Latin primary.
//! Checking only "did we emit a glyph" would pass on a run of pure tofu, which is the failure this
//! probe exists to detect.

use manuk_text::{FontContext, FontFamily, FontKey};

fn key() -> FontKey {
    FontKey {
        family: FontFamily::SansSerif,
        bold: false,
        italic: false,
    }
}

/// Print, don't assert — the point is to LEARN the state, then write the gate against what is true.
#[test]
fn probe_what_the_text_stack_does_with_non_latin_script() {
    let fonts = FontContext::new();
    let k = key();
    let primary_latin = {
        let r = fonts.shape("A", k, 16.0);
        r.glyphs.first().map(|g| g.face)
    };
    println!("\n=== SCRIPT FALLBACK PROBE ===");
    println!("faces registered: {}", fonts.face_count());
    println!("latin primary face: {primary_latin:?}\n");

    for (label, sample) in [
        ("latin", "Hello"),
        ("japanese", "こんにちは"),
        ("chinese", "你好世界"),
        ("korean", "안녕하세요"),
        ("emoji", "😀🎉"),
        ("arabic", "مرحبا"),
        ("hebrew", "שלום"),
        ("devanagari", "नमस्ते"),
    ] {
        let run = fonts.shape(sample, k, 16.0);
        let chars = sample.chars().count();
        let notdef = run.glyphs.iter().filter(|g| g.glyph_id == 0).count();
        let faces: std::collections::BTreeSet<_> =
            run.glyphs.iter().map(|g| format!("{:?}", g.face)).collect();
        let fell_back = run.glyphs.iter().any(|g| Some(g.face) != primary_latin);
        println!(
            "{label:11} chars={chars:2} glyphs={:2} notdef={notdef:2} width={:7.2} \
             fell_back={fell_back:5} faces={faces:?}",
            run.glyphs.len(),
            run.width
        );
    }
    println!("=== END PROBE ===\n");
}
