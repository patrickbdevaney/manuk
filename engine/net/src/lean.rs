//! W4.E2 — **lean agent profile**: fetch only what the agent's observation needs.
//!
//! An agent reads the accessibility tree and acts; it does not look at pixels unless it asks for a
//! screenshot. So in this profile (a) image and CSS-`url()` bodies are not fetched until a
//! screenshot is requested, and (b) subresource requests to known tracking hosts are never issued.
//! Opt in with `MANUK_AGENT_PROFILE=lean` or [`set`]. Off by default: the human browser and the
//! default agent profile are unchanged.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed};

static LEAN: AtomicBool = AtomicBool::new(false);
static ENV_READ: AtomicBool = AtomicBool::new(false);
/// Set while a screenshot is being prepared: images are wanted even in the lean profile.
static VISUAL: AtomicBool = AtomicBool::new(false);
/// Tracker requests refused since the last [`reset_stats`].
pub static BLOCKED: AtomicUsize = AtomicUsize::new(0);
/// Body bytes that came off the wire (past the HTTP cache) since the last [`reset_stats`].
pub static WIRE_BYTES: AtomicU64 = AtomicU64::new(0);

/// Is the lean profile on? Reads `MANUK_AGENT_PROFILE` once, unless [`set`] was called.
pub fn enabled() -> bool {
    if !ENV_READ.swap(true, Relaxed) {
        if std::env::var("MANUK_AGENT_PROFILE").as_deref() == Ok("lean") {
            LEAN.store(true, Relaxed);
        }
    }
    LEAN.load(Relaxed)
}

pub fn set(on: bool) {
    ENV_READ.store(true, Relaxed);
    LEAN.store(on, Relaxed);
}

/// True when image bodies should NOT be fetched now (lean, and no screenshot asked for).
pub fn images_deferred() -> bool {
    enabled() && !VISUAL.load(Relaxed)
}

/// Hold images "wanted" for the guard's lifetime (a screenshot is being prepared).
pub struct VisualGuard;
impl VisualGuard {
    pub fn new() -> Self {
        VISUAL.store(true, Relaxed);
        VisualGuard
    }
}
impl Default for VisualGuard {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for VisualGuard {
    fn drop(&mut self) {
        VISUAL.store(false, Relaxed);
    }
}

type DimsCell = std::sync::Arc<tokio::sync::OnceCell<Option<(u32, u32)>>>;
static DIMS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, DimsCell>>> =
    std::sync::OnceLock::new();

/// The one-shot cell holding `url`'s header-derived image size. The parse-time scan and the later
/// image pass `get_or_init` the SAME cell, so a size is asked for once however many callers race.
pub fn dims_cell(url: &str) -> DimsCell {
    DIMS.get_or_init(Default::default)
        .lock()
        .unwrap()
        .entry(url.to_string())
        .or_default()
        .clone()
}

pub fn reset_stats() {
    if let Some(m) = DIMS.get() {
        m.lock().unwrap().clear();
    }
    BLOCKED.store(0, Relaxed);
    WIRE_BYTES.store(0, Relaxed);
}

/// Registrable hosts that serve analytics, ads and beacons — nothing the accessibility tree is built
/// from. A deliberately small, hand-audited list (not EasyPrivacy); a request to the host or any
/// subdomain is refused.
const TRACKER_HOSTS: &[&str] = &[
    "google-analytics.com",
    "googletagmanager.com",
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "facebook.net",
    "connect.facebook.com",
    "scorecardresearch.com",
    "quantserve.com",
    "hotjar.com",
    "hotjar.io",
    "segment.io",
    "segment.com",
    "mixpanel.com",
    "amplitude.com",
    "fullstory.com",
    "newrelic.com",
    "nr-data.net",
    "chartbeat.com",
    "chartbeat.net",
    "taboola.com",
    "outbrain.com",
    "adnxs.com",
    "criteo.com",
    "criteo.net",
    "clarity.ms",
    "omtrdc.net",
    "demdex.net",
    "2o7.net",
    "optimizely.com",
    "bat.bing.com",
    "analytics.twitter.com",
    "ads-twitter.com",
    "px.ads.linkedin.com",
    "snap.licdn.com",
    "analytics.tiktok.com",
    "adsrvr.org",
    "pubmatic.com",
    "rubiconproject.com",
    "moatads.com",
    "krxd.net",
    "bluekai.com",
];

/// Is `url` a request to a known tracking host? Always false when the profile is off.
pub fn is_tracker(url: &str) -> bool {
    if !enabled() {
        return false;
    }
    host_is_tracker(url) || easyprivacy_blocks(url)
}

/// EasyPrivacy, pinned: the file under `engine/net/data/` and its SHA-256 (checked by a test).
#[cfg(feature = "adblock")]
pub const EASYPRIVACY: &str = include_str!("../data/easyprivacy.txt");
#[cfg(feature = "adblock")]
pub const EASYPRIVACY_SHA256: &str =
    "53b57dc05ee155a5eda8c0c84e5b3a42258f1201d1b0c7f32bb5f7545c903341";

/// Test seam: when set, replaces the list text (the "empty list" mutant).
#[cfg(feature = "adblock")]
static LIST_OVERRIDE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// A check sent to the blocker thread: (url, request type, reply).
#[cfg(feature = "adblock")]
type Query = (String, &'static str, std::sync::mpsc::Sender<bool>);
/// The `adblock` engine is not `Send`, so one dedicated thread owns it and answers over a channel.
/// The thread is respawned (list rebuilt) after [`set_list`].
#[cfg(feature = "adblock")]
static BLOCKER: std::sync::Mutex<Option<std::sync::mpsc::Sender<Query>>> =
    std::sync::Mutex::new(None);

/// Replace the filter list (None = the pinned EasyPrivacy). Rebuilds the blocker on next use.
#[cfg(feature = "adblock")]
pub fn set_list(text: Option<String>) {
    *LIST_OVERRIDE.lock().unwrap() = text;
    *BLOCKER.lock().unwrap() = None;
}

#[cfg(feature = "adblock")]
fn blocker_tx() -> std::sync::mpsc::Sender<Query> {
    let mut g = BLOCKER.lock().unwrap();
    g.get_or_insert_with(|| {
        let (tx, rx) = std::sync::mpsc::channel::<Query>();
        let text = LIST_OVERRIDE.lock().unwrap().clone();
        std::thread::spawn(move || {
            let lines: Vec<&str> = text.as_deref().unwrap_or(EASYPRIVACY).lines().collect();
            let b = crate::blocker::ContentBlocker::from_rules(&lines);
            for (url, ty, reply) in rx {
                let _ = reply.send(b.should_block(&url, "https://lean-page.invalid/", ty));
            }
        });
        tx
    })
    .clone()
}

/// Third-party tracker check against EasyPrivacy through the `adblock` crate. The request's type is
/// guessed from the path extension (call sites do not know it); the initiator is unknown, so every
/// request is judged as third-party to a neutral page.
#[cfg(feature = "adblock")]
fn easyprivacy_blocks(url: &str) -> bool {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();
    let ty = match path.rsplit('.').next().unwrap_or("") {
        "js" | "mjs" => "script",
        "gif" | "png" | "jpg" | "jpeg" | "webp" | "svg" => "image",
        "css" => "stylesheet",
        _ => "other",
    };
    let (rtx, rrx) = std::sync::mpsc::channel();
    if blocker_tx().send((url.to_string(), ty, rtx)).is_err() {
        return false;
    }
    rrx.recv().unwrap_or(false)
}

#[cfg(not(feature = "adblock"))]
fn easyprivacy_blocks(_url: &str) -> bool {
    false
}

fn host_is_tracker(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url) else {
        return false;
    };
    let Some(host) = u.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    TRACKER_HOSTS
        .iter()
        .any(|t| host == *t || host.ends_with(&format!(".{t}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_host_and_subdomains_only() {
        assert!(host_is_tracker("https://www.google-analytics.com/collect"));
        assert!(host_is_tracker("https://stats.g.doubleclick.net/x.js"));
        assert!(!host_is_tracker("https://example.com/google-analytics.com"));
        assert!(!host_is_tracker("https://notdoubleclick.net/x"));
        assert!(!host_is_tracker("data:text/plain,hi"));
    }

    /// W4.E2.c gate: hosts that are in EasyPrivacy but NOT in the hand list are refused under lean and
    /// fetched (not flagged) under default; an empty list turns it red. One test, because the lean flag
    /// and list are process-global.
    #[cfg(feature = "adblock")]
    #[test]
    fn easyprivacy_only_hosts_refused_under_lean() {
        use sha2::{Digest, Sha256};
        let hex: String = Sha256::digest(EASYPRIVACY.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(hex, EASYPRIVACY_SHA256, "pinned list drifted");
        let urls = [
            "https://cdn.mouseflow.com/projects/x.js",
            "https://script.crazyegg.com/pages/scripts/1.js",
            "https://www.statcounter.com/counter/counter.js",
            "https://cdn.heapanalytics.com/js/heap-1.js",
            "https://cdn.luckyorange.com/w.js",
        ];

        assert!(
            urls.iter().all(|u| !host_is_tracker(u)),
            "fixture hosts must be EasyPrivacy-only"
        );
        set(true);
        set_list(None);
        let refused: Vec<&&str> = urls.iter().filter(|u| is_tracker(u)).collect();
        set(false);
        let default_flagged = urls.iter().filter(|u| is_tracker(u)).count();
        assert_eq!(
            default_flagged, 0,
            "default profile must not refuse anything"
        );
        println!("refused under lean: {refused:?}");
        assert!(
            refused.len() >= 5,
            "EasyPrivacy-only hosts refused: {refused:?}"
        );
        // mutant: empty list
        set(true);
        set_list(Some(String::new()));
        let n = urls.iter().filter(|u| is_tracker(u)).count();
        set(false);
        set_list(None);
        assert_eq!(n, 0, "empty list refuses nothing");
        // first-party page is untouched
        set(true);
        assert!(!is_tracker("https://example.com/app.js"));
        set(false);
    }
}
