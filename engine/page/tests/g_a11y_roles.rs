//! G_A11Y_ROLES — the agent can NAME the interactive widgets modern web apps build.
//!
//! **The failure this gate exists for.** Web apps do not use native `<select>`/`<input>` for their
//! richest controls — they build them out of `<div role="tab">`, `<div role="switch">`,
//! `<div role="slider">`, `role="menu"`/`menuitem`, `role="dialog"`. The a11y tree's `Role` enum
//! stopped at ~26 roles, so every one of those collapsed to `Generic`. The observation an agent read
//! was an anonymous box:
//!
//! ```text
//! "Dark mode"        <- a role="switch" the agent cannot recognise as a toggle
//! "Home"             <- a role="tab" it cannot recognise as a selectable tab
//! ```
//!
//! It could click the box, but it could not *ground* it — could not answer "is this a switch, and is
//! it on?" even though `state_of` already computes `checked`/`selected`/`value` from the `aria-*`
//! attributes. The roles were the missing hook. This gate asserts the widget roles now surface with
//! their tokens and that the state the agent needs rides along.
//!
//! RED proof: revert any arm of `Role::from_aria_token` / `role_of` (e.g. drop the `"switch"` token)
//! and that widget renders as `generic`/its default, so the `switch "Dark mode" [checked]` line — and
//! the others — vanish. Verified red by deleting the new tokens before landing.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <div role="tablist" aria-label="Sections">
    <div role="tab" aria-selected="true">Home</div>
    <div role="tab" aria-selected="false">Settings</div>
  </div>
  <div role="switch" aria-checked="true" aria-label="Dark mode"></div>
  <div role="slider" aria-valuenow="42" aria-label="Volume"></div>
  <div role="menu" aria-label="Actions">
    <div role="menuitem">Copy</div>
    <div role="menuitem" aria-disabled="true">Paste</div>
  </div>
  <div role="dialog" aria-label="Confirm">Sure?</div>
  <progress value="0.7" aria-label="Upload"></progress>
  <input type="range" aria-label="Zoom">
  <input type="number" aria-label="Qty" value="3">
</body></html>"#;

fn line_for<'a>(lines: &'a [String], needle: &str) -> &'a str {
    lines
        .iter()
        .find(|l| l.contains(needle))
        .map(String::as_str)
        .unwrap_or_else(|| panic!("no observation line containing {needle:?} in {lines:#?}"))
}

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn the_agent_can_ground_web_app_widgets() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://app.test/", &fonts, 800.0);
    let lines = page.a11y_tree().to_observation_lines();

    // ── ARIA widget roles now carry their token, not `generic`. ────────────────────────────────
    // A selected tab and an unselected one — the state an agent reads to know which pane is up.
    let home = line_for(&lines, "\"Home\"");
    assert!(
        home.starts_with("tab "),
        "role=tab must expose `tab`, got {home:?}"
    );
    assert!(
        home.contains("selected") && !home.contains("unselected"),
        "selected tab: {home:?}"
    );
    // `selected` renders only when true, so an unselected tab carries no `selected` suffix.
    let settings = line_for(&lines, "\"Settings\"");
    assert!(settings.starts_with("tab "), "role=tab: {settings:?}");
    assert!(
        !settings.contains("selected"),
        "unselected tab has no suffix: {settings:?}"
    );

    // A switch that is ON — the checked state (from `aria-checked`) rides on the new role.
    let sw = line_for(&lines, "\"Dark mode\"");
    assert!(
        sw.starts_with("switch "),
        "role=switch must expose `switch`, got {sw:?}"
    );
    assert!(
        sw.contains("checked") && !sw.contains("unchecked"),
        "switch is on: {sw:?}"
    );

    // A slider carries its current value.
    let slider = line_for(&lines, "\"Volume\"");
    assert!(slider.starts_with("slider "), "role=slider: {slider:?}");
    assert!(slider.contains("value=\"42\""), "slider value: {slider:?}");

    // Menu container + its items; a disabled item is flagged so the agent does not click it.
    assert!(
        line_for(&lines, "\"Actions\"").starts_with("menu "),
        "role=menu"
    );
    let copy = line_for(&lines, "\"Copy\"");
    assert!(copy.starts_with("menuitem "), "role=menuitem: {copy:?}");
    let paste = line_for(&lines, "\"Paste\"");
    assert!(paste.starts_with("menuitem "), "role=menuitem: {paste:?}");
    assert!(paste.contains("disabled"), "disabled menuitem: {paste:?}");

    // A modal dialog — tells the agent a modal is up.
    assert!(
        line_for(&lines, "\"Confirm\"").starts_with("dialog "),
        "role=dialog"
    );

    // ── Native HTML widgets get their HTML-AAM implicit role. ──────────────────────────────────
    let upload = line_for(&lines, "\"Upload\"");
    assert!(
        upload.starts_with("progressbar "),
        "<progress> is a progressbar: {upload:?}"
    );
    assert!(
        upload.contains("value=\"0.7\""),
        "progress value: {upload:?}"
    );

    let zoom = line_for(&lines, "\"Zoom\"");
    assert!(
        zoom.starts_with("slider "),
        "<input type=range> is a slider: {zoom:?}"
    );

    let qty = line_for(&lines, "\"Qty\"");
    assert!(
        qty.starts_with("spinbutton "),
        "<input type=number> is a spinbutton: {qty:?}"
    );
    assert!(qty.contains("value=\"3\""), "spinbutton value: {qty:?}");
}
