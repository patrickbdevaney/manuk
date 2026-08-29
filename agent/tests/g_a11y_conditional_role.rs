//! **G_A11Y_CONDITIONAL_ROLE — A ROLE IS NOT A PROPERTY OF THE TAG ALONE.**
//!
//! After t1350 completed the role *vocabulary*, every remaining `html-aam` / `wai-aria` role
//! failure was one shape: the role depends on something OTHER than the element's own name —
//!
//! * **on an ANCESTOR** — an `<aside>` inside an `<article>` is that article's aside, not the
//!   page's; an orphaned `<li>` owns no list; a `<td>` under `<table role="none">` inherits the
//!   presentation.
//! * **on having a NAME** — `region`, `form` and `<section>` are LANDMARKS, and a landmark's whole
//!   purpose is to be an entry in a jump list. An unnamed one is a row that says nothing, so ARIA
//!   makes the role inoperative rather than let it dilute the list.
//! * **on a CONFLICTING presentational role** — `role="none"` is a request, and the spec makes it
//!   INOPERATIVE when the element is focusable or carries a global ARIA attribute, because a node
//!   the user can TAB to but that announces nothing is worse than one with a wrong name.
//! * **on the `<img>` three-way condition** — `alt=""` means decorative, an ARIA name OVERRIDES
//!   that, and an `<img>` with no source and no name is not a broken image but nothing at all.
//!
//! Measured: `html-aam` 281/335 → 310/335, `wai-aria` 387/434 → 399/434, `accname` unchanged.
//!
//! PROVEN RED under three mutations:
//!   N1  `img_role` back to `alt=="" -> None, else Image`   -> a sourceless img reads "image"
//!   N2  drop the `region`/`form` name condition            -> an unnamed pair reads "region"
//!   N3  `presentational_role_is_ignored` always false      -> a focusable role=none h1 reads "generic"

use manuk_a11y::role_of;

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
fn a_role_is_conditional_on_ancestor_name_and_conflicting_presentation() {
    // ── 1. THE `<img>` THREE-WAY CONDITION.
    assert_eq!(
        role_of_id(r#"<img id="i" alt="A cat" src="c.gif">"#, "i"),
        "image"
    );
    assert_eq!(
        role_of_id(r#"<img id="i" alt="" src="c.gif">"#, "i"),
        "",
        "an explicit empty alt is the author saying DECORATIVE — no node at all"
    );
    assert_eq!(
        role_of_id(r#"<img id="i" alt="   " src="c.gif">"#, "i"),
        "",
        "a whitespace-only alt is an empty alt"
    );
    assert_eq!(
        role_of_id(r#"<img id="i" alt="" aria-label="A cat" src="c.gif">"#, "i"),
        "image",
        "an ARIA name OVERRIDES the empty alt: the author said 'not this text', not 'nothing'"
    );
    assert_eq!(
        role_of_id(r#"<img id="i" src="c.gif">"#, "i"),
        "image",
        "no alt at all but a source: a broken image is still an image"
    );
    assert_eq!(
        role_of_id(r#"<img id="i">"#, "i"),
        "",
        "⭐ no alt, NO SOURCE and no name is not 'an image that failed to load' — it is nothing, \
         and announcing it puts a phantom in the tree on every page shipping an empty placeholder"
    );

    // ── 2. AN ANCESTOR DECIDES: <aside>.
    assert_eq!(
        role_of_id(r#"<body><aside id="a">x</aside></body>"#, "a"),
        "complementary",
        "CONTROL: a top-level aside is the page's complementary landmark"
    );
    assert_eq!(
        role_of_id(r#"<main><aside id="a">x</aside></main>"#, "a"),
        "complementary",
        "⚠ `<main>` does NOT scope an aside, though it DOES scope a <header>/<footer>. The two \
         lists differ element by element, and sharing one would silently break the other."
    );
    assert_eq!(
        role_of_id(r#"<article><aside id="a">x</aside></article>"#, "a"),
        "generic",
        "an aside inside sectioning content is that section's aside, not the page's"
    );
    assert_eq!(
        role_of_id(
            r#"<article><aside id="a" aria-label="Related">x</aside></article>"#,
            "a"
        ),
        "complementary",
        "…unless it carries a NAME that distinguishes it in the landmark list"
    );

    // ── 3. THE NAME MUST RESOLVE. A dangling `aria-labelledby` is not a name, and trusting its
    // presence made a `<section>` a landmark that announces nothing.
    assert_eq!(
        role_of_id(r#"<section id="s" aria-labelledby="typo">x</section>"#, "s"),
        "generic",
        "an aria-labelledby that resolves to no element is not an accessible name"
    );
    assert_eq!(
        role_of_id(
            r#"<span id="n">Reviews</span><section id="s" aria-labelledby="n">x</section>"#,
            "s"
        ),
        "region",
        "CONTROL: a labelledby that DOES resolve makes the section a region"
    );
    assert_eq!(
        role_of_id(r#"<section id="s" title="Reviews">x</section>"#, "s"),
        "region",
        "`title` is a name source too"
    );

    // ── 4. AN UNNAMED `region`/`form` FALLS THROUGH TO THE NEXT TOKEN — which is exactly why
    // authors write the pair `role="region group"`.
    assert_eq!(
        role_of_id(r#"<div id="r" role="region group">x</div>"#, "r"),
        "group",
        "an unnamed region is inoperative, so the fallback token takes effect"
    );
    assert_eq!(
        role_of_id(
            r#"<div id="r" role="region group" aria-label="Cart">x</div>"#,
            "r"
        ),
        "region",
        "CONTROL: name it and the landmark is real again"
    );
    assert_eq!(
        role_of_id(r#"<div id="r" role="form navigation">x</div>"#, "r"),
        "navigation",
        "`form` is name-conditional for the same reason `region` is"
    );

    // ── 5. PRESENTATIONAL-ROLE CONFLICT RESOLUTION.
    assert_eq!(
        role_of_id(r#"<h1 id="h" role="none">x</h1>"#, "h"),
        "generic",
        "CONTROL: an honoured role=none does suppress the heading semantics"
    );
    assert_eq!(
        role_of_id(r#"<h1 id="h" role="none" tabindex="0">x</h1>"#, "h"),
        "heading",
        "⭐ a node the user can TAB to must not announce nothing — role=none is INOPERATIVE on a \
         focusable element"
    );
    assert_eq!(
        role_of_id(r#"<h1 id="h" role="none" aria-label="x">x</h1>"#, "h"),
        "heading",
        "a global ARIA attribute is the author proving they meant the element to be in the tree"
    );

    // ── 6. REQUIRED OWNED ELEMENTS INHERIT THE PRESENTATION — which is the only reason
    // `role="none"` on a layout table or list is useful at all.
    assert_eq!(
        role_of_id(
            r#"<table role="none"><tr><td id="c">x</td></tr></table>"#,
            "c"
        ),
        "",
        "a cell under a presentational table must not survive as a skeleton `cell` node"
    );
    assert_eq!(
        role_of_id(
            r#"<table role="none"><tr><td id="c" aria-describedby="x">x</td></tr></table>"#,
            "c"
        ),
        "",
        "⚠ and a global attribute on the CHILD does not rescue it — the conflict-resolution \
         exception applies to the element that carries role=none, not to what it owns"
    );
    assert_eq!(
        role_of_id(r#"<table><tr><td id="c">x</td></tr></table>"#, "c"),
        "cell",
        "CONTROL: an ordinary table still has cells"
    );
    assert_eq!(
        role_of_id(r#"<ul role="none"><li id="l">x</li></ul>"#, "l"),
        "",
        "same inheritance for a presentational list"
    );

    // ── 7. AN ORPHANED `<li>` OWNS NO LIST.
    assert_eq!(
        role_of_id(r#"<div><li id="l">x</li></div>"#, "l"),
        "generic",
        "`listitem` is defined by its owning list; announcing 'list item, 1 of 1' about a stray \
         <li> states a fact the page does not contain"
    );
    assert_eq!(
        role_of_id(r#"<ul><li id="l">x</li></ul>"#, "l"),
        "listitem",
        "CONTROL: an <li> in a list is still a listitem"
    );
}
