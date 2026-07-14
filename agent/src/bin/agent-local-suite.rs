//! `agent-local-suite` — INFERENCE.MD §2's **acceptance gate**.
//!
//! Runs the *same four capabilities* the Groq baseline cleared (text-extraction,
//! link-comprehension, link-navigation, multimodal-screenshot) against a **locally
//! launched `llama-server`**, through the keyless [`OpenAiCompatBackend::local_llama`]
//! preset from §1.
//!
//!   ./scripts/setup-local-model.sh --model qwen3.5-4b-q4km --port 8081
//!   cargo run -p manuk-agent --bin agent-local-suite -- --port 8081 --key qwen3.5-4b-q4km
//!
//! It reuses `run_task` unchanged — that is the point of §4c's trait seam.
//!
//! **Reporting rule (the directive is explicit):** print the pass rate against the
//! Groq/qwen3.6-27B baseline of **4/4**, and *state plainly if it does not clear it*.
//! Nothing is rounded up.

use std::io::Write;

use anyhow::{bail, Result};
use manuk_agent::local::OpenAiCompatBackend;
use manuk_agent::{model_manifest, run_task, AgentBrowser, AgentConfig, InferenceBackend};

struct Case {
    name: &'static str,
    task: &'static str,
    start: String,
    screenshots: bool,
    max_steps: usize,
    expect_any: &'static [&'static str],
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = flag(&args, "--port")
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let key = flag(&args, "--key").unwrap_or_else(|| model_manifest::DEFAULT_KEY.to_string());

    let entry = model_manifest::by_key(&key).ok_or_else(|| {
        anyhow::anyhow!("unknown model key {key}; see --list on the setup script")
    })?;

    let backend = OpenAiCompatBackend::local_llama(port);
    // Resolving the model proves the server is up AND that §1's `/v1/models` fallback works
    // against a real server, with no API key.
    let probe = backend
        .complete(&[manuk_agent::Message::text(
            manuk_agent::Role::User,
            "reply with the single word: ok",
        )])
        .await;
    if let Err(e) = &probe {
        bail!(
            "no llama-server on 127.0.0.1:{port} ({e:#}). Run scripts/setup-local-model.sh first."
        );
    }

    println!("local suite: {} [{}]", entry.name, entry.key);
    println!("  repo:    {}", entry.repo);
    println!("  quant:   {}  [{}]", entry.gguf, entry.source);
    println!("  backend: {} (no API key)", backend.name());
    println!("  baseline to clear: Groq/qwen3.6-27B scored 4/4\n");

    let tmp = std::env::temp_dir();
    let link_fixture = tmp.join("manuk-local-links.html");
    std::fs::write(
        &link_fixture,
        r#"<title>Link Hub</title><body><h1>Hub</h1><ul>
        <li><a href="https://www.rust-lang.org/">Rust</a></li>
        <li><a href="https://example.com/">Example Domain site</a></li>
        <li><a href="https://www.iana.org/">IANA</a></li>
        </ul></body>"#,
    )?;
    let link_url = format!("file://{}", link_fixture.display());

    let cases = vec![
        Case {
            name: "text-extraction",
            task: "What is the main heading (H1) text of this page? Reply with just the heading text.",
            start: "https://example.com/".to_string(),
            screenshots: false,
            max_steps: 3,
            expect_any: &["example domain"],
        },
        Case {
            name: "link-comprehension",
            task: "Among the links on this page, which link text points to example.com? Reply with the link text.",
            start: link_url.clone(),
            screenshots: false,
            max_steps: 3,
            expect_any: &["example domain site", "example"],
        },
        Case {
            name: "link-navigation",
            task: "Click the link that goes to example.com, then report the TITLE of the page you land on.",
            start: link_url.clone(),
            screenshots: false,
            max_steps: 5,
            expect_any: &["example domain"],
        },
        Case {
            name: "multimodal-screenshot",
            task: "Look ONLY at the screenshot. Is the page's overall color theme light or dark? Answer with one word: light or dark.",
            start: "https://example.com/".to_string(),
            screenshots: true,
            max_steps: 2,
            expect_any: &["light"],
        },
    ];

    let mut passed = 0usize;
    for (i, case) in cases.iter().enumerate() {
        print!("[{}/{}] {:<22} ... ", i + 1, cases.len(), case.name);
        std::io::stdout().flush().ok();

        let mut browser = AgentBrowser::new(1024, 768);
        if let Err(e) = browser.navigate(&case.start).await {
            println!("SETUP-FAIL ({e:#})");
            continue;
        }
        let cfg = AgentConfig {
            max_steps: case.max_steps,
            send_screenshots: case.screenshots,
            text_budget: 1500,
            ..AgentConfig::default()
        };
        match run_task(&mut browser, &backend, case.task, &cfg).await {
            Ok(outcome) => {
                let ans = outcome.answer.unwrap_or_default();
                let low = ans.to_lowercase();
                let ok = case
                    .expect_any
                    .iter()
                    .any(|e| low.contains(&e.to_lowercase()));
                if ok {
                    passed += 1;
                    println!("PASS  ({} steps)  answer: {}", outcome.steps, oneline(&ans));
                } else {
                    println!("FAIL  ({} steps)  answer: {}", outcome.steps, oneline(&ans));
                }
            }
            Err(e) => println!("ERROR ({e:#})"),
        }
    }

    let n = cases.len();
    println!("\n{key}: {passed}/{n} capabilities passed");
    if passed == n {
        println!("clears the Groq/qwen3.6-27B baseline of 4/4.");
    } else {
        // The directive: state plainly if it does not clear the bar. Do not round up.
        println!("DOES NOT clear the Groq/qwen3.6-27B baseline of 4/4.");
    }
    Ok(())
}

fn oneline(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect()
}
