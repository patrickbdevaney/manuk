//! # G_FIRST_LETTER — `::first-letter` exists, and its RANGE is the feature
//!
//! CSS 2.1 §5.12.1. Until tick 1078 this pseudo-element was not in the engine at any layer: the
//! rule parsed (Stylo's *servo* build has `PseudoElement::FirstLetter`, and the selectors crate
//! accepts the CSS2 single-colon spelling), and then nothing ever asked a selector whether it
//! carried one. So `p:first-letter { font-size: 200% }` matched, cascaded, and was discarded — the
//! silent shape of failure this project keeps finding.
//!
//! **It was 10.5% of the whole CSS 2.1 suite's remaining failures** (t1077), and 339 of those 358
//! tests are one sentence of the spec:
//!
//! > Punctuation (i.e. characters defined in Unicode in the "open" (Ps), "close" (Pe), "initial"
//! > (Pi), "final" (Pf) and "other" (Po) punctuation classes) that precedes or follows the first
//! > letter should be included.
//!
//! So the styling is not the feature — **the range is**. `)A)lpha` has a three-character first
//! letter, `Charlie` has a one-character one, and `—Echo` has a one-character one for a reason
//! worth stating: the em dash is `Pd`, which is *not* one of the five classes the spec names, so it
//! is not skipped over as leading punctuation — it becomes the first letter itself.
//!
//! ## The row that discriminates, and why a smaller fixture would have shipped the bug
//!
//! The first implementation resolved the range inside a single `InlineItem::Word` and passed
//! `)Alpha`, `(Alpha`, `[Alpha`, `.Alpha` — then failed `}Bravo` and `!India`. Same Unicode
//! classes, opposite results, which is the signature of a **second mechanism**: UAX #14. `)` and
//! `]` are line-break class `CP`, `(` `[` `{` are `OP`, and both forbid a break before the letter
//! that follows — but `}` is `CL` and `!` is `EX`, which permit one, so the line breaker had
//! already handed layout *two words* and the first held no letter at all. 196 of the 339 tests were
//! on the wrong side of that split. **`#b` and `#f` below are the whole reason this gate is not a
//! one-liner.**
//!
//! ## Measured against Chrome, not reasoned
//!
//! Headless Chrome, 20px monospace (12px advance), `div:first-letter { background: red }`, reading
//! the red run's width straight off the screenshot — the range in characters:
//!
//! ```text
//!                      Chrome   ours
//!   )A)lpha              3        3     leading AND trailing punctuation
//!   }Bravo               2        2     the UAX #14 CL case
//!   Charlie              1        1     the plain case
//!   —Echo                1        1     Pd is NOT one of the five classes — the dash IS the letter
//!   «Golf                2        2     Pi
//!   !India               2        2     Po, and the other UAX #14 case (EX)
//! ```
//!
//! ## How this goes RED
//!
//! - Drop the `Pe`/`Po` arms from `first_letter_len`'s class set → `#a` loses its trailing `)`,
//!   `#b` and `#f` lose their leading punctuation entirely.
//! - Resolve the range inside one word instead of across the run → `#b` and `#f` regress to no
//!   first letter at all (their first word is pure punctuation), while `#a` and `#c` still pass —
//!   which is exactly the state this gate was written to be able to tell apart.
//! - Delete the `Atomic`/`Break` bail in `apply_first_letter` → `#d` reddens the `D` of `Delta`
//!   behind an image, which CSS 2.1 forbids and `first-letter-selector-002` asserts.
//! - Drop the `PseudoElement::FirstLetter` bucket from `PseudoIndex` → every row collapses.
//!
//! ## Result
//!
//! `manuk-wpt --wpt ~/wpt css/CSS2/selectors`, same binary tree, same hour:
//! **85 passed / 380 failed → 436 passed / 29 failed**, and the whole `css/CSS2` chapter
//! **2272 → 2624 passed** with no test that passed before failing after.
//!
//! ONE `#[test]` per file — `PageContext` is per-process (see `g_caption_paint.rs`).

use manuk_paint::DisplayItem;
use manuk_text::FontContext;

/// A monospace face keeps the assertions about *which characters* are in the range independent of
/// the metrics of whatever font resolves; the gate asserts font SIZE per run, not advances.
const HTML: &str = r##"<!doctype html><html><head><style>
body { margin: 0; font: 16px/1.5 monospace }
div.fl:first-letter { font-size: 40px }
</style></head><body>
<div class="fl">)A)lpha</div>
<div class="fl">}Bravo</div>
<div class="fl">Charlie</div>
<div class="fl"><img src="nonexistent.png" alt="" width="20" height="20"/>Delta</div>
<div class="fl">&#x2014;Echo</div>
<div class="fl">&#x0021;India</div>
<div>)Z)ulu</div>
</body></html>"##;

/// Every painted text run as `(text, rounded font-size)`, in paint order.
fn runs(list: &manuk_paint::DisplayList) -> Vec<(String, i32)> {
    list.items
        .iter()
        .filter_map(|it| match it {
            DisplayItem::Text { text, style, .. } => {
                Some((text.clone(), style.font_size.round() as i32))
            }
            _ => None,
        })
        .collect()
}

/// The concatenation of the leading runs set at `40` that together make up one element's first
/// letter, keyed by the 16px run that follows them. Returns `None` when nothing at 40px precedes
/// `rest` — i.e. the pseudo did not apply, which is an assertable outcome and not a lookup failure.
fn first_letter_before(runs: &[(String, i32)], rest: &str) -> Option<String> {
    let at = runs.iter().position(|(t, _)| t == rest)?;
    let mut out = String::new();
    let mut i = at;
    while i > 0 && runs[i - 1].1 == 40 {
        out.insert_str(0, &runs[i - 1].0);
        i -= 1;
    }
    (!out.is_empty()).then_some(out)
}

#[test]
fn first_letter_covers_the_spec_s_punctuation_range_and_nothing_else() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://example.test/", &fonts, 800.0);
    let list = page.display_list();
    let r = runs(&list);
    let seen = || format!("{r:?}");

    // ── The range, one row per mechanism. Chrome's own numbers are in the table above.
    assert_eq!(
        first_letter_before(&r, "lpha").as_deref(),
        Some(")A)"),
        "§5.12.1: punctuation that PRECEDES OR FOLLOWS the first letter is part of it — {}",
        seen()
    );
    assert_eq!(
        first_letter_before(&r, "ravo").as_deref(),
        Some("}B"),
        "`}}` is UAX #14 class CL, so the line breaker already split `}}Bravo` into two words — the \
         range has to be resolved ACROSS them — {}",
        seen()
    );
    assert_eq!(
        first_letter_before(&r, "harlie").as_deref(),
        Some("C"),
        "the plain case: one letter, no punctuation — {}",
        seen()
    );
    assert_eq!(
        first_letter_before(&r, "Echo").as_deref(),
        Some("\u{2014}"),
        "an em dash is Pd, which the spec's five classes do NOT include, so it is not skipped as \
         leading punctuation — it IS the first letter, and `Echo` keeps its own size — {}",
        seen()
    );
    assert_eq!(
        first_letter_before(&r, "ndia").as_deref(),
        Some("!I"),
        "`!` is Po and UAX #14 class EX — the other half of the two-word case — {}",
        seen()
    );

    // ── NEGATIVE 1. A replaced element before the text cancels the pseudo entirely (§5.12.1; the
    //    suite asserts it in `first-letter-selector-002`). Nothing here may be at 40px.
    assert!(
        first_letter_before(&r, "Delta").is_none(),
        "an <img> precedes the text, so there is no first letter to style — {}",
        seen()
    );
    assert!(
        r.iter().any(|(t, s)| t == "Delta" && *s == 16),
        "…and `Delta` must still be painted, at the element's own size — {}",
        seen()
    );

    // ── NEGATIVE 2. The CONTROL: the same punctuation shape with NO rule. If this row ever splits,
    //    the pseudo is being applied to elements that never asked for it.
    assert!(
        r.iter().any(|(t, s)| t == ")Z)ulu" && *s == 16),
        "a div with no ::first-letter rule must not be split at all — {}",
        seen()
    );
}
