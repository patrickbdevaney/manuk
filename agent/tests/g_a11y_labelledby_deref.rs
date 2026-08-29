//! **G_A11Y_LABELLEDBY_DEREF — A REFERENCED NODE CONTRIBUTES ITS NAME, NOT ITS TEXT.**
//!
//! `aria-labelledby` was dereferenced with `dom.text_content()` — the t1353 defect one level
//! further out — and it failed in four separate ways:
//!
//! ```html
//!   <button aria-labelledby="cb">Toggle</button>
//!   <input type="checkbox" id="cb"><label for="cb">Checkbox Label Text</label>
//! ```
//!
//! A referenced **CONTROL** has no text at all, so the button was named *"Toggle"* — its own
//! content — instead of *"Checkbox Label Text"*. A referenced node's own `aria-label` was ignored;
//! a hidden fragment nested inside it was included; and a `<span id=x hidden>` pointed at
//! deliberately contributed nothing.
//!
//! ⚠⚠⚠ **THE HIDDEN-NODE EXEMPTION IS THE SUBTREE, NOT THE ONE NODE.** accname §4.3 step 2A
//! exempts a node *directly referenced* by `aria-labelledby` from the hidden check — and if that
//! element is `display:none`, its children are hidden **because it is**. Nothing can tell a child's
//! own `display:none` from the one it inherits, so pruning inside would make the reference
//! contribute nothing and defeat the exemption entirely. The counter-case is a VISIBLE reference
//! with a hidden fragment in it, which contributes nothing and falls back.
//!
//! ⚠ `display:none` is read from the element's own **inline `style`**, which is a bounded and named
//! approximation: `accessible_name` is handed a DOM and no computed styles, so a `display:none`
//! applied by a CLASS is still missed. Stated here rather than discovered later.
//!
//! Measured: `accname` 395/484 → 411/484, `wai-aria` and `html-aam` UNCHANGED, **zero**
//! newly-failing subtests (failing-NAME lists diffed, not totals).
//!
//! PROVEN RED under three mutations:
//!   N1  dereference with `text_content` again      -> the button is named "Toggle"
//!   N2  `exempt_hidden` forced false               -> a hidden reference contributes nothing
//!   N3  `exempt_hidden` forced true                -> a visible reference stops falling back

use manuk_a11y::{accessible_name, role_of};

fn name_of(html: &str, id: &str) -> String {
    let dom = manuk_html::parse(html);
    let n = dom
        .get_element_by_id(dom.root(), id)
        .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"));
    let role = role_of(&dom, n).unwrap_or_else(|| panic!("VACUOUS: #{id} maps to no ARIA role"));
    accessible_name(&dom, n, &role)
}

#[test]
fn aria_labelledby_dereferences_to_a_name_not_to_text_content() {
    // ── 1. THE REFERENCED NODE IS A CONTROL, SO IT HAS NO TEXT AT ALL.
    assert_eq!(
        name_of(
            r#"<button id="b" aria-labelledby="cb">Toggle</button>
               <input type="checkbox" id="cb">
               <label for="cb">Checkbox Label Text</label>"#,
            "b"
        ),
        "Checkbox Label Text",
        "a referenced CONTROL contributes its own accessible NAME — which comes from its <label>. \
         `text_content` on an <input> is the empty string, so the button fell back to its own \
         content and announced 'Toggle'."
    );
    assert_eq!(
        name_of(
            r#"<button id="b" aria-labelledby="cb">Toggle</button>
               <input type="checkbox" id="cb" aria-label="Checkbox ARIA Label">"#,
            "b"
        ),
        "Checkbox ARIA Label",
        "…and the referenced node's own aria-label counts too"
    );

    // ── 2. ⭐ ONE HOP ONLY — AND `aria-labelledby` POINTING AT ITSELF IS A REAL AUTHORING PATTERN
    // ("my own label, then that heading"). Without the guard this is an infinite regress.
    assert_eq!(
        name_of(
            r#"<div role="group" id="g" aria-label="self label" aria-labelledby="g h">x</div>
               <h2 id="h">first heading</h2>"#,
            "g"
        ),
        "self label first heading",
        "the referenced computation must NOT re-enter aria-labelledby: the self-reference resolves \
         through the element's own aria-label and stops"
    );

    // ── 3. ⭐⭐⭐ A HIDDEN NODE POINTED AT DELIBERATELY CONTRIBUTES ITS WHOLE SUBTREE.
    assert_eq!(
        name_of(
            r#"<button id="b" aria-labelledby="s1" aria-label="foo">x</button>
               <span id="s1" style="display:none;"><span style="display:none;">label</span></span>"#,
            "b"
        ),
        "label",
        "a `<span hidden>` that exists only to be pointed at is how a long name is attached to an \
         icon button. If the reference is display:none its children are hidden BECAUSE it is, so \
         pruning inside would defeat the exemption and the reference would contribute nothing."
    );
    assert_eq!(
        name_of(
            r#"<button id="b" aria-labelledby="s1" aria-label="foo">x</button>
               <span id="s1" hidden>Delete permanently</span>"#,
            "b"
        ),
        "Delete permanently",
        "the HTML `hidden` attribute is the same case"
    );

    // ── 4. …AND THE COUNTER-CASE: A VISIBLE REFERENCE WITH A HIDDEN FRAGMENT INSIDE IT
    // CONTRIBUTES NOTHING, AND THE SUBJECT FALLS BACK TO ITS OWN `aria-label`.
    assert_eq!(
        name_of(
            r#"<button id="b" aria-labelledby="s5" aria-label="foo">x</button>
               <span id="s5"><span style="visibility:hidden;">label</span></span>"#,
            "b"
        ),
        "foo",
        "⚠ THE PAIR THAT PINS THE RULE: the reference here is VISIBLE, so its hidden child really \
         is separately hidden and contributes nothing — and an empty labelledby result falls \
         through to aria-label rather than winning as an empty name."
    );
    assert_eq!(
        name_of(
            r#"<button id="b" aria-labelledby="s">x</button>
               <span id="s">foo <span style="display:none;">bar <span>baz</span></span></span>"#,
            "b"
        ),
        "foo",
        "a hidden fragment inside a VISIBLE reference is excluded — 'foo bar baz' is text the user \
         cannot see being announced as the button's name"
    );

    // ── 5. `visibility` IS THE ONE HIDING MECHANISM A DESCENDANT CAN UNDO.
    assert_eq!(
        name_of(
            r#"<button id="b" aria-labelledby="s">x</button>
               <span id="s">a <span style="visibility:hidden;">b <span style="visibility:visible;">c</span></span></span>"#,
            "b"
        ),
        "a c",
        "visibility:visible inside a visibility:hidden ancestor IS shown, so the walk must carry \
         the flag down rather than prune — `display:none` prunes, `visibility` does not"
    );

    // ── 6. CONTROLS — the ordinary paths are untouched.
    assert_eq!(
        name_of(
            r#"<button id="b" aria-labelledby="s">x</button><span id="s">plain label</span>"#,
            "b"
        ),
        "plain label",
        "CONTROL: the ordinary visible reference"
    );
    assert_eq!(
        name_of(
            r#"<button id="b" aria-labelledby="a b2">x</button>
               <span id="a">one</span><span id="b2">two</span>"#,
            "b"
        ),
        "one two",
        "CONTROL: multiple references still concatenate in the order written"
    );
    assert_eq!(
        name_of(
            r#"<button id="b" aria-labelledby="nope" aria-label="fallback">x</button>"#,
            "b"
        ),
        "fallback",
        "CONTROL: a reference that resolves to nothing falls through to aria-label"
    );
    assert_eq!(
        name_of(r#"<button id="b" aria-label="Close">X</button>"#, "b"),
        "Close",
        "CONTROL: no labelledby at all — aria-label still wins over content"
    );
}
