//! **G_AX_ROLE_HTML_AAM — the implicit roles that were falling through to a plausible default.**
//!
//! Surface audit #80's ranked #1: *"use CDP `Accessibility.getFullAXTree` as the a11y oracle
//! deliberately"* — because Interop 2026 lists accessibility testing as an INVESTIGATION effort,
//! i.e. the four vendors' own position is that no suite can decide a11y-tree correctness yet. This
//! is that sweep run against `role_of`, and it found seven rows.
//!
//! ## THE MECHANISM — a default that answers plausibly instead of correctly
//!
//! `role_of`'s `<input>` dispatch ended `_ => Role::TextBox`, and its element dispatch ends
//! `_ => Role::Generic`. Both are reasonable-looking fallbacks and both produce an answer an agent
//! will ACT on. **The role is how the agent addresses a control**, so a wrong role is not a wrong
//! label — it is a wrong plan.
//!
//! ```text
//!                            chrome (CDP)          before         after
//!   <input type=file>        button                textbox        button
//!   <input type=color>       ColorWell (internal)  textbox        generic
//!   <select multiple>        listbox               combobox       listbox
//!   <select size=4>          listbox               combobox       listbox
//!   <fieldset>               group                 generic        group
//!   <details>                group                 generic        group
//!   <address>                group                 generic        group
//!   <hgroup>                 group                 generic        group
//! ```
//!
//! ⭐⭐⭐ **`<input type=file>` IS THE ROW WITH TEETH.** As a `textbox` an upload control is
//! invisible to *"click Choose File"* — and, much worse, `type_into` ACCEPTS it and silently does
//! nothing, because a file input has no text to type into. **A wrong role that an actuator will act
//! on is a lie the actuator cannot detect**, which is the same family as t1380's phantom menu link:
//! the perception layer hands the driver a plan that cannot execute.
//!
//! ⭐⭐ **`<select multiple>` and `<select size=4>` are two different widgets, not one.** A combobox
//! is opened and one option chosen; a listbox is a visible list whose selection may be plural. HTML
//! -AAM makes `multiple` OR `size > 1` the discriminator, and both spellings are asserted because
//! either alone would pass against an implementation that only read the other.
//!
//! ⭐ **`<fieldset>` is the row with corpus weight** — every multi-section form on the web is built
//! out of it, and it is what an agent walks to find *"the Billing address fields"*. Its NAME already
//! came from `<legend>` correctly, so only the role was wrong: a correct name on a meaningless role.
//!
//! ## ⚠ MEASURED AND DELIBERATELY NOT CHANGED
//!
//! ```text
//!   <input type=date/time/datetime-local/month/week>   chrome: Date / InputTime / DateTime
//!                                                       ours:   textbox   (KEPT)
//! ```
//!
//! Chrome's roles here are internal, with no ARIA equivalent — and unlike a colour well these
//! controls really do accept typed text, so `textbox` is both the useful answer and a non-harmful
//! one. Kept on purpose rather than folded into the `color` arm because the enum happened to have a
//! slot. The five rows are asserted AS `textbox` so the decision is a claim rather than an omission.
//!
//! ⚠ `<summary>` (Chrome `DisclosureTriangle`), `<figcaption>`, `<legend>`, `<dl>`, `<abbr>`,
//! `<video>`, `<audio>` all get Chrome INTERNAL role names with no ARIA counterpart. Adopting them
//! would put Chrome internals into a vocabulary that is otherwise ARIA's, so they are left `generic`
//! and named here. `<summary>`'s empty NAME (Chrome says `"More"`) is a separate gap, first surfaced
//! by t1380's own control row and still open.
//!
//! ⚠ The oracle for every row is CDP `Accessibility.getFullAXTree` on `--headless=new
//! --force-renderer-accessibility`, not a WPT expectation — see audit #80 for why there is no suite
//! to ask.

use manuk_a11y::{role_of, Role};

const HTML: &str = r##"<!doctype html><html><head></head><body>
<input id="i_file" type="file">
<input id="i_color" type="color">
<input id="i_text" type="text">
<input id="i_date" type="date">
<input id="i_time" type="time">
<input id="i_dtl" type="datetime-local">
<input id="i_month" type="month">
<input id="i_week" type="week">
<input id="i_range" type="range">
<input id="i_number" type="number">
<input id="i_search" type="search">
<input id="i_check" type="checkbox">
<input id="i_submit" type="submit" value="Go">
<select id="s_one"><option>o</option></select>
<select id="s_multi" multiple><option>o</option></select>
<select id="s_size" size="4"><option>o</option></select>
<select id="s_size1" size="1"><option>o</option></select>
<fieldset id="e_fieldset"><legend>Billing</legend><input></fieldset>
<details id="e_details"><summary>More</summary><p>b</p></details>
<address id="e_address">a</address>
<hgroup id="e_hgroup"><h2>t</h2></hgroup>
<section id="e_section">s</section>
<article id="e_article">a</article>
<aside id="e_aside">a</aside>
<blockquote id="e_quote">q</blockquote>
<figure id="e_figure">f</figure>
<hr id="e_hr">
<menu id="e_menu"><li>x</li></menu>
</body></html>"##;

#[test]
fn implicit_roles_match_html_aam() {
    let fonts = manuk_text::FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://role.test/", &fonts, 1200.0);
    let dom = page.dom();
    let tree = page.a11y_tree();
    let node = |id: &str| {
        dom.get_element_by_id(dom.root(), id)
            .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"))
    };

    // ── VACUITY. Every subject must be IN the a11y tree — a role is only a claim about something
    //    the agent can see, and t1380 is the tick that made absence a real possibility.
    for id in ["i_file", "s_multi", "e_fieldset", "e_details"] {
        let n = node(id);
        assert!(
            tree.iter().any(|x| x.node == n),
            "VACUOUS: #{id} is not in the a11y tree at all, so its role asserts nothing"
        );
    }

    // (id, the role Chrome's AX tree reports, what the row decides)
    let rows: &[(&str, Role, &str)] = &[
        (
            "i_file",
            Role::Button,
            "THE ROW WITH TEETH — as a `textbox` an upload control is invisible to \"click Choose \
             File\" AND `type_into` accepts it and silently does nothing",
        ),
        (
            "i_color",
            Role::Generic,
            "HTML-AAM gives a colour well NO corresponding role; `generic` keeps it in the tree \
             and addressable without inviting a `type_into` that cannot work",
        ),
        (
            "s_multi",
            Role::ListBox,
            "a multi-select is a LISTBOX — a visible list with a possibly-plural selection, not a \
             dropdown that opens and closes",
        ),
        (
            "s_size",
            Role::ListBox,
            "…and `size > 1` is the OTHER half of the same HTML-AAM discriminator; either alone \
             would pass against an implementation that read only the other",
        ),
        (
            "s_size1",
            Role::ComboBox,
            "CONTROL — `size=1` is the boundary and stays a combobox, so the rule is `> 1` and not \
             `has a size attribute`",
        ),
        (
            "s_one",
            Role::ComboBox,
            "CONTROL — the plain collapsed select, which must not move",
        ),
        (
            "e_fieldset",
            Role::Group,
            "THE ROW WITH CORPUS WEIGHT — every multi-section form is built out of `<fieldset>`, \
             and it is what an agent walks to find \"the Billing address fields\"",
        ),
        ("e_details", Role::Group, "HTML-AAM maps `<details>` to group"),
        ("e_address", Role::Group, "…and `<address>`"),
        ("e_hgroup", Role::Group, "…and `<hgroup>`"),
        (
            "e_section",
            Role::Generic,
            "CONTROL — an UNNAMED `<section>` is generic in Chrome too, so this is not \"anything \
             sectioning is a group\"",
        ),
        ("e_article", Role::Article, "CONTROL — the sectioning roles that were already right"),
        ("e_aside", Role::Complementary, "CONTROL"),
        ("e_quote", Role::Blockquote, "CONTROL"),
        ("e_figure", Role::Figure, "CONTROL"),
        ("e_hr", Role::Separator, "CONTROL"),
        ("e_menu", Role::List, "CONTROL"),
        ("i_text", Role::TextBox, "CONTROL — the input default, which must survive the new arms"),
        ("i_range", Role::Slider, "CONTROL"),
        ("i_number", Role::SpinButton, "CONTROL"),
        ("i_search", Role::SearchBox, "CONTROL"),
        ("i_check", Role::CheckBox, "CONTROL"),
        ("i_submit", Role::Button, "CONTROL — a button that was already a button"),
        // MEASURED AND DELIBERATELY KEPT — see the module header.
        ("i_date", Role::TextBox, "Chrome's `Date` is an internal role with no ARIA equivalent, and a date field DOES accept typed text, so textbox is useful and non-harmful — a claim, not an omission"),
        ("i_time", Role::TextBox, "…the same for `<input type=time>` (Chrome `InputTime`)"),
        ("i_dtl", Role::TextBox, "…and `datetime-local` (Chrome `DateTime`)"),
        ("i_month", Role::TextBox, "…and `month`"),
        ("i_week", Role::TextBox, "…and `week`"),
    ];
    for (id, want, why) in rows {
        let got = role_of(dom, node(id));
        assert_eq!(
            got.as_ref(),
            Some(want),
            "G_AX_ROLE_HTML_AAM #{id}: Chrome's AX tree says {:?}, got {:?}.\n  {why}",
            want.as_str(),
            got.as_ref().map(Role::as_str)
        );
    }

    // ── AND THROUGH THE TREE, not only through `role_of`: the agent reads the tree, and a mapping
    //    wired to one entrance is the shape this file's neighbours have been caught by four times.
    for (id, want) in [
        ("i_file", Role::Button),
        ("s_multi", Role::ListBox),
        ("e_fieldset", Role::Group),
    ] {
        let n = node(id);
        let in_tree = tree
            .iter()
            .find(|x| x.node == n)
            .map(|x| x.role.clone())
            .unwrap_or(Role::Generic);
        assert_eq!(
            in_tree,
            want,
            "G_AX_ROLE_HTML_AAM #{id} (AX TREE): the tree publishes {:?} where `role_of` says \
             {:?} — a mapping wired to one entrance is not wired",
            in_tree.as_str(),
            want.as_str()
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  delete the `"file" => Role::Button` arm (the pre-tick fall-through)
//       -> only `i_file` fails, at `textbox`. Every date/time row stays green, which is what says
//          the fall-through was wrong for ONE input type and deliberately kept for five others.
// N2  make the `<select>` test `el.attr("size").is_some()` instead of `> 1`
//       -> only `s_size1` fails: the boundary row is the one that separates the rule from the
//          attribute's presence.
// N3  map only `<fieldset>` to Group, leaving details/address/hgroup generic
//       -> those three fail and `e_fieldset` passes — four elements, one HTML-AAM row, and a
//          partial fix looks identical to a complete one on the fixture that motivated it.
