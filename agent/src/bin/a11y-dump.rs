//! `a11y-dump` — the agent's OWN accessibility tree for a real URL, as JSON.
//!
//! **The a11y tree has never been measured against a real site.** Track B's `>=90% node match` bar is
//! quoted from the WPT `wai-aria` / `html-aam` / `accname` suites (91.9% / 92.5% / …), and constitution
//! check #131 already recorded why that is not the same claim: Interop 2026 lists accessibility testing
//! as an **investigation effort**, which is the platform saying *there is no suite that can decide this
//! yet*. Two ticks (t1379, t1380) then found shipping defects — a name fragment hidden by a STYLESHEET
//! being announced, an entire `display:none` subtree living in the tree — that moved the WPT rows by
//! **zero**, because every hidden-node fixture in the suite writes `style="display:none"` INLINE.
//!
//! `docs/loop/V1-SCOPE.md`'s completion bar for the agentic surface is *"drives the same top-N sites a
//! human daily-drives, **measured vs the same real-site corpus**"*. This is the manuk half of that
//! measurement; the Chrome half is CDP `Accessibility.getFullAXTree`.
//!
//! ```text
//!   a11y-dump <url>                       # JSON array of semantic nodes, document order
//! ```
//!
//! Emits every node the tree holds — including the `generic` containers a comparison will want to drop
//! — because *which* nodes to drop is a judgement the comparison makes and this dump must not make for
//! it. Dropping them here would bake the modelling difference into the raw data, where nothing could
//! later question it.

use anyhow::{bail, Context, Result};
use manuk_agent::AgentBrowser;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("error")),
        )
        .with_writer(std::io::stderr)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        bail!("usage: a11y-dump <url>");
    }
    let url = &args[0];

    let mut browser = AgentBrowser::new(1024, 768);
    browser
        .navigate(url)
        .await
        .with_context(|| format!("loading {url}"))?;

    let tree = browser.a11y_tree().context("building the a11y tree")?;
    let nodes: Vec<serde_json::Value> = tree
        .iter()
        .map(|n| {
            serde_json::json!({
                "role": n.role.as_str(),
                "name": n.name,
                "x": n.bbox.map(|b| b.x),
                "y": n.bbox.map(|b| b.y),
                "w": n.bbox.map(|b| b.width),
                "h": n.bbox.map(|b| b.height),
                "state": n.state.render(),
            })
        })
        .collect();
    println!("{}", serde_json::to_string(&nodes)?);
    Ok(())
}
