//! **G_AX_LANDMARK_ROLES — the landmark map, swept against Chrome, and the one row that was wrong.**
//!
//! Surface audit #80's ranked #1, fourth pass. Twenty-four rows measured against CDP
//! `Accessibility.getFullAXTree`; **twenty-three were already correct** and are banked here, and the
//! twenty-fourth is the finding.
//!
//! ## ⭐⭐⭐ `<form>` IS A LANDMARK ONLY WHEN NAMED, AND THE RULE WAS WRITTEN DOWN NEXT DOOR
//!
//! ```text
//!                                        chrome     before    after
//!   <form>                       plain   generic    form      generic
//!   <form aria-label="FL">               form       form      form
//!   <form title="FT">                    form       form      form
//!   <form aria-labelledby=…>             form       form      form
//!   <form name="n">              CONTROL generic    form      generic
//!   <div role=form>              CONTROL generic    generic   generic
//!   <div role=form aria-label>   CONTROL form       form      form
//!   <section>                    CONTROL generic    generic   generic
//! ```
//!
//! The `<section>` arm three lines below the `<form>` arm carries the identical clause, and
//! `role_of`'s **explicit-role path** in the same function carries it for exactly these two roles:
//! `matches!(r, Role::Region | Role::Form) && !has_attribute_name(…)`.
//!
//! > **The same rule, guarded at one entrance of one function and unguarded at the other** — and
//! > the guarded entrance is `role="form"`, which almost nobody writes, while the unguarded one is
//! > `<form>`, which is on nearly every page.
//!
//! ⭐ **Why it matters to an agent and not only to a spec: a landmark list is a JUMP LIST.** Every
//! `<form>` on a page — the newsletter box, the search field, the login — appeared in it, so
//! *"go to the form"* was ambiguous exactly when there is more than one, which is the case the list
//! exists for. t1375 made the drive path REFUSE an ambiguous target, so this converted into a
//! refusal rather than a wrong click — the same conversion t1380's phantom menu link made.
//!
//! ⭐ **`name="n"` is the row that stops "has any nameish attribute" from being the rule.** A form's
//! `name` is its SUBMISSION name, not an accessible one; Chrome reports `generic`.
//!
//! ## THE TWENTY-THREE THAT WERE ALREADY RIGHT, AND ARE NOW BANKED
//!
//! ```text
//!   <section>  plain / aria-label / title / aria-labelledby   generic / region ×3
//!   <nav> <main> <aside>                    navigation / main / complementary
//!   <header> <footer> at top level          banner / contentinfo
//!   <header> inside a <div>                 banner        (a div is not sectioning content)
//!   <header> inside an <article>            sectionheader (scoped — NOT a landmark)
//!   <div role=region> unnamed / named       generic / region
//! ```
//!
//! ⭐⭐ **The `<header>` pair is the sharpest of them.** A `<header>` inside a `<div>` is still the
//! page's `banner`, because a `<div>` is not sectioning content; the same element inside an
//! `<article>` is a scoped `sectionheader` and must NOT appear in the landmark list. Getting that
//! backwards either hides the page banner or puts every card's header into the jump list. It was
//! already right and had no gate.
//!
//! ## ⚠⚠ AND A LATENT WRONG ANSWER THAT t1384 MADE VISIBLE
//!
//! ```text
//!   <fieldset disabled>          chrome: role=group, NO `disabled` property
//!     <input type=checkbox>      chrome: disabled: True
//! ```
//!
//! The native `disabled` attribute belongs to the *listed form elements*; `<fieldset>` carries it as
//! a PROPAGATOR and is not itself disabled. Ours reported it on the fieldset too — and **as a
//! nameless `generic` that node was never printed in the observation lines, so the wrong state could
//! not be seen.** t1384 promoted `<fieldset>` to `group`, which is correct, and the promotion
//! PUBLISHED the wrong state: `g_disabled_inert` (which counts `disabled` lines) went red.
//!
//! > **A latent wrong answer surfaces when the node it lives on becomes visible**, so a correctness
//! > fix can look like the thing that broke a gate when it is the thing that exposed it.
//!
//! ⚠ `aria-disabled` is NOT scoped this way and must not be: `<div role=button aria-disabled=true>`
//! reports `disabled` in Chrome on any element, because the author said so explicitly. Only the
//! NATIVE attribute belongs to controls, and the two rows here are what separate them.
//!
//! ⚠ This is the shape t1377 named: **most of a swept surface is usually correct, and the value of
//! the sweep is the one row plus the banking of the rest.** A sweep that reports only its finding
//! leaves twenty-three behaviours as unguarded as it found them.

use manuk_a11y::{role_of, Role};

const HTML: &str = r##"<!doctype html><html><head></head><body>
<form id="f_plain">plain</form>
<form id="f_label" aria-label="FL">x</form>
<form id="f_title" title="FT">x</form>
<form id="f_by" aria-labelledby="fl">x</form><span id="fl">FB</span>
<form id="f_name" name="n">x</form>
<div id="f_role" role="form">x</div>
<div id="f_role_named" role="form" aria-label="RF">x</div>
<section id="s_plain">plain</section>
<section id="s_label" aria-label="AL">named</section>
<section id="s_title" title="T">t</section>
<section id="s_by" aria-labelledby="sl">x</section><span id="sl">LB</span>
<nav id="l_nav">n</nav>
<aside id="l_aside">a</aside>
<main id="l_main">m</main>
<header id="l_header">h</header>
<footer id="l_footer">f</footer>
<div id="l_div"><header id="l_div_header">inner</header></div>
<article id="l_article"><header id="l_art_header">inner</header></article>
<div id="r_plain" role="region">unnamed</div>
<div id="r_named" role="region" aria-label="R">named</div>
<fieldset id="d_fs" disabled><input id="d_in" type="checkbox"><legend>L</legend></fieldset>
<div id="d_aria" role="button" aria-disabled="true">D</div>
</body></html>"##;

#[test]
fn a_landmark_is_a_landmark_only_when_the_spec_says_so() {
    let fonts = manuk_text::FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://land.test/", &fonts, 1200.0);
    let dom = page.dom();
    let tree = page.a11y_tree();
    let node = |id: &str| {
        dom.get_element_by_id(dom.root(), id)
            .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"))
    };

    // ── VACUITY. The named forms must really have a resolvable name, or every "named ⇒ landmark"
    //    row below would be satisfied by an implementation that never promotes anything.
    for id in ["f_label", "f_title", "f_by"] {
        let n = node(id);
        let role = role_of(dom, n).expect("a role");
        let styles = manuk_a11y::name_styles(dom, page.styles_map());
        let g = manuk_layout::generated_text(dom, page.styles_map());
        let a = manuk_layout::generated_alt_text(dom, page.styles_map());
        let name = manuk_a11y::accessible_name_generated(
            dom,
            n,
            &role,
            &manuk_a11y::empty_name_ctx(&g, &a, &styles),
        );
        assert!(
            !name.is_empty(),
            "VACUOUS: #{id} has no accessible name, so its promotion row proves nothing"
        );
    }

    // (id, the role Chrome's AX tree reports, what the row decides)
    let rows: &[(&str, Role, &str)] = &[
        ("f_plain", Role::Generic, "THE DEFECT — an unnamed `<form>` is NOT a landmark. Every form on the page was in the jump list, so \"go to the form\" was ambiguous exactly when it mattered"),
        ("f_label", Role::Form, "…and a NAMED one is: `aria-label` promotes it"),
        ("f_title", Role::Form, "…so does `title`"),
        ("f_by", Role::Form, "…and so does a RESOLVING `aria-labelledby` (the reference must produce text, not merely exist)"),
        ("f_name", Role::Generic, "CONTROL — `name=\"n\"` is the form's SUBMISSION name, not an accessible one. This is the row that stops \"has any nameish attribute\" from being the rule"),
        ("f_role", Role::Generic, "CONTROL — the EXPLICIT `role=form` path already had this guard, which is what makes the defect \"one entrance of one function\""),
        ("f_role_named", Role::Form, "CONTROL — and it promotes when named, exactly as the implicit path now does"),
        ("s_plain", Role::Generic, "CONTROL — `<section>` already carried the identical clause three lines away"),
        ("s_label", Role::Region, "CONTROL — a named section is a region"),
        ("s_title", Role::Region, "CONTROL — `title` names a section too"),
        ("s_by", Role::Region, "CONTROL — and `aria-labelledby`"),
        ("l_nav", Role::Navigation, "CONTROL — an unconditional landmark, banked"),
        ("l_aside", Role::Complementary, "CONTROL"),
        ("l_main", Role::Main, "CONTROL"),
        ("l_header", Role::Banner, "CONTROL — a top-level `<header>` is the page banner"),
        ("l_footer", Role::ContentInfo, "CONTROL"),
        ("l_div_header", Role::Banner, "⭐ a `<header>` inside a `<div>` is STILL the page banner — a `<div>` is not sectioning content"),
        ("l_art_header", Role::SectionHeader, "⭐ …and the same element inside an `<article>` is a SCOPED header, which must not appear in the landmark list. Backwards, this either hides the page banner or puts every card's header in the jump list"),
        ("r_plain", Role::Generic, "CONTROL — an unnamed `role=region` is inoperative, the sibling rule to the form one"),
        ("r_named", Role::Region, "CONTROL"),
    ];
    for (id, want, why) in rows {
        let got = role_of(dom, node(id));
        assert_eq!(
            got.as_ref(),
            Some(want),
            "G_AX_LANDMARK_ROLES #{id}: Chrome's AX tree says {:?}, got {:?}.\n  {why}",
            want.as_str(),
            got.as_ref().map(Role::as_str)
        );
    }

    // ── ⚠⚠⚠ **A `<fieldset disabled>` IS NOT ITSELF DISABLED, AND GIVING IT A ROLE IS WHAT MADE
    //    THAT VISIBLE.** t1384 promoted `<fieldset>` from `generic` to `group`; as a nameless
    //    generic the node was not printed in the observation lines at all, so a `disabled` it
    //    should never have carried could not be seen. Chrome-measured: the fieldset is a `group`
    //    with **no `disabled` property**, and only the controls inside it report `disabled`.
    //
    //    ⚠ `aria-disabled` is deliberately NOT scoped this way — `<div role=button
    //    aria-disabled=true>` reports `disabled` in Chrome on any element, because the author said
    //    so. Only the NATIVE attribute belongs to the listed form elements, and that pair of rows
    //    is what separates the two.
    let state = |id: &str| {
        let n = node(id);
        let r = role_of(dom, n).expect("a role");
        manuk_a11y::state_of(dom, n, &r)
    };
    assert!(
        !state("d_fs").disabled,
        "G_AX_LANDMARK_ROLES #d_fs: Chrome reports NO `disabled` on a `<fieldset disabled>` — it          propagates the state to its controls and does not carry it"
    );
    assert!(
        state("d_in").disabled,
        "G_AX_LANDMARK_ROLES #d_in: …and the control inside it DOES, which is the half that must          not be lost while scoping the other"
    );
    assert!(
        state("d_aria").disabled,
        "G_AX_LANDMARK_ROLES #d_aria: `aria-disabled` is NOT scoped to form controls — an author          who writes it on a `<div role=button>` means it, and Chrome reports it"
    );

    // ── AND THROUGH THE TREE. The landmark list is what an agent walks, so the promotion has to be
    //    visible there and not only in `role_of`.
    let forms = tree
        .iter()
        .filter(|n| n.role == Role::Form)
        .map(|n| n.name.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        forms,
        vec!["FL", "FT", "FB", "RF"],
        "G_AX_LANDMARK_ROLES (AX TREE): exactly the four NAMED forms are landmarks — the three
         `<form>`s that carry a name and the explicit `role=form` that does. An UNNAMED `<form>` in
         this list is the jump-list pollution the fix is about, and it is asserted as the NAME SET
         rather than a count so an extra entry says which one it is."
    );
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  restore `"form" => Role::Form` (the pre-tick arm)
//       -> `f_plain` and `f_name` fail, and the tree row reports six form landmarks instead of
//          four (the two unnamed `<form>`s join it). Every named
//          row stays green, which is what says the defect was the GUARD and not the mapping.
// N2  guard `<form>` on `attr("aria-label").is_some()` alone
//       -> `f_title` and `f_by` fail: three spellings name a landmark, not one.
// N3b restore `inherits_disabled` to walk from ANY element (the pre-tick behaviour)
//       -> `d_fs` fails: the fieldset reports a `disabled` it does not have. `d_in` and `d_aria`
//          stay green, which is what says the scope is the fix and not the removal.
// N3  swap the `<header>` scoping list so a `<div>` counts as sectioning content
//       -> only `l_div_header` fails, at `sectionheader`. That row and `l_art_header` are the pair
//          that decides the rule, and each is inert without the other.
