//! **G_A11Y_ROLE_VOCABULARY — AN ARIA ROLE TOKEN IS ASCII CASE-INSENSITIVE, AND HALF THE
//! VOCABULARY WAS MISSING.**
//!
//! Three defects in one subsystem — `manuk_a11y::Role` — measured on WPT's `wai-aria` and
//! `html-aam` suites, which is what all four engines score themselves on:
//!
//! 1. ⭐ **THE CASE FOLD EXISTED AND THE WEB'S ENTRANCE DID NOT USE IT.** `Role::parse` is
//!    `from_aria_token(&tok.trim().to_ascii_lowercase())` and is what the AGENT calls. `role_of` —
//!    the path a real `role="…"` attribute takes — called the raw matcher, so `role="BUTTON"` and
//!    `role="foo Link"` matched nothing and fell through to the implicit role. **114 of 172
//!    failing `wai-aria` subtests were nothing but this.** (Two entrances, one guarded: t1353.)
//! 2. **~30 ROLE TOKENS WERE ABSENT** and resolved to `generic` — `code`, `time`, `term`,
//!    `deletion`, `insertion`, `emphasis`, `strong`, `figure`, `meter`, `grid`, `rowgroup`,
//!    `searchbox`, `log`, `timer`, `math`, … A tree that answers "a box" about a word the author
//!    marked up on purpose cannot be the agent's perception layer.
//! 3. **TWO ROLES WERE COLLAPSED ONTO NEIGHBOURS** — `gridcell`→`cell`,
//!    `menuitemcheckbox`/`menuitemradio`→`menuitem`. A collapse is invisible in a tree dump and
//!    wrong in the one place the role is read: `menuitemcheckbox` IS the announcement that the
//!    item carries a state.
//!
//! Plus the HTML side of the same vocabulary, including the one that is a real-page bug rather
//! than a conformance point: **a `<footer>` inside an `<article>` is not the page's footer.** A
//! blog index with thirty articles was publishing thirty `contentinfo` LANDMARKS, so the one real
//! page footer stopped being findable.
//!
//! PROVEN RED under three mutations:
//!   N1  role_of back to `find_map(Role::from_aria_token)`  -> role="BUTTON" reads "generic"
//!   N2  drop the `menuitemcheckbox`/`gridcell` arms         -> they read "menuitem" / "cell"
//!   N3  make `in_sectioning_content` always false           -> a nav's footer reads "contentinfo"

use manuk_a11y::{role_of, Role};

/// The computed role string the engine publishes for `#id` — the same value
/// `test_driver.get_computed_role()` reads, and the same one the agent targets by.
fn role_of_id(html: &str, id: &str) -> String {
    let dom = manuk_html::parse(html);
    let n = dom
        .get_element_by_id(dom.root(), id)
        .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"));
    role_of(&dom, n)
        .map(|r| r.as_str().to_string())
        .unwrap_or_default()
}

#[test]
fn an_aria_role_token_is_case_insensitive_and_the_vocabulary_is_complete() {
    // ── 1. THE CASE FOLD. Every one of these is the SAME role token.
    for spelling in ["button", "BUTTON", "Button", "buTtOn"] {
        assert_eq!(
            role_of_id(&format!(r#"<div id="b" role="{spelling}">x</div>"#), "b"),
            "button",
            "an ARIA role token is ASCII case-insensitive (ARIA in HTML), and `role={spelling:?}` \
             fell through to the implicit role. `Role::parse` already folded case — the `role=` \
             attribute, the entrance the WEB uses, called the raw matcher instead."
        );
    }

    // ── 2. THE FALLBACK-TOKEN FORM, which is how authors ship forward-compatible roles: an
    // unknown token, then a real one. The first VALID token wins — and validity is case-blind.
    assert_eq!(
        role_of_id(r#"<span id="l" role="foo Link" tabindex="0">x</span>"#, "l"),
        "link",
        "the first VALID token wins, so an unknown token must not stop the scan — and `Link` is \
         valid. This is the exact shape of 114 failing wai-aria subtests."
    );

    // ── 3. THE ABSENT VOCABULARY. Each of these read `generic` before.
    for (tok, want) in [
        ("code", "code"),
        ("time", "time"),
        ("term", "term"),
        ("definition", "definition"),
        ("deletion", "deletion"),
        ("insertion", "insertion"),
        ("emphasis", "emphasis"),
        ("strong", "strong"),
        ("subscript", "subscript"),
        ("superscript", "superscript"),
        ("blockquote", "blockquote"),
        ("caption", "caption"),
        ("figure", "figure"),
        ("meter", "meter"),
        ("grid", "grid"),
        ("rowgroup", "rowgroup"),
        ("searchbox", "searchbox"),
        ("scrollbar", "scrollbar"),
        ("log", "log"),
        ("timer", "timer"),
        ("marquee", "marquee"),
        ("math", "math"),
        ("note", "note"),
        ("application", "application"),
        ("suggestion", "suggestion"),
        ("mark", "mark"),
        ("sectionheader", "sectionheader"),
        ("sectionfooter", "sectionfooter"),
    ] {
        assert_eq!(
            role_of_id(&format!(r#"<div id="r" role="{tok}">x</div>"#), "r"),
            want,
            "role={tok:?} must resolve to itself, not to `generic`"
        );
    }

    // ── 4. THE UN-COLLAPSED ROLES. A collapse reads as a plausible neighbour, which is exactly
    // why it survives review.
    assert_eq!(
        role_of_id(r#"<div id="g" role="gridcell">x</div>"#, "g"),
        "gridcell",
        "a gridcell is not a cell: a grid is the interactive widget, a table is static content"
    );
    for tok in ["menuitemcheckbox", "menuitemradio"] {
        assert_eq!(
            role_of_id(&format!(r#"<div id="m" role="{tok}">x</div>"#), "m"),
            tok,
            "{tok} IS the announcement that the item carries a state — grounding it as `menuitem` \
             deletes the only thing that distinguishes it"
        );
    }

    // ── 5. ⚠ AND UN-COLLAPSING MUST NOT BREAK THE CALLER THAT ASKED THE COARSE QUESTION.
    // `manuk-agent` targets by role NAME; `Role::matches` keeps the old query working.
    assert!(
        Role::parse("menuitem")
            .unwrap()
            .matches(&Role::parse("menuitemcheckbox").unwrap()),
        "an agent asking for a `menuitem` must still match a `menuitemcheckbox` — the tree got \
         more precise, and a precision gain that silently drops a match is a REGRESSION"
    );
    assert!(
        Role::parse("cell")
            .unwrap()
            .matches(&Role::parse("gridcell").unwrap()),
        "likewise `cell` must still match a `gridcell`"
    );

    // ── 6. THE HTML SIDE OF THE SAME VOCABULARY.
    for (markup, want) in [
        (r#"<em id="e">x</em>"#, "emphasis"),
        (r#"<strong id="e">x</strong>"#, "strong"),
        (r#"<code id="e">x</code>"#, "code"),
        (r#"<sub id="e">x</sub>"#, "subscript"),
        (r#"<sup id="e">x</sup>"#, "superscript"),
        (r#"<mark id="e">x</mark>"#, "mark"),
        (r#"<time id="e">x</time>"#, "time"),
        (r#"<del id="e">x</del>"#, "deletion"),
        (r#"<s id="e">x</s>"#, "deletion"),
        (r#"<ins id="e">x</ins>"#, "insertion"),
        (r#"<blockquote id="e">x</blockquote>"#, "blockquote"),
        (r#"<figure id="e">x</figure>"#, "figure"),
        (r#"<meter id="e">x</meter>"#, "meter"),
        (r#"<dfn id="e">x</dfn>"#, "term"),
        (r#"<output id="e">x</output>"#, "status"),
        (
            r#"<select><optgroup id="e" label="x"></optgroup></select>"#,
            "group",
        ),
        (
            r#"<table><tbody id="e"><tr><td>x</td></tr></tbody></table>"#,
            "rowgroup",
        ),
        (r#"<input id="e" type="search">"#, "searchbox"),
    ] {
        assert_eq!(
            role_of_id(markup, "e"),
            want,
            "HTML-AAM gives this element a role, and it was reading `generic`: {markup}"
        );
    }

    // ── 7. ⭐ A `<footer>` INSIDE SECTIONING CONTENT IS NOT THE PAGE'S FOOTER (ARIA 1.3).
    // This is the arm that is a real-page bug rather than a conformance point.
    assert_eq!(
        role_of_id(r#"<body><footer id="f">x</footer></body>"#, "f"),
        "contentinfo",
        "CONTROL: the page's own footer is still the `contentinfo` LANDMARK"
    );
    assert_eq!(
        role_of_id(r#"<nav><footer id="f">x</footer></nav>"#, "f"),
        "sectionfooter",
        "a footer scoped to a section is `sectionfooter`. A blog index with thirty articles was \
         publishing thirty `contentinfo` landmarks — which is worse than none, because the one \
         real page footer stops being findable in the landmark list."
    );
    assert_eq!(
        role_of_id(r#"<body><header id="h">x</header></body>"#, "h"),
        "banner",
        "CONTROL: the page's own header is still the `banner` landmark"
    );
    assert_eq!(
        role_of_id(r#"<article><header id="h">x</header></article>"#, "h"),
        "sectionheader"
    );

    // ── 8. CONTROL — an INVALID token is still ignored and the implicit role still wins, which is
    // what makes the fallback form work at all.
    assert_eq!(
        role_of_id(r#"<button id="c" role="notarole">x</button>"#, "c"),
        "button",
        "CONTROL: an unknown role token falls through to the implicit role, it does not blank it"
    );
    // ── CONTROL — a plain <div> is still honestly `generic`.
    assert_eq!(
        role_of_id(r#"<div id="c">x</div>"#, "c"),
        "generic",
        "CONTROL: `generic` is still the honest answer for an element with no semantics"
    );
}
