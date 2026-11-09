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
//! RG.E2.b closed the second half: `AgentBrowser::activate` now performs the script navigation (staynb's
//! Reserve stay read NO EFFECT for the same reason). This gate pins both halves on a hermetic React-style
//! page (one delegated listener on `document`, the card is a plain `<div>`, the target an inner `<img>`):
//!   1. the handler is called with the inner element as `target`  (the handler names its target page after `e.target`: `..._IMG.html`)
//!   2. the navigation it requested is performed by the click      (the browser is on the target page)
//! Mutant: the same page with the handler never attached must stay on the card page, so a click that
//! skips the handler cannot pass.

use manuk_agent::AgentBrowser;

fn page(attach: bool) -> String {
    let attach = if attach { "true" } else { "false" };
    format!(
        r##"<!doctype html><html><head><meta charset=utf-8><title>Shop</title><style>
body {{ margin: 0 }} .card {{ width: 200px; height: 120px }} img {{ width: 200px; height: 120px; display: block }}
</style></head><body>
<div class="card" data-href="g_p0a_card_target_"><img alt="Image Papa Johns Pizza" width="200" height="120"></div>
<pre id="log"></pre>
<script>
if ({attach}) document.addEventListener('click', function (e) {{
  var c = e.target.closest ? e.target.closest('[data-href]') : null;
  document.getElementById('log').textContent = 'handled target=' + e.target.tagName;
  if (c) location.href = c.getAttribute('data-href') + e.target.tagName + '.html';
}});
</script></body></html>"##
    )
}

/// The URL the browser is on after clicking the card.
fn click_card(attach: bool) -> String {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let dir = std::env::temp_dir();
    let path = dir.join(format!("g_p0a_card_{attach}.html"));
    std::fs::write(&path, page(attach)).unwrap();
    std::fs::write(
        dir.join("g_p0a_card_target_IMG.html"),
        "<html><body><h1>Store</h1></body></html>",
    )
    .unwrap();
    let mut b = AgentBrowser::new(1024, 768);
    rt.block_on(async {
        b.navigate(&format!("file://{}", path.display()))
            .await
            .expect("navigate");
        let node = b
            .resolve_handle(&manuk_a11y::Role::Image, "Image Papa Johns Pizza")
            .expect("the card image resolves");
        b.activate(node).await.expect("click");
    });
    b.current_url().unwrap().to_string()
}

// One #[test]: SpiderMonkey is one runtime per process thread; two parallel tests segfault.
#[test]
fn a_click_on_a_card_image_reaches_the_pages_handler_and_follows_its_navigation() {
    let url = click_card(true);
    assert!(
        url.ends_with("g_p0a_card_target_IMG.html"),
        "the handler (target=IMG) asked for the target page and the click must go there; on {url}"
    );

    // MUTANT: the handler never attached (a click that skips it) must stay on the card page.
    let url = click_card(false);
    assert!(
        url.ends_with("g_p0a_card_false.html"),
        "no handler attached: the click must not navigate; on {url}"
    );
}
