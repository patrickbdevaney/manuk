//! **G_AGENT_CLICK_FOCUSES_INPUT — a pointer press on a text field focuses it.**
//!
//! staynb's GUESTS popover opens from `onFocus` on a readonly `<input>`; the engine's click
//! dispatched pointer/mouse/click events but never moved focus, so the popover never opened
//! (0/3 vs 3/3 once `.focus()` was called by hand — W4.P0.a.b.a). Mutant:
//! `MANUK_CLICK_FOCUS_MUTATE=off` drops the focus step and this gate goes RED.

use manuk_a11y::Role;
use manuk_agent::AgentBrowser;

const HTML: &str = r#"<body>
<input id=a aria-label="Alpha" readonly onfocus="window.m=(window.m||'')+'a+'" onblur="window.m=(window.m||'')+'a-'" style="display:block;width:200px;height:30px">
<input id=b aria-label="Beta" onfocus="window.m=(window.m||'')+'b+'" style="display:block;width:200px;height:30px">
<input id=d aria-label="Dis" disabled onfocus="window.m=(window.m||'')+'d+'" style="display:block;width:200px;height:30px">
<div id=__probe></div>
<p id=p style="width:200px;height:30px">plain text</p>
</body>"#;

static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn click_name(b: &mut AgentBrowser, name: &str) {
    let node = b
        .a11y_tree()
        .unwrap()
        .iter()
        .find(|n| n.role == Role::TextBox && n.name == name)
        .map(|n| n.node)
        .unwrap_or_else(|| panic!("VACUOUS: no textbox {name:?}"));
    b.activate(node).await.unwrap();
}

fn eval(b: &mut AgentBrowser, js: &str) -> String {
    b.probe_eval(&format!(
        "document.getElementById('__probe').textContent=String({js})"
    ))
    .0
}

#[tokio::test]
async fn a_click_on_a_text_input_focuses_it() {
    let _s = ONE_AT_A_TIME.lock().await;
    let mut b = AgentBrowser::new(800, 600);
    b.navigate(&format!("data:text/html,{HTML}")).await.unwrap();
    // vacuity: handlers are live and nothing is focused yet
    assert_eq!(eval(&mut b, "String(window.m)"), "undefined");
    assert_eq!(eval(&mut b, "document.activeElement.tagName"), "BODY");

    click_name(&mut b, "Alpha").await;
    assert_eq!(eval(&mut b, "document.activeElement.id"), "a");
    assert_eq!(eval(&mut b, "window.m"), "a+");

    // moving to another field blurs the previous one, then focuses the new one
    click_name(&mut b, "Beta").await;
    assert_eq!(eval(&mut b, "document.activeElement.id"), "b");
    assert_eq!(eval(&mut b, "window.m"), "a+a-b+");

    // a disabled input never takes focus
    click_name(&mut b, "Dis").await;
    assert_eq!(eval(&mut b, "document.activeElement.id"), "b");
    assert_eq!(eval(&mut b, "window.m"), "a+a-b+");
}
