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
    host_is_tracker(url)
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
}
