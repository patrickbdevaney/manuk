//! G_A11Y_STATE — the agent can CONFIRM ITS OWN ACTION.
//!
//! **The failure this gate exists for.** `A11yNode` carried `role`, `name`, `bbox` and `z` — and
//! nothing about state. So the observation an agent reads was:
//!
//! ```text
//! checkbox "Remember me"      <- before the click
//! checkbox "Remember me"      <- after the click
//! ```
//!
//! Byte-identical. An agent that cannot observe the result of its own action cannot verify it: it
//! either proceeds on faith, or re-clicks and toggles the setting straight back off. Same for the
//! menu it just opened (`aria-expanded`), the field it just typed into (`value`), and the button it
//! must NOT click (`disabled` — clicking one means waiting forever for a result that is not coming).
//!
//! This is the agentic moat rather than an a11y nicety, which is why it is gated on the *difference*
//! between two snapshots rather than on the presence of a field.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <input type="checkbox" id="remember">
  <input type="checkbox" id="partial" aria-checked="mixed">
  <button id="send">Send</button>
  <button id="dead" disabled>Send</button>
  <button id="menu" aria-expanded="false">Menu</button>
  <input type="text" id="who" value="">
  <input type="email" id="mail" required>
  <details id="more"><summary>More</summary>body</details>
  <div role="slider" aria-valuenow="42" aria-label="Volume"></div>
  <script>
    document.getElementById('send').addEventListener('click', function() {
      // What a real page does on click: flip the control's state.
      document.getElementById('remember').checked = true;
      document.getElementById('menu').setAttribute('aria-expanded', 'true');
      document.getElementById('who').setAttribute('value', 'ada');
      document.getElementById('more').setAttribute('open', '');
    });
  </script>
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
fn the_agent_can_read_back_the_state_it_just_changed() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://app.test/", &fonts, 800.0);

    // ── BEFORE ───────────────────────────────────────────────────────────────────────────────
    let before = page.a11y_tree().to_observation_lines();

    // States that are simply read off the document.
    assert!(
        line_for(&before, "\"Volume\"").contains("value=\"42\""),
        "aria-valuenow reaches the agent: {:?}",
        line_for(&before, "\"Volume\"")
    );
    let disabled = before
        .iter()
        .filter(|l| l.contains("button") && l.contains("disabled"))
        .count();
    assert_eq!(
        disabled, 1,
        "exactly the disabled button reports `disabled` — an agent that clicks a disabled button \
         waits forever for a result that is never coming. Lines: {before:#?}"
    );
    assert!(
        before.iter().any(|l| l.contains("mixed")),
        "tri-state: aria-checked=\"mixed\" is not flattened to unchecked — a \"select all\" parent \
         checkbox means it. Lines: {before:#?}"
    );
    assert!(
        before.iter().any(|l| l.contains("required")),
        "a required field is marked, so an agent knows what a blocked submit is complaining about"
    );
    assert!(
        before.iter().any(|l| l.contains("collapsed")),
        "aria-expanded=false reads as collapsed: {before:#?}"
    );

    // A static, semantics-free line carries NO state suffix — state must be signal, not noise.
    assert!(
        before.iter().any(|l| l.trim_end().ends_with("\"Send\"")),
        "a plain button gets no state suffix at all: {before:#?}"
    );

    // ── ACT ──────────────────────────────────────────────────────────────────────────────────
    let send = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#send")[0];
    page.dispatch_click(send, &fonts, 800.0);

    // ── AFTER — and the whole claim is that this DIFFERS. ────────────────────────────────────
    let after = page.a11y_tree().to_observation_lines();
    assert_ne!(
        before, after,
        "G_A11Y_STATE: the observation must CHANGE when the page's state changes. If these are \
         equal, an agent has no way to tell whether its click did anything at all — which is the \
         entire failure this gate exists for."
    );

    let checkbox_after = after
        .iter()
        .find(|l| l.contains("checkbox") && l.contains("checked") && !l.contains("unchecked"))
        .unwrap_or_else(|| panic!("no checked checkbox after the click: {after:#?}"));
    assert!(
        checkbox_after.contains("checked"),
        "the checkbox the handler set now reads as checked — this is the agent confirming its own \
         action, and `el.checked = true` from script must be visible in the tree"
    );
    assert!(
        after
            .iter()
            .any(|l| l.contains("expanded") && !l.contains("collapsed")),
        "the menu the click opened now reads as expanded: {after:#?}"
    );
    assert!(
        after.iter().any(|l| l.contains("value=\"ada\"")),
        "the field's new value is readable back — how an agent verifies what it typed: {after:#?}"
    );
    assert!(
        after.iter().any(|l| l.contains("expanded")),
        "<details open> reports expanded: {after:#?}"
    );

    // ── FOCUS — host-owned, so it only appears when the caller supplies it. ──────────────────
    let who = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#who")[0];
    let unfocused = page.a11y_tree().to_observation_lines();
    assert!(
        !unfocused.iter().any(|l| l.contains("focused")),
        "the plain builder does not guess at focus: {unfocused:#?}"
    );
    let focused = page.a11y_tree_with_focus(Some(who)).to_observation_lines();
    assert!(
        focused.iter().any(|l| l.contains("focused")),
        "given the focused node by the host, the tree reports it: {focused:#?}"
    );
    assert_eq!(
        focused.iter().filter(|l| l.contains("focused")).count(),
        1,
        "exactly one node is focused"
    );
}
