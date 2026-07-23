//! **G_DISABLED_FOCUS — a `disabled` form control cannot receive focus.**
//!
//! A disabled control is not a tab stop and `el.focus()` on it is a no-op — Tab skips it, and no
//! `:focus` styling should ever land on a greyed-out button. `Page::set_focus` (the single sink the
//! shell's Tab handling, the agent's focus grounding, and the JS `el.focus()` queue all funnel through)
//! checked `inert` (tick 451) but not `disabled`, so focus reached a disabled control and drove its
//! `:focus` rule. The engine already knows disabledness (`is_disabled` — own attribute *or* an ancestor
//! `<fieldset disabled>`, the idiomatic bulk-disable); `set_focus` now consults it too. Each claim is a
//! way this goes RED:
//!
//!   * focusing an ENABLED control succeeds (returns `true`, `:focus` applies) — the fix is not a veto.
//!   * focusing a `disabled` control is refused (returns `false`, `:focus` does not apply).
//!   * focusing a control inside `<fieldset disabled>` is refused too (inherited disabledness).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  input          { box-sizing: border-box; width: 100px; }
  #live:focus    { width: 300px; }
  #off:focus     { width: 300px; }
  #inset:focus   { width: 300px; }
</style></head><body style="margin:0">
<input id="live">
<input id="off" disabled>
<fieldset disabled>
  <input id="inset">
</fieldset>
</body></html>"##;

fn node_of(page: &manuk_page::Page, sel: &str) -> manuk_dom::NodeId {
    let root = page.dom().root();
    manuk_css::query_selector_all(page.dom(), root, sel)[0]
}

fn width_of(page: &manuk_page::Page, sel: &str) -> f32 {
    let n = node_of(page, sel);
    page.node_rects().get(&n).map(|r| r.width).unwrap_or(0.0)
}

#[test]
fn disabled_control_cannot_receive_focus() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://disabled-focus.test/", &fonts, 800.0);

    // Baseline: all three inputs at their base 100px, or every claim below is vacuous.
    for id in ["#live", "#off", "#inset"] {
        assert!(
            (width_of(&page, id) - 100.0).abs() < 0.5,
            "G_DISABLED_FOCUS: unfocused baseline for {id} must be 100px — got {}. The fixture is not \
             measuring what it claims.",
            width_of(&page, id)
        );
    }

    // An ENABLED control focuses normally.
    let live = node_of(&page, "#live");
    assert!(
        page.set_focus(Some(live), true, &fonts, 800.0),
        "G_DISABLED_FOCUS: focusing an enabled control must succeed — the fix must not veto all focus."
    );
    assert!(
        (width_of(&page, "#live") - 300.0).abs() < 0.5,
        "G_DISABLED_FOCUS: `#live:focus` must apply to the enabled control — got {}.",
        width_of(&page, "#live")
    );

    // A directly-`disabled` control must refuse focus.
    let off = node_of(&page, "#off");
    assert!(
        !page.set_focus(Some(off), true, &fonts, 800.0),
        "G_DISABLED_FOCUS: focusing a `disabled` control must be refused (return false) — it returned \
         true, so Tab lands on a greyed-out button and `:focus` styles it."
    );
    assert!(
        (width_of(&page, "#off") - 100.0).abs() < 0.5,
        "G_DISABLED_FOCUS: `#off:focus` must NOT apply — got width {}. Focus reached a disabled control \
         and drove the cascade.",
        width_of(&page, "#off")
    );

    // A control inside `<fieldset disabled>` inherits disabledness and must refuse focus too.
    let inset = node_of(&page, "#inset");
    assert!(
        !page.set_focus(Some(inset), true, &fonts, 800.0),
        "G_DISABLED_FOCUS: a control inside `<fieldset disabled>` must refuse focus — inherited \
         disabledness (the idiomatic bulk-disable) is not being consulted."
    );
    assert!(
        (width_of(&page, "#inset") - 100.0).abs() < 0.5,
        "G_DISABLED_FOCUS: `#inset:focus` must NOT apply — got width {}.",
        width_of(&page, "#inset")
    );
}
