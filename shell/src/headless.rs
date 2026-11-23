//! W6.H1.a: a windowless entry point for the W6.H1 A/B (`manuk agent-task <url> <task> --driver
//! taskrun|hub`). It runs the OLD shell agent (`AgentRun`/`TaskRun`) or the NEW one (`HubRun`/`Hub`)
//! to completion with no window and no human gesture, and prints the answer and the per-step
//! transcript, so the two paths can be batch-measured on the same tasks.
//!
//! Both drivers are the SAME state machines the window pumps (not copies), started through
//! `start_headless`, which holds no `HandoffConsent`: nothing here can mint one.

use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use manuk_agent::{AgentConfig, Handoff, InferenceBackend};
use manuk_text::FontContext;

use crate::agent_run::{AgentRun, Phase};
use crate::hub_run::HubRun;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Driver {
    TaskRun,
    Hub,
}

impl Driver {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "taskrun" => Ok(Driver::TaskRun),
            "hub" => Ok(Driver::Hub),
            o => bail!("--driver {o}: want taskrun|hub"),
        }
    }
}

#[derive(Debug)]
pub struct Outcome {
    pub driver: Driver,
    pub phase: Phase,
    pub steps: usize,
    pub transcript: Vec<String>,
}

impl Outcome {
    pub fn answer(&self) -> Option<&str> {
        match &self.phase {
            Phase::Done(a) => Some(a),
            _ => None,
        }
    }
}

/// Fetch `url` (http(s), file://, or a local path) into a live page, as `manuk render` does.
pub fn load_handoff(url: &str, width: u32) -> Result<Handoff> {
    let url = if url.contains("://") || url.starts_with("data:") {
        url.to_string()
    } else {
        let p = std::fs::canonicalize(url).with_context(|| format!("{url}: not a URL or file"))?;
        format!("file://{}", p.display())
    };
    let rt = manuk_net::runtime();
    let fonts = FontContext::new();
    let (mut page, _) = rt.block_on(manuk_page::fetch_streaming_page(&url, &fonts, width as f32))?;
    rt.block_on(page.fetch_and_apply_stylesheets(&fonts, width as f32));
    rt.block_on(page.fetch_and_apply_images(&fonts, width as f32));
    let final_url = page.final_url.clone();
    Ok(Handoff {
        page,
        scroll_y: 0.0,
        history: vec![final_url],
    })
}

enum Run {
    Task(Box<AgentRun>),
    Hub(Box<HubRun>),
}

/// Drive `handoff`'s page with `driver` until the run ends or `budget` passes.
pub fn run(
    driver: Driver,
    task: &str,
    handoff: Handoff,
    config: AgentConfig,
    backend: Box<dyn InferenceBackend>,
    size: (u32, u32),
    budget: Duration,
) -> Outcome {
    let rt = manuk_net::runtime();
    let mut r = match driver {
        Driver::TaskRun => Run::Task(Box::new(AgentRun::start_headless(
            task, handoff, config, backend, size,
        ))),
        Driver::Hub => Run::Hub(Box::new(HubRun::start_headless(
            task, handoff, config, backend, size,
        ))),
    };
    let deadline = Instant::now() + budget;
    loop {
        let (active, timed_out) = match &mut r {
            Run::Task(a) => {
                a.pump(rt, None);
                (a.is_active(), false)
            }
            Run::Hub(h) => {
                h.pump(rt, None);
                (h.is_active(), false)
            }
        };
        let _ = timed_out;
        if !active {
            break;
        }
        if Instant::now() >= deadline {
            match &mut r {
                Run::Task(a) => a.request_stop(),
                Run::Hub(h) => h.request_stop(),
            }
            // One more pump lets the stop land; the phase then reads Stopped.
            match &mut r {
                Run::Task(a) => a.pump(rt, None),
                Run::Hub(h) => h.pump(rt, None),
            };
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    match r {
        Run::Task(a) => Outcome {
            driver,
            phase: a.phase().clone(),
            steps: a.steps(),
            transcript: a.log_lines().to_vec(),
        },
        Run::Hub(h) => Outcome {
            driver,
            phase: h.phase().clone(),
            steps: h.steps(),
            transcript: h.log_lines().to_vec(),
        },
    }
}

/// `manuk agent-task <url> <task> [--driver taskrun|hub] [--script replies.json] [--max-steps N]
/// [--timeout-s N]`. With `--script` the model is a JSON array of verbatim replies (no network);
/// otherwise the panel's backend (local llama-server, else Groq) is used.
pub fn cmd(args: &[String]) -> Result<()> {
    let pos: Vec<&String> = {
        let mut v = Vec::new();
        let mut skip = false;
        for a in args {
            if skip {
                skip = false;
            } else if a.starts_with("--") {
                skip = true;
            } else {
                v.push(a);
            }
        }
        v
    };
    let (Some(url), Some(task)) = (pos.first(), pos.get(1)) else {
        bail!("agent-task: usage: manuk agent-task <url> <task> [--driver taskrun|hub] [--script f.json]");
    };
    let flag = |n: &str| crate::flag_value(args, &[n]);
    let driver = Driver::parse(flag("--driver").unwrap_or("hub"))?;
    let max_steps: usize = flag("--max-steps").and_then(|s| s.parse().ok()).unwrap_or(40);
    let budget = Duration::from_secs(flag("--timeout-s").and_then(|s| s.parse().ok()).unwrap_or(300));
    let mut config = crate::panel::PanelScope::assistant().agent_config();
    config.max_steps = max_steps;
    let backend: Box<dyn InferenceBackend> = match flag("--script") {
        Some(f) => {
            let replies: Vec<String> = serde_json::from_str(
                &std::fs::read_to_string(f).with_context(|| format!("{f}: unreadable"))?,
            )
            .with_context(|| format!("{f}: want a JSON array of reply strings"))?;
            Box::new(manuk_agent::replay::ReplayBackend::new(replies))
        }
        None => {
            let port = std::env::var("MANUK_LLAMA_PORT").ok().and_then(|s| s.parse().ok());
            manuk_agent::env::load_dotenv();
            let Some(kind) = crate::panel::resolve_panel_backend(
                port,
                manuk_agent::env::single_key().is_some(),
            ) else {
                bail!("agent-task: no model: set MANUK_LLAMA_PORT, put GROQ_API_KEY in .env, or pass --script");
            };
            Box::new(
                crate::panel::build_panel_backend(&kind).context("agent-task: backend key vanished")?,
            )
        }
    };
    let width = 1024;
    let handoff = load_handoff(url, width)?;
    let out = run(driver, task, handoff, config, backend, (width, 768), budget);
    for l in &out.transcript {
        println!("{l}");
    }
    println!("driver: {:?}  steps: {}", out.driver, out.steps);
    match &out.phase {
        Phase::Done(a) => {
            println!("ANSWER: {a}");
            Ok(())
        }
        p => bail!("agent-task did not finish: {p:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use manuk_agent::replay::ReplayBackend;

    const FIXTURE: &str = "<title>Fixture</title><body><h1>Price list</h1><p>The widget costs 42 dollars.</p>\
        <div style=\"height:3000px\">spacer</div><p>the end</p></body>";

    fn handoff() -> Handoff {
        let dir = std::env::temp_dir().join(format!("manuk-headless-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("fixture.html");
        std::fs::write(&f, FIXTURE).unwrap();
        load_handoff(f.to_str().unwrap(), 1024).unwrap()
    }

    fn go(d: Driver, replies: &[&str]) -> Outcome {
        let cfg = AgentConfig {
            max_steps: 8,
            ..AgentConfig::default()
        };
        run(
            d,
            "what does the widget cost?",
            handoff(),
            cfg,
            Box::new(ReplayBackend::new(replies.iter().map(|s| s.to_string()).collect())),
            (1024, 768),
            Duration::from_secs(60),
        )
    }

    #[test]
    fn both_drivers_finish_with_an_answer_headless() {
        let t = go(
            Driver::TaskRun,
            &[
                r#"{"action":"scroll","dy":120}"#,
                r#"{"action":"finish","answer":"42 dollars"}"#,
            ],
        );
        assert_eq!(t.answer(), Some("42 dollars"), "{:?}", t.transcript);
        let h = go(
            Driver::Hub,
            &[
                r#"{"tool":"scroll","args":{"dy":120}}"#,
                r#"{"tool":"finish","args":{"answer":"42 dollars"}}"#,
            ],
        );
        assert_eq!(h.answer(), Some("42 dollars"), "{:?}", h.transcript);
        // The Hub transcript names Hub tools: the opening `observe` result and the `scroll` call.
        let joined = h.transcript.join("\n");
        assert!(joined.contains("observe:"), "{joined}");
        assert!(joined.contains("scroll "), "{joined}");
    }

    /// A model reply the Hub must act on: if the driver ignored the scripted reply it would not
    /// reach the scroll tool or the scripted answer.
    #[test]
    fn hub_driver_follows_the_scripted_reply() {
        let h = go(
            Driver::Hub,
            &[
                r#"{"tool":"scroll","args":{"dy":300}}"#,
                r#"{"tool":"finish","args":{"answer":"from the script"}}"#,
            ],
        );
        assert_eq!(h.answer(), Some("from the script"));
        assert!(h.transcript.iter().any(|l| l.starts_with("scroll ")));
    }

    #[test]
    fn exhausted_script_is_a_failure_not_an_answer() {
        let h = go(Driver::Hub, &[]);
        assert!(h.answer().is_none(), "{:?}", h.phase);
    }
}
