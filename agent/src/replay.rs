//! G-d — **deterministic replay / provenance**.
//!
//! An agent run is only reproducible if you record the parts that are *not*
//! reproducible. Two things are non-deterministic: the **model** (same prompt, different
//! sampling) and the **network**. So the log records the model's raw responses verbatim,
//! and a replay feeds them back in order via [`ReplayBackend`] instead of calling the
//! model at all. Everything else — parse, style, layout, CPU raster — is deterministic
//! here, which is precisely why this works: the agent renders with `tiny-skia` on the
//! CPU and never touches a GPU, so **screenshot hashes are stable** where a GPU-backed
//! agent's would drift between machines and drivers.
//!
//! A replay therefore does two jobs at once:
//!
//! * **Provenance** — an append-only, auditable record of what the agent saw and did.
//! * **A regression harness** — in [`Strict`](ReplayMode::Strict) mode, any divergence
//!   between the recorded observation and the freshly computed one is an error. A green
//!   strict replay *is* the reproducibility proof.
//!
//! **The digest is not cryptographic.** [`digest`] is FNV-1a: a fast, stable checksum
//! for comparing two renderings of the same page. It is never used for security, so it
//! is not a hand-rolled crypto primitive (CLAUDE.md's prohibition stands: E2's at-rest
//! encryption uses audited AEAD crates and nothing here touches that path).
//!
//! **Documented gaps (not faked):** the network is *not* recorded, so replaying a run
//! against a live site that has changed will (correctly) report divergence — the log
//! proves what was seen, it does not resurrect the server. Recording a response cache is
//! the tracked follow-up.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{Action, InferenceBackend, Message, Observation};

/// A stable, non-cryptographic 64-bit digest (FNV-1a). Used only to compare a recorded
/// rendering with a replayed one; never for security.
pub fn digest(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

/// A recorded observation — the parts that must match on replay.
///
/// The full page text is *not* stored (it can be megabytes); its digest is. The
/// accessibility tree is stored in full because it is small, semantic, and the most
/// useful thing to read when a replay diverges.
// No `Eq`: the record carries `f32` scroll/height offsets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservationRecord {
    pub url: String,
    pub title: String,
    pub text_digest: u64,
    pub semantics: Vec<String>,
    /// Digest of the CPU-rendered PNG. Stable because the raster is display-free.
    pub screenshot_digest: Option<u64>,
    pub scroll_y: f32,
    pub content_height: f32,
}

impl ObservationRecord {
    pub fn of(obs: &Observation, screenshot: Option<&[u8]>) -> Self {
        ObservationRecord {
            url: obs.url.clone(),
            title: obs.title.clone(),
            text_digest: digest(obs.text.as_bytes()),
            semantics: obs.semantics.clone(),
            screenshot_digest: screenshot.map(digest),
            scroll_y: obs.scroll_y,
            content_height: obs.content_height,
        }
    }

    /// The first field that differs, as a human-readable reason.
    fn diff(&self, other: &ObservationRecord) -> Option<String> {
        if self.url != other.url {
            return Some(format!("url: {:?} -> {:?}", self.url, other.url));
        }
        if self.title != other.title {
            return Some(format!("title: {:?} -> {:?}", self.title, other.title));
        }
        if self.text_digest != other.text_digest {
            return Some(format!(
                "page text digest: {:#x} -> {:#x}",
                self.text_digest, other.text_digest
            ));
        }
        if self.semantics != other.semantics {
            return Some("accessibility tree differs".to_string());
        }
        if self.screenshot_digest != other.screenshot_digest {
            return Some(format!(
                "screenshot digest: {:?} -> {:?}",
                self.screenshot_digest, other.screenshot_digest
            ));
        }
        if self.scroll_y != other.scroll_y {
            return Some(format!("scroll_y: {} -> {}", self.scroll_y, other.scroll_y));
        }
        None
    }
}

/// One entry in the append-only log. Order is the record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// The run's parameters, written once at the head.
    Start {
        task: String,
        backend: String,
        max_steps: usize,
    },
    /// What the agent perceived at the start of a step.
    Observed {
        step: usize,
        observation: Box<ObservationRecord>,
    },
    /// The model's **raw** reply. This is the non-reproducible part, so it is verbatim.
    Model { step: usize, raw: String },
    /// The action parsed from that reply (or the parse error).
    Acted { step: usize, action: String },
    /// The Action-Guard refused the action.
    Blocked { step: usize, reason: String },
    /// The run ended.
    Finished {
        steps: usize,
        answer: Option<String>,
    },
}

/// An append-only event log. Serializes as **JSON Lines** so it can be appended to a
/// file and tailed, and so a truncated log is still parseable up to the last whole line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventLog {
    events: Vec<Event>,
}

impl EventLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, e: Event) {
        self.events.push(e);
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// The model replies, in order — what a replay feeds back.
    pub fn model_replies(&self) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|e| match e {
                Event::Model { raw, .. } => Some(raw.clone()),
                _ => None,
            })
            .collect()
    }

    /// The recorded observation for `step`, if any.
    pub fn observation(&self, step: usize) -> Option<&ObservationRecord> {
        self.events.iter().find_map(|e| match e {
            Event::Observed {
                step: s,
                observation,
            } if *s == step => Some(&**observation),
            _ => None,
        })
    }

    pub fn to_jsonl(&self) -> String {
        self.events
            .iter()
            .map(|e| serde_json::to_string(e).expect("Event is serializable"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Parse JSON Lines. Blank lines are skipped; a malformed line is an error naming
    /// its number, because a silently-dropped line would corrupt the replay order.
    pub fn from_jsonl(s: &str) -> Result<Self> {
        let mut events = Vec::new();
        for (i, line) in s.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            events.push(
                serde_json::from_str(line)
                    .with_context(|| format!("malformed event on line {}", i + 1))?,
            );
        }
        Ok(EventLog { events })
    }
}

/// An [`InferenceBackend`] that replays recorded model replies instead of calling a
/// model. Running out of replies is an error, not a silent stop.
pub struct ReplayBackend {
    replies: std::sync::Mutex<std::collections::VecDeque<String>>,
    name: String,
}

impl ReplayBackend {
    pub fn new(replies: Vec<String>) -> Self {
        ReplayBackend {
            replies: std::sync::Mutex::new(replies.into()),
            name: "replay".to_string(),
        }
    }

    pub fn from_log(log: &EventLog) -> Self {
        ReplayBackend::new(log.model_replies())
    }

    pub fn remaining(&self) -> usize {
        self.replies.lock().expect("replay lock").len()
    }
}

#[async_trait::async_trait]
impl InferenceBackend for ReplayBackend {
    async fn complete(&self, _messages: &[Message]) -> Result<String> {
        self.replies
            .lock()
            .expect("replay lock")
            .pop_front()
            .context(
                "replay log exhausted: the run asked for more model replies than were recorded",
            )
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn supports_images(&self) -> bool {
        true
    }
}

/// How strictly a replay checks itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayMode {
    /// Any divergence between a recorded and a replayed observation is an error.
    Strict,
    /// Divergences are collected and reported, but the replay continues.
    Lenient,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReplayReport {
    pub steps_checked: usize,
    /// `(step, reason)` for each observation that did not match.
    pub divergences: Vec<(usize, String)>,
}

impl ReplayReport {
    pub fn is_identical(&self) -> bool {
        self.divergences.is_empty()
    }
}

/// Compare a fresh observation against the recorded one for `step`.
///
/// In [`ReplayMode::Strict`] the first divergence aborts. A green strict replay is the
/// reproducibility proof G-d exists to provide.
pub fn check_step(
    log: &EventLog,
    step: usize,
    fresh: &ObservationRecord,
    mode: ReplayMode,
    report: &mut ReplayReport,
) -> Result<()> {
    let Some(recorded) = log.observation(step) else {
        bail!("no recorded observation for step {step}");
    };
    report.steps_checked += 1;
    if let Some(reason) = recorded.diff(fresh) {
        if mode == ReplayMode::Strict {
            bail!("replay diverged at step {step}: {reason}");
        }
        report.divergences.push((step, reason));
    }
    Ok(())
}

/// Render `action` the way the log stores it, so recorded and replayed actions compare.
pub fn action_repr(action: &Action) -> String {
    format!("{action:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs_rec(url: &str, text: &str) -> ObservationRecord {
        ObservationRecord {
            url: url.to_string(),
            title: "T".into(),
            text_digest: digest(text.as_bytes()),
            semantics: vec!["heading level 1 \"Hi\"".into()],
            screenshot_digest: Some(42),
            scroll_y: 0.0,
            content_height: 100.0,
        }
    }

    #[test]
    fn digest_is_stable_and_distinguishes_inputs() {
        // FNV-1a of the empty string is its offset basis.
        assert_eq!(digest(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(digest(b"hello"), digest(b"hello"));
        assert_ne!(digest(b"hello"), digest(b"hellp"));
    }

    #[test]
    fn jsonl_round_trips_and_preserves_order() {
        let mut log = EventLog::new();
        log.push(Event::Start {
            task: "find the price".into(),
            backend: "groq:x".into(),
            max_steps: 3,
        });
        log.push(Event::Observed {
            step: 0,
            observation: Box::new(obs_rec("https://a.test/", "hello")),
        });
        log.push(Event::Model {
            step: 0,
            raw: "{\"action\":\"scroll\",\"dy\":600}".into(),
        });
        log.push(Event::Acted {
            step: 0,
            action: "Scroll { dy: 600.0 }".into(),
        });
        log.push(Event::Finished {
            steps: 1,
            answer: Some("42".into()),
        });

        let text = log.to_jsonl();
        assert_eq!(text.lines().count(), 5, "one JSON object per line");
        let back = EventLog::from_jsonl(&text).unwrap();
        assert_eq!(back, log);
    }

    #[test]
    fn a_truncated_log_parses_up_to_the_last_whole_line() {
        let mut log = EventLog::new();
        log.push(Event::Model {
            step: 0,
            raw: "a".into(),
        });
        log.push(Event::Model {
            step: 1,
            raw: "b".into(),
        });
        let text = log.to_jsonl();
        // Simulate a crash mid-write: drop the last (partial) line.
        let truncated = &text[..text.rfind('\n').unwrap()];
        let back = EventLog::from_jsonl(truncated).unwrap();
        assert_eq!(back.len(), 1);
    }

    #[test]
    fn a_malformed_line_names_its_number_rather_than_being_dropped() {
        let bad = "{\"event\":\"model\",\"step\":0,\"raw\":\"a\"}\nNOT JSON\n";
        let err = EventLog::from_jsonl(bad).unwrap_err();
        assert!(format!("{err:#}").contains("line 2"), "{err:#}");
    }

    #[tokio::test]
    async fn replay_backend_returns_recorded_replies_in_order_then_errors() {
        let mut log = EventLog::new();
        log.push(Event::Model {
            step: 0,
            raw: "first".into(),
        });
        log.push(Event::Model {
            step: 1,
            raw: "second".into(),
        });

        let b = ReplayBackend::from_log(&log);
        assert_eq!(b.remaining(), 2);
        assert_eq!(b.complete(&[]).await.unwrap(), "first");
        assert_eq!(b.complete(&[]).await.unwrap(), "second");
        // Running past the end is an error, never a silent stop.
        let err = b.complete(&[]).await.unwrap_err();
        assert!(format!("{err:#}").contains("exhausted"));
    }

    #[test]
    fn strict_replay_is_green_when_observations_match() {
        let mut log = EventLog::new();
        log.push(Event::Observed {
            step: 0,
            observation: Box::new(obs_rec("https://a.test/", "hello")),
        });
        let mut report = ReplayReport::default();
        check_step(
            &log,
            0,
            &obs_rec("https://a.test/", "hello"),
            ReplayMode::Strict,
            &mut report,
        )
        .unwrap();
        assert!(report.is_identical());
        assert_eq!(report.steps_checked, 1);
    }

    #[test]
    fn strict_replay_aborts_on_the_first_divergence_and_names_it() {
        let mut log = EventLog::new();
        log.push(Event::Observed {
            step: 0,
            observation: Box::new(obs_rec("https://a.test/", "hello")),
        });
        let err = check_step(
            &log,
            0,
            &obs_rec("https://a.test/", "CHANGED"),
            ReplayMode::Strict,
            &mut ReplayReport::default(),
        )
        .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("step 0"), "{msg}");
        assert!(msg.contains("page text digest"), "{msg}");
    }

    #[test]
    fn lenient_replay_collects_divergences_and_keeps_going() {
        let mut log = EventLog::new();
        log.push(Event::Observed {
            step: 0,
            observation: Box::new(obs_rec("https://a.test/", "hello")),
        });
        log.push(Event::Observed {
            step: 1,
            observation: Box::new(obs_rec("https://b.test/", "world")),
        });

        let mut report = ReplayReport::default();
        check_step(
            &log,
            0,
            &obs_rec("https://a.test/", "hello"),
            ReplayMode::Lenient,
            &mut report,
        )
        .unwrap();
        check_step(
            &log,
            1,
            &obs_rec("https://OTHER/", "world"),
            ReplayMode::Lenient,
            &mut report,
        )
        .unwrap();

        assert_eq!(report.steps_checked, 2);
        assert_eq!(report.divergences.len(), 1);
        assert_eq!(report.divergences[0].0, 1);
        assert!(report.divergences[0].1.contains("url"));
    }

    #[test]
    fn a_screenshot_difference_is_a_divergence() {
        let mut a = obs_rec("https://a.test/", "x");
        let b = ObservationRecord {
            screenshot_digest: Some(43),
            ..a.clone()
        };
        assert!(a.diff(&b).unwrap().contains("screenshot"));
        // ...and identical screenshots are not.
        a.screenshot_digest = Some(43);
        assert!(a.diff(&b).is_none());
    }

    #[test]
    fn missing_recorded_step_is_an_error() {
        let log = EventLog::new();
        let err = check_step(
            &log,
            7,
            &obs_rec("https://a.test/", "x"),
            ReplayMode::Strict,
            &mut ReplayReport::default(),
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("step 7"));
    }
}
