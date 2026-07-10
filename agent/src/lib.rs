//! manuk-agent — the headless agentic browser.
//!
//! CLAUDE.md's agent phase, kept strictly layered so the pieces are independently
//! testable and swappable:
//!
//! - [`AgentBrowser`] — a **headless page driver** over the shared `manuk-page`
//!   pipeline. It knows nothing about LLMs: navigate, scroll, screenshot, observe.
//! - [`InferenceBackend`] — the **model abstraction**. The agent loop talks to this
//!   trait, never to a specific provider. [`groq::GroqBackend`] is one impl.
//! - [`run_task`] — the **agent loop** (observe → decide → act). It takes a
//!   `&dyn InferenceBackend` and `&mut AgentBrowser`; it has no dependency on Groq
//!   or on the test harness that drives it.
//!
//! This decoupling is the point: the parallel key harness, the single-key
//! runner, and any future backend all reuse the same `run_task` unchanged.

use anyhow::{anyhow, Context, Result};
use manuk_page::{fetch_html, Link, Page};
use manuk_text::FontContext;

pub mod env;
pub mod groq;

/// Default model — a Groq-hosted multimodal model (overridable via `GROQ_MODEL`).
pub const DEFAULT_MODEL: &str = "qwen/qwen3.6-27b";

// ---------------------------------------------------------------------------
// Inference backend abstraction (provider-agnostic)
// ---------------------------------------------------------------------------

/// A single piece of message content: text or an inline PNG image (multimodal).
#[derive(Clone, Debug)]
pub enum Content {
    Text(String),
    ImagePng(Vec<u8>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

/// A chat message: a role plus one or more content parts.
#[derive(Clone, Debug)]
pub struct Message {
    pub role: Role,
    pub content: Vec<Content>,
}

impl Message {
    pub fn text(role: Role, text: impl Into<String>) -> Self {
        Message {
            role,
            content: vec![Content::Text(text.into())],
        }
    }
}

/// A model that can complete a chat conversation. Provider-agnostic and
/// object-safe so the agent loop can hold `&dyn InferenceBackend`.
#[async_trait::async_trait]
pub trait InferenceBackend: Send + Sync {
    /// Complete `messages`, returning the assistant's raw text.
    async fn complete(&self, messages: &[Message]) -> Result<String>;
    /// Identifier for logs/about pages, e.g. "groq:qwen/qwen3.6-27b".
    fn name(&self) -> String;
    /// Whether this backend accepts image content.
    fn supports_images(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Headless page driver
// ---------------------------------------------------------------------------

/// A headless browser the agent drives. Renders with the CPU raster tier, so it is
/// display-free and deterministic.
pub struct AgentBrowser {
    fonts: FontContext,
    width: u32,
    height: u32,
    page: Option<Page>,
    scroll_y: f32,
}

/// What the agent perceives after an action: the textual channel plus enough
/// structure to act (links by index). The screenshot is fetched separately via
/// [`AgentBrowser::screenshot_png`].
#[derive(Clone, Debug)]
pub struct Observation {
    pub url: String,
    pub title: String,
    pub text: String,
    pub links: Vec<Link>,
    pub scroll_y: f32,
    pub content_height: f32,
    pub viewport: (u32, u32),
}

impl Observation {
    /// A compact human/model-readable rendering of the observation.
    pub fn to_prompt(&self, text_budget: usize) -> String {
        use std::fmt::Write as _;
        let mut s = String::new();
        let _ = writeln!(s, "URL: {}", self.url);
        let _ = writeln!(s, "TITLE: {}", self.title);
        let _ = writeln!(
            s,
            "SCROLL: {:.0}/{:.0}px  VIEWPORT: {}x{}",
            self.scroll_y, self.content_height, self.viewport.0, self.viewport.1
        );
        if self.links.is_empty() {
            let _ = writeln!(s, "LINKS: (none)");
        } else {
            let _ = writeln!(s, "LINKS (index: text -> href):");
            for (i, l) in self.links.iter().enumerate().take(40) {
                let t = if l.text.is_empty() { "(no text)" } else { &l.text };
                let _ = writeln!(s, "  {i}: {t} -> {}", l.href);
            }
        }
        let text: String = self.text.chars().take(text_budget).collect();
        let _ = writeln!(s, "VISIBLE TEXT:\n{text}");
        s
    }
}

impl AgentBrowser {
    pub fn new(width: u32, height: u32) -> Self {
        AgentBrowser {
            fonts: FontContext::new(),
            width: width.max(1),
            height: height.max(1),
            page: None,
            scroll_y: 0.0,
        }
    }

    pub fn has_fonts(&self) -> bool {
        self.fonts.face_count() > 0
    }

    /// Load `url` (http(s)/file/path) and lay it out at the current width.
    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        let (html, final_url) = fetch_html(url).await?;
        let page = Page::load(&html, &final_url, &self.fonts, self.width as f32);
        self.scroll_y = 0.0;
        self.page = Some(page);
        Ok(())
    }

    /// Scroll by `dy` px, clamped to the page.
    pub fn scroll_by(&mut self, dy: f32) {
        let max = self
            .page
            .as_ref()
            .map(|p| (p.content_height - self.height as f32).max(0.0))
            .unwrap_or(0.0);
        self.scroll_y = (self.scroll_y + dy).clamp(0.0, max);
    }

    /// PNG screenshot of the current viewport.
    pub fn screenshot_png(&self) -> Result<Vec<u8>> {
        let page = self.page.as_ref().context("no page loaded")?;
        page.paint_scrolled(&self.fonts, self.width, self.height, self.scroll_y)
            .encode_png()
    }

    /// The current observation.
    pub fn observe(&self) -> Result<Observation> {
        let page = self.page.as_ref().context("no page loaded")?;
        Ok(Observation {
            url: page.final_url.clone(),
            title: page.title.clone(),
            text: page.visible_text(),
            links: page.links(),
            scroll_y: self.scroll_y,
            content_height: page.content_height,
            viewport: (self.width, self.height),
        })
    }
}

// ---------------------------------------------------------------------------
// Agent loop
// ---------------------------------------------------------------------------

/// An action the model can take. Deserialized from the model's JSON.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// Load a URL directly.
    Navigate { url: String },
    /// Click the link at the given index in the last observation.
    Click { index: usize },
    /// Scroll by `dy` px (positive = down).
    Scroll {
        #[serde(default)]
        dy: f32,
    },
    /// Finish the task with an answer.
    Finish {
        #[serde(default)]
        answer: String,
    },
}

/// Agent loop configuration.
#[derive(Clone, Debug)]
pub struct AgentConfig {
    pub max_steps: usize,
    /// Attach a screenshot to each observation (requires a multimodal backend).
    pub send_screenshots: bool,
    /// Max characters of page text per observation.
    pub text_budget: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        AgentConfig {
            max_steps: 8,
            send_screenshots: true,
            text_budget: 2000,
        }
    }
}

/// The result of an agent run.
#[derive(Clone, Debug, Default)]
pub struct AgentOutcome {
    pub answer: Option<String>,
    pub steps: usize,
    pub transcript: Vec<String>,
}

const SYSTEM_PROMPT: &str = "\
You are an autonomous web-browsing agent controlling a headless browser. Each turn \
you receive an observation (URL, title, links by index, visible text, and possibly a \
screenshot). Decide the single next action and reply with ONE JSON object, nothing \
else. Any reasoning must stay inside <think>...</think> tags; the JSON must be the \
last thing you output.

Available actions:
  {\"action\":\"navigate\",\"url\":\"https://...\"}   - load a URL
  {\"action\":\"click\",\"index\":N}                  - click link N from the observation
  {\"action\":\"scroll\",\"dy\":600}                  - scroll down (or negative to go up)
  {\"action\":\"finish\",\"answer\":\"...\"}           - end the task with your answer

Finish as soon as you can answer the task. Do not invent links; only click indices \
that appear in the observation.";

/// Run `task` to completion (or `max_steps`) with the given backend + browser.
///
/// This function is the decoupling seam: it never mentions a provider or a harness.
pub async fn run_task(
    browser: &mut AgentBrowser,
    backend: &dyn InferenceBackend,
    task: &str,
    config: &AgentConfig,
) -> Result<AgentOutcome> {
    let mut outcome = AgentOutcome::default();
    // Conversation memory: (observation_text, action_json) per prior step.
    let mut history: Vec<(String, String)> = Vec::new();

    for step in 0..config.max_steps {
        outcome.steps = step + 1;
        let obs = browser.observe()?;
        let obs_text = obs.to_prompt(config.text_budget);

        // Rebuild messages: system, prior turns (text only), then the current
        // observation (with a screenshot if enabled + supported).
        let mut messages = vec![
            Message::text(Role::System, SYSTEM_PROMPT),
            Message::text(Role::User, format!("TASK: {task}")),
        ];
        for (past_obs, past_act) in &history {
            messages.push(Message::text(Role::User, past_obs.clone()));
            messages.push(Message::text(Role::Assistant, past_act.clone()));
        }
        let mut current = vec![Content::Text(format!(
            "OBSERVATION (step {}):\n{obs_text}\nRespond with the next action as JSON.",
            step + 1
        ))];
        if config.send_screenshots && backend.supports_images() {
            if let Ok(png) = browser.screenshot_png() {
                current.push(Content::ImagePng(png));
            }
        }
        messages.push(Message {
            role: Role::User,
            content: current,
        });

        let reply = backend
            .complete(&messages)
            .await
            .with_context(|| format!("inference failed at step {}", step + 1))?;

        let action = match parse_action(&reply) {
            Ok(a) => a,
            Err(e) => {
                outcome
                    .transcript
                    .push(format!("step {}: unparseable reply ({e})", step + 1));
                // Record and continue so the next observation re-prompts the model.
                history.push((obs_text, reply));
                continue;
            }
        };

        let action_json = strip_think(&reply).trim().to_string();
        outcome
            .transcript
            .push(format!("step {}: {action:?}", step + 1));

        match action {
            Action::Finish { answer } => {
                outcome.answer = Some(answer);
                return Ok(outcome);
            }
            Action::Navigate { url } => {
                if let Err(e) = browser.navigate(&url).await {
                    outcome.transcript.push(format!("  navigate error: {e:#}"));
                }
            }
            Action::Click { index } => {
                let href = obs.links.get(index).map(|l| l.href.clone());
                match href {
                    Some(h) => {
                        if let Err(e) = browser.navigate(&h).await {
                            outcome.transcript.push(format!("  click error: {e:#}"));
                        }
                    }
                    None => outcome
                        .transcript
                        .push(format!("  no link at index {index}")),
                }
            }
            Action::Scroll { dy } => browser.scroll_by(dy),
        }

        history.push((obs_text, action_json));
    }

    Ok(outcome)
}

/// Remove `<think>...</think>` reasoning blocks (qwen/DeepSeek-style models emit
/// them). Handles an unclosed trailing `<think>`.
pub fn strip_think(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let lower = s.to_ascii_lowercase();
    let mut i = 0;
    while i < s.len() {
        if lower[i..].starts_with("<think>") {
            match lower[i..].find("</think>") {
                Some(end) => i += end + "</think>".len(),
                None => break, // unclosed: drop the rest
            }
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Extract and parse an [`Action`] from a model reply: strip reasoning, then take
/// the last balanced `{...}` object and deserialize it.
pub fn parse_action(reply: &str) -> Result<Action> {
    let cleaned = strip_think(reply);
    let json =
        last_json_object(&cleaned).ok_or_else(|| anyhow!("no JSON object found in reply"))?;
    serde_json::from_str::<Action>(&json).with_context(|| format!("bad action JSON: {json}"))
}

/// Find the last top-level `{...}` object in `s` by scanning backward for a `}` and
/// matching braces (string-aware).
fn last_json_object(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let end = bytes.iter().rposition(|&b| b == b'}')?;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    let mut i = end as isize;
    while i >= 0 {
        let c = bytes[i as usize];
        if in_str {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_str = false;
            }
        } else {
            match c {
                b'}' => depth += 1,
                b'{' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(s[i as usize..=end].to_string());
                    }
                }
                b'"' => in_str = true,
                _ => {}
            }
        }
        i -= 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_think_blocks() {
        let r = "<think>let me reason\nabout it</think> {\"action\":\"finish\"}";
        assert_eq!(strip_think(r).trim(), "{\"action\":\"finish\"}");
    }

    #[test]
    fn parses_actions_after_reasoning() {
        let r = "<think>I should scroll</think>\nHere: {\"action\":\"scroll\",\"dy\":600}";
        match parse_action(r).unwrap() {
            Action::Scroll { dy } => assert_eq!(dy, 600.0),
            other => panic!("wrong action: {other:?}"),
        }
    }

    #[test]
    fn parses_click_and_navigate_and_finish() {
        assert!(matches!(
            parse_action("{\"action\":\"click\",\"index\":3}").unwrap(),
            Action::Click { index: 3 }
        ));
        assert!(matches!(
            parse_action("blah {\"action\":\"navigate\",\"url\":\"https://x.test/\"}").unwrap(),
            Action::Navigate { .. }
        ));
        assert!(matches!(
            parse_action("{\"action\":\"finish\",\"answer\":\"42\"}").unwrap(),
            Action::Finish { .. }
        ));
    }

    #[test]
    fn picks_last_json_object() {
        let s = "{\"action\":\"scroll\"} then final {\"action\":\"finish\",\"answer\":\"ok\"}";
        assert!(matches!(parse_action(s).unwrap(), Action::Finish { .. }));
    }

    #[tokio::test]
    async fn driver_navigates_local_file() {
        let path = std::env::temp_dir().join("manuk-agent-test.html");
        std::fs::write(
            &path,
            "<title>T</title><body><p>hello <a href='https://e.test/x'>go</a></p></body>",
        )
        .unwrap();
        let mut b = AgentBrowser::new(400, 300);
        b.navigate(&format!("file://{}", path.display()))
            .await
            .unwrap();
        let obs = b.observe().unwrap();
        assert_eq!(obs.title, "T");
        assert!(obs.text.contains("hello"));
        assert_eq!(obs.links.len(), 1);
        assert_eq!(obs.links[0].href, "https://e.test/x");
        let png = b.screenshot_png().unwrap();
        assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
    }
}
