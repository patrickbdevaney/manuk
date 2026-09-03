//! **G_AX_POPOVER_MINIMUM_ROLE — the semantic half of tick 1395's popover work.**
//!
//! Chosen by **constitution check #132**, not by the histogram. t1395 landed the popover's observable
//! state — `:popover-open` in both selector engines, a real `ToggleEvent` — and moved
//! `html/semantics/popovers` 9 → 62. The same suite then said what it had not done:
//!
//! ```text
//!   popover-minimum-role.html   assert_equals: role starts as none, expected "none" but got "generic"
//! ```
//!
//! So after t1395 a popover opened, painted, matched `:popover-open` and announced itself with a
//! typed event — **and the agent's perception layer still could not tell it from an ordinary
//! `<div>`.** The capability was complete on every channel a human uses and absent on the one this
//! project exists to serve. I3 says the semantic model lands in LOCKSTEP; five ticks in a row it had
//! not, and only the per-window view showed it.
//!
//! ## ⭐⭐⭐ THE RULE, AND THE PAIR THAT NAMES IT
//!
//! HTML-AAM raises a **visible** `[popover]` to `group` — but only when the element has **no role
//! mapping of its own**. Every row below is headless-Chrome-measured through CDP
//! `Accessibility.getPartialAXTree` (the oracle check #131 named, because Interop 2026 lists
//! accessibility testing as an *investigation* effort — the vendors' own position is that no suite
//! can decide a11y-tree correctness yet).
//!
//! ```text
//!                                             chrome            before      after
//!   <div popover>            closed           none (ignored)    generic     Generic
//!   <div popover>            visible          GROUP             generic     Group
//!   <span popover>           visible          GROUP             generic     Group
//!   <section popover>        visible          GENERIC           generic     Generic
//!   <section popover>        visible + named  region            region      Region
//!   <button popover>         visible          button            button      Button
//!   <nav popover>            visible          navigation        navigation  Navigation
//!   <div popover role=none>  visible          none (ignored)    generic     Generic
//!   <div popover role=alert> visible          alert             alert       Alert
//!   <div popover> + visibility:hidden         none (ignored)    generic     Generic
//! ```
//!
//! ⭐⭐ **`<div>` and an unnamed `<section>` BOTH compute to `generic` without the attribute, and the
//! popover raises only one of them.** A rule written against the *computed* role would have raised
//! both and been wrong about `<section>`. The discriminator is *does HTML-AAM map this tag at all* —
//! `<section>` has a mapping (region-when-named, generic otherwise), `<div>` and `<span>` have none.
//! One row could not have shown that; the pair is the finding, and it is why the implementation lives
//! in `role_of`'s DEFAULT ARM, which **is** the set of unmapped tags rather than a list of them.
//!
//! ## ⚠ MEASURED AND DELIBERATELY NOT CHANGED
//!
//! ```text
//!   <div popover hidden style="display:block">   chrome: GROUP     ours: excluded from the tree
//! ```
//!
//! Chrome renders it — an inline `display:block` beats the UA sheet's `[hidden] { display: none }` —
//! so it is a `group`. `is_hidden()` treats the `hidden` ATTRIBUTE as an unconditional exclusion, one
//! screen away from this code and with its own callers. That is a real divergence about `hidden`, not
//! about popovers, and folding it in here would make this tick's claim untestable. Recorded, not
//! bundled.

use manuk_a11y::{role_of, Role};

const HTML: &str = r##"<!doctype html><html><head></head><body>
<div     id="p_closed"  popover>closed</div>
<div     id="p_open"    popover data-manuk-popover-open>open via showPopover()</div>
<div     id="p_inline"  popover style="display:block">forced visible inline</div>
<span    id="p_span"    popover style="display:block">span</span>
<section id="p_section" popover style="display:block">section, unnamed</section>
<section id="p_sec_lbl" popover style="display:block" aria-label="Named">section, named</section>
<button  id="p_button"  popover style="display:block">button</button>
<nav     id="p_nav"     popover style="display:block">nav</nav>
<div     id="p_rnone"   popover style="display:block" role="none">role none</div>
<div     id="p_ralert"  popover style="display:block" role="alert">role alert</div>
<div     id="p_invis"   popover style="display:block;visibility:hidden">invisible</div>
<div     id="c_div">plain div, no popover</div>
<section id="c_section">plain section, unnamed</section>
</body></html>"##;

#[test]
fn a_visible_popover_with_no_role_mapping_of_its_own_is_a_group() {
    let fonts = manuk_text::FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://popover-role.test/", &fonts, 1200.0);
    let dom = page.dom();
    let node = |id: &str| {
        dom.get_element_by_id(dom.root(), id)
            .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"))
    };

    // ── VACUITY. A role is a claim about something that EXISTS; assert the fixture parsed the
    //    attribute at all, or every row below could pass against an empty document.
    assert!(
        dom.element(node("p_open"))
            .and_then(|e| e.attr("popover"))
            .is_some(),
        "VACUOUS: #p_open has no `popover` attribute, so nothing below tests the popover rule"
    );

    // (id, the role Chrome's AX tree reports, what the row decides)
    let rows: &[(&str, Role, &str)] = &[
        (
            "p_closed",
            Role::Generic,
            "a CLOSED popover is not rendered, so the mapping does not apply — Chrome ignores the \
             node entirely and reports `none`",
        ),
        (
            "p_open",
            Role::Group,
            "THE ROW WITH TEETH — `showPopover()` writes `data-manuk-popover-open`, and that marker \
             IS the rendered-state answer because the UA sheet keys `display` off exactly it",
        ),
        (
            "p_inline",
            Role::Group,
            "an author's inline `display` beats the UA sheet's `[popover] { display: none }`, and \
             this is the spelling WPT's own popover-minimum-role.html uses",
        ),
        (
            "p_span",
            Role::Group,
            "`<span>` has no HTML-AAM mapping either, so the rule reaches it — the arm is the set",
        ),
        (
            "p_section",
            Role::Generic,
            "⭐ THE PAIR: an unnamed <section> computes to `generic` exactly like a <div>, and the \
             popover raises only the <div>. <section> HAS a mapping, so the rule never applies",
        ),
        (
            "p_sec_lbl",
            Role::Region,
            "…and naming it proves the <section> row is the MAPPING winning, not the rule failing",
        ),
        (
            "p_button",
            Role::Button,
            "an implicit role of its own wins — `group` is a MINIMUM, not an override",
        ),
        ("p_nav", Role::Navigation, "likewise for a landmark"),
        (
            "p_rnone",
            Role::Generic,
            "an explicit `role=none` wins over the popover mapping (Chrome ignores the node)",
        ),
        (
            "p_ralert",
            Role::Alert,
            "…and so does any other explicit role — the explicit branch returns before the arm",
        ),
        (
            "p_invis",
            Role::Generic,
            "`visibility: hidden` un-renders it whatever `display` says, so the mapping lapses",
        ),
        (
            "c_div",
            Role::Generic,
            "CONTROL — the same <div> without the attribute must NOT be a group, or the rule is \
             matching everything",
        ),
        (
            "c_section",
            Role::Generic,
            "CONTROL — an unnamed <section> is `generic` with or without a popover, which is what \
             makes the p_section row readable as the mapping and not as an accident",
        ),
    ];

    for (id, want, why) in rows {
        let got = role_of(dom, node(id));
        assert_eq!(
            got,
            Some(want.clone()),
            "G_AX_POPOVER_MINIMUM_ROLE: #{id} — expected {want:?}, got {got:?}\n  {why}\n\n  \
             HTML-AAM raises a VISIBLE [popover] to `group` only when the element has NO role \
             mapping of its own, which is exactly `role_of`'s default arm. Every row is measured \
             against headless Chrome via CDP Accessibility.getPartialAXTree."
        );
    }
}
