//! **G_A11Y_NAME_FROM_CONTENT — ONE accname WALK, TWO CALLERS, AND ONLY ONE OF THEM RECURSED.**
//!
//! accname §4.3 defines a single traversal. This engine had it twice: the `<label>` path walked
//! properly (skipping the labelled control, substituting an embedded control's value), while
//! **name-from-content flattened its subtree with `dom.text_content()`**. A flatten cannot see what
//! the spec puts in the middle of a name:
//!
//! ```html
//!   <button><span>one</span> <img alt="two"> <span>three</span></button>
//!   <h3>heading <a aria-label="link aria-label">ignored link text</a> heading</h3>
//! ```
//!
//! `text_content` reads the first as *"one three"* — the picture in the middle of the button is
//! silently dropped — and the second as *"heading ignored heading"*, announcing the very text the
//! author overrode. Both are names an agent then cannot match on, which is the whole purpose of
//! the string.
//!
//! ⚠⚠ **AND THE TWO WPT SUITES LOOK CONTRADICTORY ON `<img alt="" title="x">` AND ARE NOT.**
//! `html-aam` says it still has the `image` ROLE (a tooltip keeps it in the tree); `accname` says
//! its NAME is `""` (`alt=""` is the author saying the picture says nothing, and a tooltip does not
//! overrule that). Conflating role-presence with name-presence made one fix break the other.
//!
//! Measured: `accname` 380/484 → 395/484, with `wai-aria` and `html-aam` UNCHANGED and **zero**
//! newly-failing subtests (the failing-name lists were diffed, not just the totals).
//!
//! PROVEN RED under three mutations:
//!   N1  name-from-content back to `text_content`  -> "one three"
//!   N2  drop the descendant aria-name branch      -> "heading ignored link text heading"
//!   N3  drop the empty-alt title guard            -> an alt=""/title image is named "title"

use manuk_a11y::{accessible_name, role_of};

fn role_of_id(html: &str, id: &str) -> String {
    let dom = manuk_html::parse(html);
    let n = dom.get_element_by_id(dom.root(), id).expect("fixture id");
    role_of(&dom, n)
        .map(|r| r.as_str().to_string())
        .unwrap_or_default()
}

fn name_of(html: &str, id: &str) -> String {
    let dom = manuk_html::parse(html);
    let n = dom
        .get_element_by_id(dom.root(), id)
        .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"));
    let role = role_of(&dom, n).unwrap_or_else(|| panic!("VACUOUS: #{id} maps to no ARIA role"));
    accessible_name(&dom, n, &role)
}

#[test]
fn name_from_content_is_the_accname_walk_not_a_text_flatten() {
    // ── 1. A CHILD IMAGE CONTRIBUTES ITS ALT, IN PLACE.
    assert_eq!(
        name_of(
            r#"<button id="b"><span>one</span> <img alt="two" src="x.gif"> <span>three</span></button>"#,
            "b"
        ),
        "one two three",
        "an <img alt> in the middle of a button is part of its name. `text_content` drops it and \
         yields 'one three' — a name the agent cannot match the button on."
    );

    // ── 2. A DESCENDANT'S OWN ARIA NAME WINS, AND ITS SUBTREE IS NOT READ THROUGH.
    assert_eq!(
        name_of(
            r##"<h3 id="h">heading <a href="#" aria-label="link aria-label">ignored link text</a> heading</h3>"##,
            "h"
        ),
        "heading link aria-label heading",
        "overriding a name is the entire point of aria-label; reading through it announces the \
         text the author replaced"
    );
    assert_eq!(
        name_of(
            r#"<span id="s">Reviews</span><h3 id="h">a <em aria-labelledby="s">ignored</em> b</h3>"#,
            "h"
        ),
        "a Reviews b",
        "aria-labelledby on a DESCENDANT is dereferenced the same way it is on the subject"
    );

    // ── 3. A DECORATIVE CHILD IMAGE CONTRIBUTES NOTHING, AND WE DO NOT DESCEND INTO AN IMAGE.
    assert_eq!(
        name_of(
            r#"<button id="b"><span>one</span> <img alt="" src="x.gif"> <span>two</span></button>"#,
            "b"
        ),
        "one two",
        "alt=\"\" is a declaration, not a gap"
    );

    // ── 4. ⚠ ROLE-PRESENCE AND NAME-PRESENCE ARE DIFFERENT QUESTIONS. The two WPT suites only
    // look contradictory if you answer them in one place.
    assert_eq!(
        role_of_id(r#"<img id="i" alt="" title="tip" src="x.gif">"#, "i"),
        "image",
        "html-aam: a tooltip keeps the <img> IN THE TREE even with alt=\"\""
    );
    assert_eq!(
        name_of(r#"<img id="i" alt="" title="tip" src="x.gif">"#, "i"),
        "",
        "accname: …and its NAME is still empty. `title` is a LAST RESORT and is skipped when the \
         host language already supplied a label that came out empty ON PURPOSE."
    );
    assert_eq!(
        name_of(r#"<img id="i" title="tip" src="x.gif">"#, "i"),
        "tip",
        "CONTROL: with NO alt attribute at all there is no empty host-language label, so `title` \
         is the name"
    );

    // ── 5. ⚠ §4.3 STEP 2C OUTRANKS 2D: an embedded control speaks its VALUE, not its own name.
    // An ARIA-only combobox's value IS its content, and getting this wrong announces the control's
    // label where the sentence wants the number the user chose.
    assert_eq!(
        name_of(
            r#"<label><input type="checkbox" id="c"> Flash the screen
               <span role="combobox" aria-label="number of times">3</span> times</label>"#,
            "c"
        ),
        "Flash the screen 3 times",
        "step 2C (embedded control -> VALUE) is checked BEFORE step 2D (aria-label). Before the \
         descendant-aria-name branch existed this passed by ACCIDENT, because the walk fell \
         through to the text."
    );
    assert_eq!(
        name_of(
            r#"<label><input type="checkbox" id="c"> Flash the screen
               <span role="combobox" aria-label="number of times">3</span> times</label>
               <div id="probe"></div>"#,
            "c"
        ),
        "Flash the screen 3 times"
    );

    // ── 6. CONTROLS — the ordinary paths are untouched.
    assert_eq!(
        name_of(r#"<button id="b">Sign in</button>"#, "b"),
        "Sign in",
        "CONTROL: plain name from content"
    );
    assert_eq!(
        name_of(r#"<button id="b" aria-label="Close">X</button>"#, "b"),
        "Close",
        "CONTROL: the subject's own aria-label still outranks its content"
    );
    assert_eq!(
        name_of(
            r#"<button id="b">a <span hidden>skip</span> b</button>"#,
            "b"
        ),
        "a b",
        "CONTROL: a hidden descendant contributes nothing"
    );
    assert_eq!(
        name_of(r#"<div id="d">not a widget</div>"#, "d"),
        "",
        "CONTROL: name-from-content is still gated on the ROLE allowing it — a generic div has no \
         name, and making the walk recursive must not hand one to every box on the page"
    );
}
