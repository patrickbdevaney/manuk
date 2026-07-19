//! **G_QUIRKS_MODE — the parser's quirks verdict reaches the style system, and `compatMode` tells the truth.**
//!
//! Measured at tick 241 and found to be a **dead-end wire**, which is worse than an absent feature:
//! html5ever detected quirks correctly and `engine/html/src/sink.rs` stored the verdict in a field that
//! was **written and never read**, while every Stylo call site hard-coded `QuirksMode::NoQuirks` and
//! `document.compatMode` returned a hard-coded `"CSS1Compat"` behind a comment asserting *"our documents
//! are never quirks-mode"*. The parser knew; nothing asked.
//!
//! **The two halves must land together, and that is the whole design constraint.** Reporting
//! `BackCompat` while still rendering standards is not a smaller lie than reporting `CSS1Compat` — it
//! is a worse one, because it is *actionable by the page*: a site that branches on `compatMode` would
//! take a quirks code path this engine does not honour. So this gate asserts **rendering and reporting
//! agree**, in both modes.
//!
//! **The quirk under test is the unitless length**, chosen because it is the one WPT states most
//! crisply (`quirks/unitless-length/`: `{input:"1", q:"1px"}` — `width: 1` is `1px` in quirks and
//! *invalid* in standards) and because Stylo already implements it: `AllowQuirks::allowed(quirks_mode)`
//! at `stylo/style/values/specified/length.rs:1175`. We are not implementing a quirk — we are
//! delivering a verdict Stylo is already waiting for. That is what makes this plumbing rather than
//! layout math.
//!
//! **A doctype is the only difference between the two fixtures.** Anything else varying between them
//! would let a difference in the *content* impersonate a difference in the *mode*.

use manuk_text::FontContext;

/// Identical documents, distinguished ONLY by the presence of `<!doctype html>`.
fn doc(doctype: &str) -> String {
    format!(
        r#"{doctype}<html><body style="margin:0">
<div id="a" style="width: 100; height: 40px; background: #00f"></div>
<div id="out">-</div>
<script>document.getElementById('out').textContent = document.compatMode;</script>
</body></html>"#
    )
}

fn measure(doctype: &str) -> (f32, String) {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(&doc(doctype), "https://quirks.test/", &fonts, 800.0);
    let root = page.dom().root();
    let a = manuk_css::query_selector_all(page.dom(), root, "#a")[0];
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let width = page.node_rects()[&a].width;
    (width, page.dom().text_content(out))
}

// **ONE test function, deliberately.** Three `#[test]`s in this binary SIGSEGV'd: each runs
// `Page::load`, cargo runs them on separate threads, and SpiderMonkey is not shared-thread-safe — the
// crash was the harness, not the claim. It also produced a subtler artifact first: `compatMode` read
// back as the placeholder `"-"` on one test and the real value on another, i.e. a script that silently
// did not run. A gate whose fixture is racing itself cannot tell a regression from its own harness, so
// all the claims live in one sequential test.

#[test]
fn the_parsers_quirks_verdict_reaches_both_layout_and_compat_mode() {
    let (qw, qc) = measure("");
    let (sw, sc) = measure("<!doctype html>");

    // ── 1. Reporting, both directions.
    assert_eq!(
        qc, "BackCompat",
        "G_QUIRKS_MODE: a document with NO doctype is quirks-mode, so document.compatMode must be \
         'BackCompat' — got {qc:?}. The parser has always known this; the verdict was thrown away."
    );
    assert_eq!(
        sc, "CSS1Compat",
        "G_QUIRKS_MODE: a document WITH a doctype is standards-mode — got {sc:?}."
    );

    // ── 2. Rendering, both directions. WPT quirks/unitless-length: {input:"1", q:"1px"}.
    assert!(
        (qw - 100.0).abs() < 0.5,
        "G_QUIRKS_MODE: in quirks mode `width: 100` is the unitless-length quirk and resolves to 100px \
         — got {qw}. ~800 means the declaration was DROPPED as invalid and the div fell back to its \
         block auto width, i.e. the quirks verdict never reached Stylo."
    );
    assert!(
        sw > 700.0,
        "G_QUIRKS_MODE: in standards mode `width: 100` is INVALID (no unit) and must be dropped, \
         leaving the block at its auto width (~800) — got {sw}. A value near 100 means the quirk is \
         applied unconditionally: the same bug in the opposite direction."
    );

    // ── 3. The claim that ties the other four together. Each assertion above can be satisfied by a
    //    constant; this one cannot. If the modes ever collapse into one, in EITHER direction, this fails.
    assert_ne!(
        qc, sc,
        "G_QUIRKS_MODE: compatMode reported {qc:?} for BOTH documents — the value is constant, i.e. \
         nobody is reading the parser's verdict."
    );
    assert!(
        (qw - sw).abs() > 50.0,
        "G_QUIRKS_MODE: the same `width: 100` laid out at {qw} in quirks and {sw} in standards — the \
         modes are indistinguishable to LAYOUT even if compatMode differs. That is the failure this \
         gate exists to catch: REPORTING a mode we do not RENDER."
    );
}
