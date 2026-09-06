//! **G_THE_AGENTS_BROWSER_RUNS_THE_PAGE — the agentic browser had no JavaScript, and every number
//! Track B and Track C have ever reported was measured on static HTML.**
//!
//! ⭐⭐⭐ Two independent omissions, in two files, compounding:
//!
//! ```text
//!   agent/Cargo.toml   `manuk-page.workspace = true`   — `spidermonkey` is OPT-IN and was not taken
//!   agent/src/lib.rs   `Page::load(...)`               — the SYNCHRONOUS constructor: parses, lays
//!                                                        out, stops. No subresources, no
//!                                                        lifecycle, no scripts.
//! ```
//!
//! Neither reads as a defect on its own. A `Cargo.toml` line that omits a feature looks like a
//! smaller build, and `Page::load` looks like loading a page. Together they are **a browser that
//! never finishes loading**, and it is the browser `a11y-score`, `drive-probe`, `a11y-dump`,
//! `agent-run` and ~30 `agent/tests/` gates all run on.
//!
//! ## How it was found, which is the part worth keeping
//!
//! Not by reading the manifest. `a11y-score` said Wikipedia's tree scores **25.9% precision**, and
//! constitution check #140 asked what the ~1,900 excess nodes *are* rather than assuming. The
//! multiset difference named them in one run:
//!
//! ```text
//!   OURS IN EXCESS                     CHROME IN EXCESS
//!     681  listitem  ""                   4  button  "[show]"
//!      90  list      ""                   1  columnheader "[show] v · t · e Timeline of web browsers"
//!      71  row       ""
//! ```
//!
//! ⭐⭐ **The two columns are the same fact from both sides.** Chrome has `[show]` buttons that
//! MediaWiki's `jquery.makeCollapsible` *creates*; we have exactly the content those buttons hide.
//! We had not run the script. **An a11y "precision defect" was a JavaScript-execution gap** — and
//! the area it was filed under could not have told anyone that.
//!
//! ## The fixture is the whole of Wikipedia's mechanism in twelve lines
//!
//! A script that adds a class hiding a subtree, and appends a button. Before: 7 nodes, 28.6%
//! precision, the hidden list exposed and the button missing. After: 3 nodes, **100% / 100% / 100%**
//! against Chrome.
//!
//! ⚠⚠⚠ **`stylo` WAS PART OF THIS CHANGE AND WAS REFUSED.** Enabling it alongside `spidermonkey`
//! turned `g_ax_tree_excludes_display_none` red: under Stylo's UA sheet a collapsed `<select>`'s
//! `<option>`s are hidden, and Chrome exposes both. That gate's `Option` row exists for exactly this
//! and says so in its own comment — *"if the UA sheet hid them the way it hides a closed `<dialog>`,
//! this tick would have deleted every dropdown from the agent's perception."* The ratchet refuses a
//! capability bought with a regression, so only the JavaScript half landed. The cascade half is
//! recorded, not shipped, and it now has a named blocker instead of a preference.
//!
//! ⭐⭐ **THE TWO OMISSIONS NEEDED TWO FIXTURES, AND THE FIRST MUTATION PASS PROVED IT.** Reverting
//! `load_async` → `load` left the inline-script arm GREEN: the synchronous constructor *does* run an
//! inline `<script>` once SpiderMonkey is compiled in. What it cannot do is FETCH one. So the second
//! arm loads its script from a separate file — the only shape that separates "has a JS engine" from
//! "finishes loading" — and a gate written without it would have shipped an unproven change.
//!
//! Mutations that must turn this red:
//!   1. `Page::load` instead of `Page::load_async`   → the EXTERNAL script never runs
//!   2. drop the `spidermonkey` feature              → neither script runs
//!   3. assert only that the button appears          → passes without the hiding half
//!
//! ⚠⚠⚠ **ONE `#[test]` PER BINARY.** Each test builds a tokio runtime and a SpiderMonkey context;
//! two in one binary abort on drop with *"There are outstanding JS engine handles"* and the harness
//! reports a SIGSEGV. The external-script arm therefore lives in
//! `g_the_agents_browser_fetches_a_script.rs`, and the two must stay split.

use manuk_agent::AgentBrowser;

const FIXTURE: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
body { margin: 0 }
.collapsed .content { display: none }
</style></head><body>
<div id="box" class="navbox"><button id="tog">[show]</button>
 <div class="content"><ul><li><a href="/a">Alpha</a></li><li><a href="/b">Beta</a></li></ul></div></div>
<script>
document.getElementById('box').className = 'navbox collapsed';
var b = document.createElement('button'); b.textContent = '[hide]';
document.getElementById('box').appendChild(b);
</script></body></html>"##;

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
#[test]
fn the_agents_browser_runs_the_page_script() {
    let path = std::env::temp_dir().join("g_agent_runs_the_page.html");
    std::fs::write(&path, FIXTURE).unwrap();
    let bag = roles_and_names(&format!("file://{}", path.display()));
    let names: Vec<&str> = bag.iter().map(|(_, n)| n.as_str()).collect();
    println!("AGENT TREE: {bag:?}");

    // ── VACUITY. The button the markup ships with must be there, or this is measuring whether the
    //    page loaded at all rather than whether its SCRIPT ran.
    assert!(
        names.contains(&"[show]"),
        "VACUOUS: the static button is missing, so the page did not load — got {bag:?}"
    );

    // 1. THE SCRIPT RAN — this button exists only because `appendChild` created it.
    assert!(
        names.contains(&"[hide]"),
        "the agent's browser did not execute the page script — got {bag:?}"
    );

    // 2. AND ITS EFFECT ON THE TREE LANDED. The script's class hides the list; the accessibility
    //    tree must lose it. Chrome publishes neither `Alpha` nor `Beta` here.
    //    ⚠ Both halves are required: a browser that runs the script but re-reads a stale tree
    //    passes (1) and fails this.
    assert!(
        !names.contains(&"Alpha") && !names.contains(&"Beta"),
        "a subtree hidden by the page's own script is still in the agent's tree — this is \
         Wikipedia's 681 phantom list items in miniature — got {bag:?}"
    );
}
