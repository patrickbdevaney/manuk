//! `drive-probe` — of the targets an agent can PERCEIVE on a real page, how many can it ACT on?
//!
//! `g_agent_drive_loop` (t1455) closed the perceive → ground → actuate → observe loop on a hermetic
//! fixture and named its own gap: *"what this does not prove is that a real page's markup is
//! reachable this way."* This is that measurement.
//!
//! ```text
//!   drive-probe <url>...          # one row per url, then a TOTAL
//!   drive-probe --list <url>      # every non-drivable target, with its verdict
//! ```
//!
//! See [`manuk_agent::drivability`] for what each verdict means. The measurement is local by
//! design: the question is not whether our tree matches Chrome's — [`a11y-score`] asks that — but
//! whether our own tree is actionable by our own agent.

use anyhow::{bail, Result};
use manuk_agent::drivability::{self, classify, targets, Verdict};
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

    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let list = args.first().map(|a| a == "--list").unwrap_or(false);
    if list {
        args.remove(0);
    }
    if args.is_empty() {
        bail!("usage: drive-probe [--list] <url>...");
    }

    println!(
        "{:<40} {:>7} {:>9} {:>10} {:>8} {:>8} {:>10} {:>10} {:>9}",
        "url",
        "targets",
        "drivable",
        "ambiguous",
        "mis-hit",
        "rate",
        "+landmark",
        "+heading",
        "ceiling"
    );
    let mut total = drivability::Tally::default();

    for url in &args {
        let mut browser = AgentBrowser::new(1024, 768);
        let tree = match browser
            .navigate(url)
            .await
            .and_then(|_| browser.a11y_tree())
        {
            Ok(t) => t,
            Err(e) => {
                println!("{url:<40} FAILED: {e}");
                continue;
            }
        };
        let t = drivability::tally(&tree);
        println!(
            "{:<40} {:>7} {:>9} {:>10} {:>8} {:>7.1}% {:>9.1}% {:>9.1}% {:>8.1}%",
            url.chars().take(40).collect::<String>(),
            t.total,
            t.drivable,
            t.ambiguous,
            t.mishit,
            t.rate() * 100.0,
            t.scoped_rate() * 100.0,
            t.sectioned_rate() * 100.0,
            t.ordinal_rate() * 100.0
        );

        if list {
            let ts = targets(&tree);
            let mut counts = std::collections::HashMap::<(String, String), usize>::new();
            for x in &ts {
                *counts
                    .entry((x.role.as_str().to_string(), x.name.trim().to_string()))
                    .or_insert(0) += 1;
            }
            for x in &ts {
                let d = counts[&(x.role.as_str().to_string(), x.name.trim().to_string())];
                let v = classify(&tree, x, d);
                if v != Verdict::Drivable {
                    println!(
                        "    {:<11} {:<10} {:?}",
                        format!("{v:?}"),
                        x.role.as_str(),
                        x.name.chars().take(60).collect::<String>()
                    );
                }
            }
        }

        total.total += t.total;
        total.drivable += t.drivable;
        total.ungrounded += t.ungrounded;
        total.ambiguous += t.ambiguous;
        total.mishit += t.mishit;
        total.drivable_scoped += t.drivable_scoped;
        total.drivable_sectioned += t.drivable_sectioned;
        total.drivable_ordinal += t.drivable_ordinal;
    }

    println!(
        "{:<40} {:>7} {:>9} {:>10} {:>8} {:>7.1}% {:>9.1}% {:>9.1}% {:>8.1}%",
        "TOTAL",
        total.total,
        total.drivable,
        total.ambiguous,
        total.mishit,
        total.rate() * 100.0,
        total.scoped_rate() * 100.0,
        total.sectioned_rate() * 100.0,
        total.ordinal_rate() * 100.0
    );
    Ok(())
}
