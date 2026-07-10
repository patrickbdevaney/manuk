//! INFERENCE.MD §4 — **content-addressed cache with freshness scoring**.
//!
//! The lever for traversal at scale is *not* fetching a page faster; it is *not fetching it
//! at all* when nothing that matters has changed. Two design choices make that work:
//!
//! **Key by extracted content, not raw bytes.** Raw HTML churns on every request — ad
//! tokens, CSRF nonces, `<!-- generated 12:04:33 -->` timestamps — even when the article
//! text is identical. Hashing the *extracted* content (the visible text / a11y rendering
//! the agent actually consumes) means "unchanged" tracks the thing we care about, so the
//! volatility signal isn't drowned in noise. The digest is [`crate::replay::digest`]
//! (FNV-1a) — a non-cryptographic checksum used only to compare two extractions, never for
//! security.
//!
//! **Learn a per-URL volatility.** Each URL carries a volatility in `[0,1]`: an unchanged
//! re-fetch **decays** it toward 0 (check less often), a changed one **raises** it toward 1
//! (check more often). The caller sets a per-request [`Freshness`] policy; the adaptive mode
//! scales that baseline by the learned stability, within fixed bounds, so a stable reference
//! page earns a long effective TTL while a live feed keeps a short one.
//!
//! Time is passed in as explicit `now_ms` (monotonic milliseconds) rather than read from a
//! clock, so the whole cache is deterministic and unit-testable — and it never depends on
//! `Date::now()`, which this project treats as non-reproducible.
//!
//! **Documented gap (not faked):** this is an in-process, in-memory cache — it does not
//! persist across runs and stores only digests + metadata, not response bodies. It answers
//! "should I re-fetch this URL?", not "give me the bytes I saw last time". A persistent
//! body store (keyed by the same content digest) is the tracked follow-up.

use std::collections::HashMap;

/// A per-request freshness policy — how stale a cached observation may be before the
/// caller wants a re-fetch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Freshness {
    /// Never serve from cache; always re-fetch. (For a "force refresh".)
    Always,
    /// Serve from cache while the entry is younger than this many milliseconds — a **hard**
    /// ceiling the learned volatility does not override.
    MaxAge(u64),
    /// Serve from cache while younger than `base_ms` **scaled by learned stability**: a
    /// stable URL gets up to [`MAX_STABILITY_FACTOR`]× `base_ms`, a volatile one as little
    /// as [`MIN_VOLATILITY_FACTOR`]× `base_ms`. This is the adaptive lever.
    Adaptive { base_ms: u64 },
}

/// A fully stable URL may be trusted this many multiples of the requested base age.
pub const MAX_STABILITY_FACTOR: f32 = 4.0;
/// A fully volatile URL is trusted only this fraction of the requested base age.
pub const MIN_VOLATILITY_FACTOR: f32 = 0.25;

/// The volatility a never-before-seen URL starts at: agnostic, halfway.
const INITIAL_VOLATILITY: f32 = 0.5;
/// On an unchanged re-fetch, volatility is multiplied by this (decays toward 0).
const DECAY: f32 = 0.5;
/// On a changed re-fetch, volatility moves this fraction of the way toward 1.
const RISE: f32 = 0.5;

/// What a `record_fetch` did to the cache — the fact the caller (and the volatility model)
/// acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FetchOutcome {
    /// First time this URL was recorded.
    New,
    /// Re-fetched; the extracted content matched the last recording.
    Unchanged,
    /// Re-fetched; the extracted content differed.
    Changed,
}

#[derive(Clone, Debug)]
struct Entry {
    /// FNV-1a of the *extracted* content (not raw HTML).
    content_digest: u64,
    /// When this entry was last recorded, in caller-supplied monotonic ms.
    fetched_at_ms: u64,
    /// Learned volatility in `[0,1]`; higher ⇒ re-check more often.
    volatility: f32,
    /// How many times this URL has been recorded (for diagnostics).
    fetches: u32,
}

/// An in-memory, content-addressed freshness cache. Not a body store — see the module docs.
#[derive(Default)]
pub struct ContentCache {
    entries: HashMap<String, Entry>,
}

impl ContentCache {
    pub fn new() -> Self {
        ContentCache {
            entries: HashMap::new(),
        }
    }

    /// Whether a re-fetch of `url` should be skipped in favor of the cached observation,
    /// given `now_ms` and the caller's `freshness` policy. `false` for an unknown URL (there
    /// is nothing to serve) and always `false` under [`Freshness::Always`].
    pub fn is_fresh(&self, url: &str, now_ms: u64, freshness: Freshness) -> bool {
        let Some(e) = self.entries.get(url) else {
            return false;
        };
        let age = now_ms.saturating_sub(e.fetched_at_ms);
        match freshness {
            Freshness::Always => false,
            Freshness::MaxAge(ttl) => age < ttl,
            Freshness::Adaptive { base_ms } => age < e.effective_ttl(base_ms),
        }
    }

    /// The inverse of [`is_fresh`](Self::is_fresh): whether the caller should actually fetch.
    pub fn should_fetch(&self, url: &str, now_ms: u64, freshness: Freshness) -> bool {
        !self.is_fresh(url, now_ms, freshness)
    }

    /// Record the result of fetching `url` whose *extracted* content is `extracted`. Updates
    /// the digest, timestamp, and learned volatility, and reports whether the content was
    /// new/unchanged/changed relative to the last recording.
    ///
    /// Pass the extracted content — visible text or the a11y rendering — **not** raw HTML;
    /// see [`digest_extracted`] and the module docs for why.
    pub fn record_fetch(&mut self, url: &str, extracted: &str, now_ms: u64) -> FetchOutcome {
        let digest = digest_extracted(extracted);
        match self.entries.get_mut(url) {
            None => {
                self.entries.insert(
                    url.to_string(),
                    Entry {
                        content_digest: digest,
                        fetched_at_ms: now_ms,
                        volatility: INITIAL_VOLATILITY,
                        fetches: 1,
                    },
                );
                FetchOutcome::New
            }
            Some(e) => {
                let changed = e.content_digest != digest;
                if changed {
                    // Move toward 1: check this URL more often.
                    e.volatility += (1.0 - e.volatility) * RISE;
                } else {
                    // Decay toward 0: it has been stable, check it less often.
                    e.volatility *= DECAY;
                }
                e.content_digest = digest;
                e.fetched_at_ms = now_ms;
                e.fetches = e.fetches.saturating_add(1);
                if changed {
                    FetchOutcome::Changed
                } else {
                    FetchOutcome::Unchanged
                }
            }
        }
    }

    /// The learned volatility for `url` in `[0,1]`, if seen. Exposed for diagnostics and the
    /// traversal scheduler (a volatile URL is worth revisiting sooner).
    pub fn volatility(&self, url: &str) -> Option<f32> {
        self.entries.get(url).map(|e| e.volatility)
    }

    /// The effective adaptive TTL for `url` at `base_ms`, if seen — what [`is_fresh`] uses
    /// under [`Freshness::Adaptive`]. Exposed so a caller can explain a decision.
    ///
    /// [`is_fresh`]: Self::is_fresh
    pub fn effective_ttl(&self, url: &str, base_ms: u64) -> Option<u64> {
        self.entries.get(url).map(|e| e.effective_ttl(base_ms))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Entry {
    /// Scale `base_ms` by stability (`1 - volatility`) between the fixed bounds. A stable
    /// URL (volatility→0) approaches `MAX_STABILITY_FACTOR × base`, a volatile one
    /// (volatility→1) approaches `MIN_VOLATILITY_FACTOR × base`.
    fn effective_ttl(&self, base_ms: u64) -> u64 {
        let stability = 1.0 - self.volatility.clamp(0.0, 1.0);
        let factor =
            MIN_VOLATILITY_FACTOR + stability * (MAX_STABILITY_FACTOR - MIN_VOLATILITY_FACTOR);
        (base_ms as f32 * factor) as u64
    }
}

/// Digest the *extracted* content of a page for cache-keying. A thin, intention-revealing
/// wrapper over [`crate::replay::digest`]: callers should pass extracted text, and naming it
/// here documents that raw HTML is the wrong input.
pub fn digest_extracted(extracted: &str) -> u64 {
    crate::replay::digest(extracted.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_url_is_never_fresh_and_should_be_fetched() {
        let c = ContentCache::new();
        assert!(!c.is_fresh("https://x.test/", 1000, Freshness::MaxAge(10_000)));
        assert!(c.should_fetch("https://x.test/", 1000, Freshness::MaxAge(10_000)));
    }

    /// The headline property: two fetches whose *raw HTML* differs but whose *extracted
    /// content* is identical count as Unchanged — the whole reason to key on extraction.
    #[test]
    fn identical_extraction_under_churning_html_is_unchanged() {
        let mut c = ContentCache::new();
        // Simulate extraction: the caller passes the extracted text, so two pages whose
        // ads/nonces differ but whose article text matches produce the same input here.
        let extracted = "Breaking: the article body that actually matters.";
        assert_eq!(c.record_fetch("https://news.test/a", extracted, 0), FetchOutcome::New);
        assert_eq!(
            c.record_fetch("https://news.test/a", extracted, 1_000),
            FetchOutcome::Unchanged
        );
    }

    #[test]
    fn changed_extraction_is_detected() {
        let mut c = ContentCache::new();
        c.record_fetch("https://news.test/a", "version one", 0);
        assert_eq!(
            c.record_fetch("https://news.test/a", "version two", 1_000),
            FetchOutcome::Changed
        );
    }

    /// Volatility decays on unchanged re-fetches and rises on changed ones — the adaptive
    /// signal the scheduler and the Adaptive freshness policy read.
    #[test]
    fn volatility_decays_when_stable_and_rises_when_changing() {
        let mut c = ContentCache::new();
        let url = "https://x.test/";
        c.record_fetch(url, "same", 0);
        let start = c.volatility(url).unwrap();

        c.record_fetch(url, "same", 1);
        c.record_fetch(url, "same", 2);
        let after_stable = c.volatility(url).unwrap();
        assert!(after_stable < start, "stable re-fetches decay volatility: {after_stable} !< {start}");

        c.record_fetch(url, "different now", 3);
        let after_change = c.volatility(url).unwrap();
        assert!(after_change > after_stable, "a change raises volatility");
    }

    /// Adaptive freshness gives a stable URL a longer effective TTL than a volatile one, for
    /// the same requested base age — and MaxAge stays a hard ceiling regardless.
    #[test]
    fn adaptive_ttl_rewards_stability_and_maxage_is_hard() {
        let mut c = ContentCache::new();
        let stable = "https://stable.test/";
        let volatile = "https://volatile.test/";

        // Drive `stable` to low volatility and `volatile` to high volatility.
        c.record_fetch(stable, "x", 0);
        for t in 1..6 {
            c.record_fetch(stable, "x", t);
        }
        c.record_fetch(volatile, "a", 0);
        for t in 1..6 {
            c.record_fetch(volatile, &format!("v{t}"), t);
        }

        let base = 10_000u64;
        let ttl_stable = c.effective_ttl(stable, base).unwrap();
        let ttl_volatile = c.effective_ttl(volatile, base).unwrap();
        assert!(
            ttl_stable > base && ttl_volatile < base,
            "stable earns >base ({ttl_stable}), volatile earns <base ({ttl_volatile})"
        );
        assert!(ttl_stable > ttl_volatile);

        // At an age between the two effective TTLs, the stable URL is still fresh and the
        // volatile one is not.
        let now = base + 1; // just past the base age
        assert!(c.is_fresh(stable, now, Freshness::Adaptive { base_ms: base }));
        assert!(!c.is_fresh(volatile, now, Freshness::Adaptive { base_ms: base }));

        // MaxAge ignores volatility entirely — a hard ceiling measured from the last fetch
        // (both URLs were last recorded at t=5 in the loops above).
        let last = 5u64;
        assert!(c.is_fresh(stable, last + base - 1, Freshness::MaxAge(base)));
        assert!(!c.is_fresh(stable, last + base + 1, Freshness::MaxAge(base)));
        // Always never serves from cache.
        assert!(!c.is_fresh(stable, 0, Freshness::Always));
    }
}
