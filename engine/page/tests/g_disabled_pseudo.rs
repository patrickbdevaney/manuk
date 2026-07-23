//! **G_DISABLED_PSEUDO — `:disabled`/`:enabled` follow `<fieldset disabled>` inheritance, in BOTH engines.**
//!
//! A form control is `:disabled` when it has its own `disabled` attribute OR when it sits inside a
//! `<fieldset disabled>` — the idiomatic way to disable a whole form section at once. Both selector
//! engines only checked the element's OWN attribute, so a control disabled via its fieldset:
//!   * was not returned by `querySelector(':disabled')` (the querySelector engine, `pseudo_matches`), and
//!   * was not styled by `input:disabled { … }` (the live Stylo cascade, `stylo_dom.rs`) — it rendered
//!     as if enabled, un-greyed, on every form that bulk-disables a section.
//!
//! One shared `is_disabled_control` (own attribute or an ancestor `<fieldset disabled>` — the same rule
//! the focus path uses) now backs both, so cascade / querySelector / focusability agree. Each claim is a
//! way this goes RED:
//!
//!   * `querySelector(':disabled')` includes the fieldset-disabled control and the own-disabled one, not
//!     the enabled one.
//!   * `querySelector(':enabled')` includes only the enabled one.
//!   * the cascade: `input:disabled { width:300px }` applies to the fieldset-disabled control (Stylo path).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  input           { box-sizing: border-box; width: 100px; }
  input:disabled  { width: 300px; }
</style></head><body style="margin:0">
<input id="own" disabled>
<fieldset disabled>
  <input id="inset">
</fieldset>
<input id="live">
</body></html>"##;

fn ids_matching(page: &manuk_page::Page, sel: &str) -> Vec<String> {
    let root = page.dom().root();
    manuk_css::query_selector_all(page.dom(), root, sel)
        .into_iter()
        .filter_map(|n| {
            page.dom()
                .element(n)
                .and_then(|e| e.attr("id"))
                .map(String::from)
        })
        .collect()
}

fn width_of(page: &manuk_page::Page, id: &str) -> f32 {
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, &format!("#{id}"))[0];
    page.node_rects().get(&n).map(|r| r.width).unwrap_or(0.0)
}

#[test]
fn disabled_pseudo_honours_fieldset_inheritance() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://disabled-pseudo.test/", &fonts, 800.0);

    // ── querySelector engine (pseudo_matches).
    let disabled = ids_matching(&page, ":disabled");
    assert!(
        disabled.contains(&"inset".to_string()),
        "G_DISABLED_PSEUDO: `:disabled` must match a control inside `<fieldset disabled>` — got {disabled:?}. \
         The querySelector engine is still checking only the element's own attribute."
    );
    assert!(
        disabled.contains(&"own".to_string()),
        "G_DISABLED_PSEUDO: `:disabled` must still match an own-`disabled` control — got {disabled:?}."
    );
    assert!(
        !disabled.contains(&"live".to_string()),
        "G_DISABLED_PSEUDO: `:disabled` must NOT match an enabled control — got {disabled:?} (over-matching)."
    );

    let enabled = ids_matching(&page, ":enabled");
    assert!(
        enabled.contains(&"live".to_string())
            && !enabled.contains(&"inset".to_string())
            && !enabled.contains(&"own".to_string()),
        "G_DISABLED_PSEUDO: `:enabled` must match only the enabled control (not the fieldset-disabled or \
         own-disabled ones) — got {enabled:?}."
    );

    // ── Live Stylo cascade (stylo_dom.rs): `input:disabled { width:300px }`.
    assert!(
        (width_of(&page, "inset") - 300.0).abs() < 0.5,
        "G_DISABLED_PSEUDO: `input:disabled` must style a control disabled via its `<fieldset disabled>` — \
         #inset width is {}, not 300. The cascade matcher ignores fieldset inheritance, so the section \
         renders un-greyed.",
        width_of(&page, "inset")
    );
    assert!(
        (width_of(&page, "own") - 300.0).abs() < 0.5,
        "G_DISABLED_PSEUDO: `input:disabled` must still style an own-disabled control — #own width {}.",
        width_of(&page, "own")
    );
    assert!(
        (width_of(&page, "live") - 100.0).abs() < 0.5,
        "G_DISABLED_PSEUDO: `input:disabled` must NOT style an enabled control — #live width {} (over-match).",
        width_of(&page, "live")
    );
}
