//! **W4.P0.a — the click family, traced.** Eight REAL tasks (dashdish-2/7/10, gocalendar-1/3/4,
//! gomail-5, staynb-2) pass on cdp-layer and fail on manuk because "a click on a JS-handled card does
//! not do what cdp-layer's click does".
//!
//! Traced on the live DashDish store card (`click-probe`, 3/3 runs): the click DOES reach the page's
//! own delegated handler (target = the card's `<img>`, handler ran). What the handler does is
//! `location.href = '/store/..'`, which the engine records as a script navigation
//! (`Page::take_script_navigation`) for the HOST to perform. The shell and the WPT runner drain it;
//! `AgentBrowser::activate` — the engine arm of manuk-mcp — never does, so the tab stays on the
//! home page and the act reports `Inert` / "url: unchanged".
//!
//! This gate pins both halves on a hermetic React-style page (one delegated listener on `document`,
//! the card is a plain `<div>`, the target an inner `<img>`):
//!   1. the handler is called with the inner element as `target`           (the part that works)
//!   2. the navigation it requested is pending on the page after the click  (the part nobody follows)
//! Mutant: the same page with the handler never attached must turn (1) red, so a click that skips the
//! handler cannot pass.

use manuk_agent::AgentBrowser;

fn page(attach: bool) -> String {
    let attach = if attach { "true" } else { "false" };
    format!(
        r##"<!doctype html><html><head><meta charset=utf-8><title>Shop</title><style>
body {{ margin: 0 }} .card {{ width: 200px; height: 120px }} img {{ width: 200px; height: 120px; display: block }}
</style></head><body>
<div class="card" data-href="/store/42"><img alt="Image Papa Johns Pizza" width="200" height="120"></div>
<pre id="log"></pre>
<script>
if ({attach}) document.addEventListener('click', function (e) {{
  var c = e.target.closest ? e.target.closest('[data-href]') : null;
  document.getElementById('log').textContent = 'handled target=' + e.target.tagName;
  if (c) location.href = c.getAttribute('data-href');
}});
</script></body></html>"##
    )
}

/// (handler log text, script navigation pending after the click)
fn click_card(attach: bool) -> (String, Option<String>) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let path = std::env::temp_dir().join(format!("g_p0a_card_{attach}.html"));
    std::fs::write(&path, page(attach)).unwrap();
    let mut b = AgentBrowser::new(1024, 768);
    rt.block_on(async {
        b.navigate(&format!("file://{}", path.display())).await.expect("navigate");
        let node = b
            .resolve_handle(&manuk_a11y::Role::Image, "Image Papa Johns Pizza")
            .expect("the card image resolves");
        b.activate(node).await.expect("click");
    });
    let (log, _) = b.probe_eval(
        "(function(){var p=document.createElement('pre');p.id='__probe';document.body.appendChild(p);p.textContent=document.getElementById('log').textContent;})()",
    );
    (log, b.probe_take_script_navigation())
}

// One #[test]: SpiderMonkey is one runtime per process thread; two parallel tests segfault.
#[test]
fn a_click_on_a_card_image_reaches_the_pages_handler_and_leaves_its_navigation_pending() {
    let (log, nav) = click_card(true);
    assert_eq!(log, "handled target=IMG", "the page's own listener must run with the inner element as target");
    assert!(
        nav.as_deref().is_some_and(|u| u.ends_with("/store/42")),
        "the handler asked for /store/42; got {nav:?}"
    );

    // MUTANT: the handler never attached (a click that skips it) must read as red.
    let (log, nav) = click_card(false);
    assert_ne!(log, "handled target=IMG", "no handler attached: the probe must NOT see a call");
    assert_eq!(nav, None);
}
