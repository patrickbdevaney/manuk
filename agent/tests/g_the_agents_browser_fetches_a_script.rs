//! **G_THE_AGENTS_BROWSER_FETCHES_A_SCRIPT — the discriminating half of t1461: `Page::load` runs an
//! INLINE script perfectly well; what it never does is FETCH one.**
//!
//! ⭐⭐ The first mutation pass on `g_the_agents_browser_runs_the_page` came back **GREEN** when
//! `load_async` was reverted to `load` — the inline arm could not tell "has a JS engine" from
//! "finishes loading", because `Page::load` has the engine and simply never requests a subresource.
//! Wikipedia's collapsible behaviour is external, and so is essentially every real site's, so this
//! arm is the one that matches the web.
//!
//! ⚠ Kept in its own binary: two SpiderMonkey contexts in one test binary abort on drop.
//!
//! Mutations that must turn this red:
//!   1. `Page::load` instead of `Page::load_async`   → the external script never runs
//!   2. drop the `spidermonkey` feature              → it runs nothing at all

use manuk_agent::AgentBrowser;

fn roles_and_names(url: &str) -> Vec<(String, String)> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut b = AgentBrowser::new(1024, 768);
        b.navigate(url).await.expect("navigate");
        let tree = b.a11y_tree().expect("a11y tree");
        let (bag, _) = manuk_agent::a11y_score::manuk_bag(&tree);
        bag
    })
}

const EXTERNAL_HOST: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
body { margin: 0 }
.collapsed .content { display: none }
</style></head><body>
<div id="box" class="navbox"><button id="tog">[show]</button>
 <div class="content"><ul><li><a href="/a">Gamma</a></li></ul></div></div>
<script src="g_agent_runs_the_page_ext.js"></script>
</body></html>"##;

const EXTERNAL_JS: &str = r##"document.getElementById('box').className = 'navbox collapsed';
var b = document.createElement('button'); b.textContent = '[hide]';
document.getElementById('box').appendChild(b);"##;

#[test]
fn the_agents_browser_fetches_and_runs_an_external_script() {
    // ⭐ The discriminating arm. `Page::load` runs an INLINE script perfectly well; what it never
    //   does is fetch one. Wikipedia's collapsible behaviour is external, and so is essentially
    //   every real site's.
    let dir = std::env::temp_dir();
    std::fs::write(dir.join("g_agent_runs_the_page_ext.js"), EXTERNAL_JS).unwrap();
    let host = dir.join("g_agent_runs_the_page_ext.html");
    std::fs::write(&host, EXTERNAL_HOST).unwrap();

    let bag = roles_and_names(&format!("file://{}", host.display()));
    let names: Vec<&str> = bag.iter().map(|(_, n)| n.as_str()).collect();
    println!("EXTERNAL: {bag:?}");

    assert!(
        names.contains(&"[show]"),
        "VACUOUS: the static button is missing, so the page did not load — got {bag:?}"
    );
    assert!(
        names.contains(&"[hide]"),
        "the agent's browser did not FETCH and run an external script — `Page::load` parses and \
         lays out and stops, which is a browser that never finishes loading — got {bag:?}"
    );
    assert!(
        !names.contains(&"Gamma"),
        "the external script's hiding did not reach the tree — got {bag:?}"
    );
}
