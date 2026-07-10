//! `agent-run` — drive the headless agentic browser with a **single** API key.
//!
//! This is the committed runner. The local-only development harness
//! (`agent-run`) is a local-only tool and is gitignored; see CLAUDE.md.
//!
//! Usage:
//!   agent-run "<task>" <start-url>
//!
//! Reads `GROQ_API_KEY` (falling back to `GROQ_API_KEY` from a local `.env`) and
//! `GROQ_MODEL` (default `qwen/qwen3.6-27b`).

use anyhow::{bail, Context, Result};
use manuk_agent::{env, groq::GroqBackend, run_task, AgentBrowser, AgentConfig, InferenceBackend};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    env::load_dotenv();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        bail!("usage: agent-run \"<task>\" <start-url>");
    }
    let task = &args[0];
    let start_url = &args[1];

    let key =
        env::single_key().context("no API key: set GROQ_API_KEY (or GROQ_API_KEY in .env)")?;
    let backend = GroqBackend::from_key(key);
    println!("backend: {}", backend.name());

    let mut browser = AgentBrowser::new(1024, 768);
    if !browser.has_fonts() {
        eprintln!("warning: no system fonts; screenshots will have no text");
    }
    browser
        .navigate(start_url)
        .await
        .with_context(|| format!("loading start URL {start_url}"))?;

    let cfg = AgentConfig::default();
    let outcome = run_task(&mut browser, &backend, task, &cfg).await?;

    println!("\n--- transcript ({} steps) ---", outcome.steps);
    for line in &outcome.transcript {
        println!("{line}");
    }
    println!("\n--- answer ---");
    match &outcome.answer {
        Some(a) => println!("{a}"),
        None => println!("(no answer within {} steps)", cfg.max_steps),
    }
    Ok(())
}
