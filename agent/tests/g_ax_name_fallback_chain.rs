//! **G_AX_NAME_FALLBACK_CHAIN — the last steps of the accessible-name chain, in the right order.**
//!
//! Surface audit #80's ranked #1, third pass, against CDP `Accessibility.getFullAXTree`.
//!
//! ## THE FOUR DEFECTS, AND THEY ARE ALL "THE END OF THE CHAIN"
//!
//! ```text
//!                                                   chrome    before    after
//!   <input placeholder="PH" title="TT">             TT        PH        TT
//!   <textarea placeholder="TA">                     TA        (none)    TA
//!   <input type=submit>            (no value)       Submit    (none)    Submit
//!   <input type=reset>             (no value)       Reset     (none)    Reset
//!   <input type=submit value="">   CONTROL          (none)    (none)    (none)
//!   <input type=button>            CONTROL          (none)    (none)    (none)
//!   <table summary="TS">                            TS        (none)    TS
//! ```
//!
//! ⭐⭐⭐ **`title` BEATS `placeholder`, AND WE HAD IT THE OTHER WAY ROUND.** HTML-AAM's input chain
//! is `<label>` → `aria-label` → **`title`** → `placeholder`; ours applied `placeholder` inside step
//! 3 (host-language label), which put it *ahead* of the step-5 tooltip. A placeholder is the hint
//! that disappears the moment the user types; a `title` is the author's stated label. Announcing the
//! transient one is not a tie-break, it is the wrong answer.
//!
//! ⭐⭐ **`<input type=submit>` WITH NO `value` IS THE COMMONEST SUBMIT BUTTON ON THE WEB, AND IT WAS
//! NAMELESS.** The UA renders the word *Submit* on it; HTML-AAM names it by that default. Without it
//! *"click Submit"* resolves to nothing — a form an agent can fill and cannot send.
//!
//! ⭐ **`type=button` is the control that stops this being a blanket rule.** Chrome-measured, a
//! valueless `<input type=button>` has **no name at all**, because the UA renders no default label
//! on it. Three button types, two defaults.
//!
//! ⭐ **`value=""` SUPPRESSES the default** — the same rule this file already carries for
//! `<img alt="">`: *an explicit empty host-language label is an answer, not a missing one.* The
//! attribute's PRESENCE is the discriminator, not its content.
//!
//! ⭐ **`<textarea placeholder>` was nameless because the rule lived inside an `el.name == "input"`
//! branch.** One rule, two elements, and only one of them had it — the shape this file has been
//! caught by five times.
//!
//! ## ⚠ THE `<table summary>` ROW SHADOWED THE `<caption>` ARM, AND THE EXISTING GATE CAUGHT IT
//!
//! Written first as its own `"table" => …` match arm, it shadowed the `"fieldset" | "table"` arm
//! below that reads `<caption>` — so **every captioned table went nameless** and
//! `G_A11Y_LABEL`'s *"a `<table>` is named by its `<caption>`"* row went red on the first run.
//! `summary` is a FALLBACK BEHIND the caption, Chrome-measured across all four combinations:
//!
//! ```text
//!   summary + caption   -> CAP        caption wins
//!   summary alone       -> TS
//!   caption alone       -> CAP
//!   + aria-label        -> AL         which beats both
//! ```
//!
//! > **A new arm in a match on tag names is a SHADOWING hazard, and the tag that already has an arm
//! > is the one you are about to break.**
//!
//! ## ⚠ MEASURED AND NOT BUILT
//!
//! ```text
//!   <div title="DT">content</div>     chrome: ""      ours: "DT"
//!   <abbr title="Abbrev">AB</abbr>    chrome: "Abbrev"  ours: "Abbrev"   ✓
//! ```
//!
//! `title` is a name fallback only for elements HTML-AAM says so — on a plain `<div>` it is a
//! DESCRIPTION, not a name. Ours applies it universally. Narrowing it needs its own battery of which
//! elements title-names (the `<abbr>` row shows the rule is not simply "generic cannot be named"),
//! and getting that wrong DELETES names rather than adding them, so it is recorded rather than
//! guessed at.

use manuk_a11y::{accessible_name_generated, empty_name_ctx, name_styles, role_of};

const HTML: &str = r##"<!doctype html><html><head></head><body>
<input id="n1" placeholder="Search here">
<input id="n2" title="Tip">
<input id="n3" placeholder="PH" title="TT">
<input id="n4" aria-label="AL" placeholder="PH" title="TT">
<label for="n5">Lbl</label><input id="n5" placeholder="PH" title="TT">
<input id="n10" type="submit">
<input id="n11" type="reset">
<input id="n12" type="button">
<input id="n18" type="submit" value="Go">
<input id="n19" type="reset" value="Clear">
<input id="n20" type="submit" value="">
<textarea id="n13" placeholder="TA"></textarea>
<table id="n30" summary="TS"><caption>CAP</caption><tr><td>x</td></tr></table>
<table id="n31" summary="TS"><tr><td>x</td></tr></table>
<table id="n32"><caption>CAP</caption><tr><td>x</td></tr></table>
<table id="n33" summary="TS" aria-label="AL"><caption>CAP</caption><tr><td>x</td></tr></table>
<button id="n6" title="BT"></button>
<a id="n7" href="/x" title="AT"></a>
<img id="n8" src="x.png" title="IT">
<select id="n14" title="ST"><option>o</option></select>
<fieldset id="n16"><legend>L1</legend><legend>L2</legend></fieldset>
</body></html>"##;

#[test]
fn the_name_chain_ends_in_the_right_order() {
    let fonts = manuk_text::FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://name.test/", &fonts, 1200.0);
    let dom = page.dom();
    let styles = name_styles(dom, page.styles_map());
    let generated = manuk_layout::generated_text(dom, page.styles_map());
    let alt = manuk_layout::generated_alt_text(dom, page.styles_map());
    let ctx = empty_name_ctx(&generated, &alt, &styles);
    let name = |id: &str| {
        let n = dom
            .get_element_by_id(dom.root(), id)
            .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"));
        let role = role_of(dom, n).unwrap_or_else(|| panic!("VACUOUS: #{id} maps to no ARIA role"));
        accessible_name_generated(dom, n, &role, &ctx)
    };

    // ── VACUITY. The steps ABOVE the ones this gate changes must still win, or a chain that
    //    collapsed to "always take the last step" would satisfy every row below.
    assert_eq!(
        name("n5"),
        "Lbl",
        "VACUOUS: `<label>` no longer beats the chain below it"
    );
    assert_eq!(
        name("n4"),
        "AL",
        "VACUOUS: `aria-label` no longer beats the chain below it"
    );

    // (id, the name Chrome's AX tree reports, what the row decides)
    let rows: &[(&str, &str, &str)] = &[
        ("n3", "TT", "THE ORDER — `title` beats `placeholder`. A placeholder is the hint that disappears the moment the user types; a title is the author's stated label"),
        ("n1", "Search here", "CONTROL — placeholder alone still names, so the reorder did not delete the step"),
        ("n2", "Tip", "CONTROL — title alone still names"),
        ("n13", "TA", "`<textarea placeholder>` — the rule lived inside an `el.name == \"input\"` branch, so one of its two elements never had it"),
        ("n10", "Submit", "THE COMMONEST SUBMIT BUTTON ON THE WEB — no `value`, so the UA renders `Submit` and HTML-AAM names it that. Nameless before, and \"click Submit\" resolved to nothing"),
        ("n11", "Reset", "…and its twin"),
        ("n12", "", "CONTROL — a valueless `type=button` has NO name in Chrome, because the UA renders no default label on it. Three button types, two defaults"),
        ("n18", "Go", "CONTROL — an explicit `value` still wins over the default"),
        ("n19", "Clear", "CONTROL — the same for reset"),
        ("n20", "", "`value=\"\"` SUPPRESSES the default — an explicit empty host-language label is an answer, not a missing one. The attribute's PRESENCE is the discriminator"),
        ("n31", "TS", "`<table summary>` — the pre-ARIA spelling, still in HTML-AAM because a decade of pages use it"),
        ("n30", "CAP", "…and it is a FALLBACK BEHIND the caption. Writing it as its own match arm SHADOWED the caption arm and made every captioned table nameless"),
        ("n32", "CAP", "CONTROL — caption alone, the row the shadowing broke"),
        ("n33", "AL", "CONTROL — `aria-label` beats both"),
        ("n6", "BT", "CONTROL — a `<button title>` with no content"),
        ("n7", "AT", "CONTROL — an `<a title>` with no content"),
        ("n8", "IT", "CONTROL — an `<img title>` with no alt"),
        ("n14", "ST", "CONTROL — a `<select title>`"),
        ("n16", "L1", "CONTROL — the FIRST `<legend>`, not the last"),
    ];
    for (id, want, why) in rows {
        let got = name(id);
        assert_eq!(
            &got, want,
            "G_AX_NAME_FALLBACK_CHAIN #{id}: Chrome computes {want:?}, got {got:?}.\n  {why}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  move the `placeholder` step back above the `title` step (the pre-tick order)
//       -> only `n3` fails, at "PH". Both single-source controls stay green, which is what says the
//          defect was the ORDER and not either step.
// N2  drop the `el.attr("value").is_none()` guard on the UA default
//       -> only `n20` fails, at "Submit". The three valued rows agree either way, which is why the
//          empty-value row is in the fixture at all.
// N3  give `<table summary>` its own `"table" => …` match arm again
//       -> `n30` and `n32` fail (nameless) while `n31` passes — the shadowing, isolated, and the
//          shape that made an EXISTING gate go red on this tick's first run.
