//! **G_A11Y_LABEL — THE `<label>` THAT WRAPS ITS CONTROL, WHICH IS THE SPELLING AUTHORS USE.**
//!
//! HTML has two ways to attach a label to a form control and this engine implemented one:
//!
//! ```html
//!   <label for="a">Remember me</label><input id="a" type="checkbox">   <- was found
//!   <label><input type="checkbox"> Remember me</label>                 <- WAS NOT FOUND
//! ```
//!
//! The wrapping (implicit) form invents no `id` and makes the text itself a click target, which is
//! why it is everywhere. Measured on WPT's `accname` suite before the fix, **35 subtests turned on
//! nothing but this** — the largest named mechanism in the suite's failures — and every fixture in
//! `comp_embedded_control` (13 more) is built out of it.
//!
//! ⚠⚠⚠ **THIS GATE LIVES IN `manuk-agent`, NOT IN `manuk-a11y`, AND THAT IS THE POINT.** The
//! accessible name is not a conformance number here: CONSTITUTION I3 makes the semantic tree a
//! load-bearing subsystem and the agent's observation channel resolves *"tick the Remember me
//! box"* through exactly this string. A checkbox inside its own `<label>` had **no name at all** —
//! an anonymous, unaddressable box on the commonest form idiom on the web. (It is also the only
//! crate that depends on `manuk-a11y` and is run by the wall, so a regression here is a RED tick
//! rather than a number nobody re-reads.)
//!
//! PROVEN RED under three mutations:
//!   N1  drop the implicit branch of `label_index`      -> every `encapsulation` arm reads ""
//!   N2  keep only the first associated label            -> the multi-label arm reads "first"
//!   N3  drop `embedded_control_value`                   -> "Flash the screen times"

use manuk_a11y::{accessible_name, role_of};

/// The accessible name the engine publishes for `#id` — the same call
/// `test_driver.get_computed_label()` makes, and the same one the agent reads.
fn name_of(html: &str, id: &str) -> String {
    let dom = manuk_html::parse(html);
    let n = dom
        .get_element_by_id(dom.root(), id)
        .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"));
    let role = role_of(&dom, n).unwrap_or_else(|| panic!("VACUOUS: #{id} maps to no ARIA role"));
    accessible_name(&dom, n, &role)
}

#[test]
fn a_label_that_wraps_its_control_names_it() {
    // ── 1. THE DEFECT. The control is INSIDE its label and has no `id` at all.
    assert_eq!(
        name_of(
            r#"<label><input type="checkbox" id="cb"> Remember me</label>"#,
            "cb"
        ),
        "Remember me",
        "an encapsulating <label> names the control it wraps. Without this the commonest checkbox \
         on the web is an anonymous box: the agent can click it and can never say what it is."
    );

    // ── 2. CONTROL — the `for=` spelling, which already worked, still works.
    assert_eq!(
        name_of(
            r#"<label for="e">Email address</label><input id="e" type="email">"#,
            "e"
        ),
        "Email address",
        "CONTROL: the explicit `for=` association is unchanged by the rewrite"
    );

    // ── 3. EVERY label, IN DOCUMENT ORDER. Two defects in one assertion: the old scan returned
    // ONE label, and it returned the LAST one, because it walked with `stack.pop()` after
    // `stack.extend(children)` — reverse document order per level.
    assert_eq!(
        name_of(
            r#"<label for="m">textfield label 1</label>
               <label for="m">textfield label 2</label>
               <input id="m" type="text">"#,
            "m"
        ),
        "textfield label 1 textfield label 2",
        "HTML-AAM concatenates EVERY associated label in TREE order. A single label, or the last \
         one, is a different name — and 'label 2 label 1' is the reverse-walk bug this replaced."
    );

    // ── 4. ⭐ accname §4.3 step 2C — a DIFFERENT control inside the label speaks its VALUE, and
    // the labelled control speaks nothing. Plain `text_content` gives "Flash the screen times",
    // which is a different instruction from the one on screen.
    assert_eq!(
        name_of(
            r#"<label><input type="checkbox" id="fl"> Flash the screen
               <input value="3" aria-label="number of times"> times</label>"#,
            "fl"
        ),
        "Flash the screen 3 times",
        "an embedded control contributes its VALUE (not its aria-label, not its subtree), and the \
         control being named contributes nothing to its own label"
    );

    // ── 5. The labelled control does not name itself, even when it carries a `value`.
    assert_eq!(
        name_of(
            r#"<label><input type="checkbox" id="v" value="test"> checkbox label</label>"#,
            "v"
        ),
        "checkbox label",
        "the control being named is EXCLUDED from its own label's text — otherwise its value is \
         folded back into the name of the thing that holds it"
    );

    // ── 6. An ARIA listbox embedded in a label speaks its SELECTED option, not all of them.
    assert_eq!(
        name_of(
            r#"<label><input type="checkbox" id="lb"> Flash the screen
               <ul role="listbox" aria-label="number of times">
                 <li role="option">1</li>
                 <li role="option" aria-selected="true">3</li>
                 <li role="option">5</li>
               </ul> times</label>"#,
            "lb"
        ),
        "Flash the screen 3 times",
        "a listbox contributes the SELECTED option. Walking its subtree gives '1 3 5' — every \
         value the user did NOT choose, which is worse than no name."
    );

    // ── 7. `aria-valuetext` outranks `aria-valuenow`, which outranks the rendered content: the
    // author wrote the spoken form on purpose.
    assert_eq!(
        name_of(
            r#"<label><input type="checkbox" id="sl"> Flash the screen
               <span role="slider" aria-valuenow="3.0" aria-valuetext="3">3.0</span> times</label>"#,
            "sl"
        ),
        "Flash the screen 3 times",
        "aria-valuetext is the author's spoken form of the value and outranks both aria-valuenow \
         and the element's own rendered text"
    );

    // ── 8. `<select>` inside its label — a different labelable element down the same path.
    assert_eq!(
        name_of(
            r#"<label>select label <select id="s"><option>foo</option></select></label>"#,
            "s"
        ),
        "select label",
        "encapsulation is a property of the LABEL, not of `<input>` — every labelable element gets it"
    );

    // ── 9. CONTROL — `for` WINS EVEN WHEN IT RESOLVES TO NOTHING. A `<label for="typo">` wrapped
    // around a control labels it in no engine, and silently falling back to the wrapped control
    // would invent a name the user never sees.
    assert_eq!(
        name_of(
            r#"<label for="typo"><input type="checkbox" id="q"> not my label</label>"#,
            "q"
        ),
        "",
        "CONTROL: a present-but-dangling `for` is still an explicit association, so the implicit \
         one must NOT be used as a fallback"
    );

    // ── 10. CONTROL — `aria-label` still beats the host-language label.
    assert_eq!(
        name_of(
            r#"<label><input type="checkbox" id="ov" aria-label="From ARIA"> From the label</label>"#,
            "ov"
        ),
        "From ARIA",
        "CONTROL: accname order is unchanged — aria-label outranks the host-language label"
    );

    // ── 11. `<input type=image>` is a button whose face is a picture: it is named by `alt`, and it
    // was reaching neither arm (the `alt` arm matches the TAG, this one only read `value`).
    assert_eq!(
        name_of(
            r#"<input type="image" id="img" alt="image input label" src="x.gif">"#,
            "img"
        ),
        "image input label"
    );

    // ── 12. The same host-language clause pointing one level IN rather than one level OUT.
    assert_eq!(
        name_of(
            r#"<fieldset id="fs"><legend>fieldset legend label</legend><input></fieldset>"#,
            "fs"
        ),
        "fieldset legend label",
        "a <fieldset> is named by its <legend>"
    );
    assert_eq!(
        name_of(
            r#"<table id="tb"><caption>table caption label</caption><tr><td>1</td></tr></table>"#,
            "tb"
        ),
        "table caption label",
        "a <table> is named by its <caption>"
    );

    // ── 13. CONTROL — a `<label>` around something UNLABELABLE labels nothing, so the "first
    // labelable descendant" rule has to check rather than take the first element it meets.
    assert_eq!(
        name_of(
            r#"<label>wrapper text <a href="/x" id="lk">link text</a></label>"#,
            "lk"
        ),
        "link text",
        "CONTROL: a link is not a labelable element — it keeps its name from content, and does not \
         inherit the text of a <label> that happens to enclose it"
    );
}
