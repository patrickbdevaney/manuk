//! INFERENCE.MD §4 — **the traversal driver that composes the three levers into one working
//! capability.**
//!
//! `cache`, `triage`, and `concurrency` are the pieces; this is where they come together as
//! the "traverse/scrape at scale" path the directive describes. One `visit` of a URL:
//!
//! 1. **Freshness check** ([`cache`](crate::cache)) — if the URL is still fresh under the
//!    caller's policy, return the cached verdict and *fetch nothing*. This is the primary
//!    lever: the cheapest fetch is the one you skip.
//! 2. **Fetch** under a **network** permit ([`concurrency`](crate::concurrency)) — the wide,
//!    I/O-bound tier.
//! 3. **Triage** ([`triage`](crate::triage)) — decide from the server-rendered HTML alone
//!    whether a JS pass would even add content, and extract the visible text in the same
//!    pass.
//! 4. **Record** in the cache, keyed by the *extracted* content (not raw bytes), which
//!    updates the URL's learned volatility.
//! 5. **(reserved) JS pass** under the narrow **JS** permit — only for the minority triage
//!    flags as needing it. This build's page path is JS-less (the realm-per-worker engine
//!    track is separate), so the permit is acquired and the stage is a no-op today; when
//!    per-page JS lands it slots in here with no change to the scheduling shape.
//!
//! [`visit_all`](Traversal::visit_all) runs a batch: the network tier fans out wide while the
//! cache prunes redundant fetches, so a re-traversal of a mostly-unchanged frontier does
//! almost no network work.
//!
//! The `fetch` seam is a trait ([`Fetch`]) so the driver is testable without a network — a
//! test supplies canned pages and asserts the cache actually suppresses the second fetch.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::cache::{ContentCache, FetchOutcome, Freshness};
use crate::concurrency::{ConcurrencyLimits, TraversalPool};
use crate::triage::{self, TriageReport};

/// How a page's bytes are obtained. Abstracted so the traversal driver has no hard network
/// dependency and can be driven by a fixture in tests.
#[async_trait]
pub trait Fetch: Send + Sync {
    /// Fetch `url`, returning its HTML as a string. Errors propagate to the visit result.
    async fn fetch_html(&self, url: &str) -> Result<String>;
}

/// The default [`Fetch`]: the real network via `manuk_net`.
pub struct NetFetch;

#[async_trait]
impl Fetch for NetFetch {
    async fn fetch_html(&self, url: &str) -> Result<String> {
        let resp = manuk_net::fetch(url).await?;
        Ok(resp.decoded_text())
    }
}

/// What one [`visit`](Traversal::visit) did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Visit {
    /// The URL was fresh in the cache; nothing was fetched.
    ServedFromCache,
    /// The URL was fetched. Carries whether the extracted content was new/unchanged/changed
    /// and whether triage says a JS pass is needed.
    Fetched {
        outcome: FetchOutcome,
        needs_js: bool,
        extracted_len: usize,
    },
    /// The fetch failed; the error message is retained (the run continues for other URLs).
    Failed(String),
}

impl Visit {
    pub fn was_fetched(&self) -> bool {
        !matches!(self, Visit::ServedFromCache)
    }
}

/// The traversal driver: owns the freshness cache and the two-tier concurrency pool.
pub struct Traversal {
    cache: Arc<Mutex<ContentCache>>,
    pool: TraversalPool,
}

impl Traversal {
    pub fn new(limits: ConcurrencyLimits) -> Self {
        Traversal {
            cache: Arc::new(Mutex::new(ContentCache::new())),
            pool: TraversalPool::new(limits),
        }
    }

    /// Convenience: limits sized to the machine.
    pub fn for_machine() -> Self {
        Traversal::new(ConcurrencyLimits::for_machine())
    }

    pub fn pool(&self) -> &TraversalPool {
        &self.pool
    }

    /// The learned volatility for a URL, if seen.
    pub async fn volatility(&self, url: &str) -> Option<f32> {
        self.cache.lock().await.volatility(url)
    }

    /// Visit one URL: freshness → fetch → triage → record. `now_ms` is the caller's monotonic
    /// clock (kept explicit for determinism).
    pub async fn visit<F: Fetch + ?Sized>(
        &self,
        fetcher: &F,
        url: &str,
        freshness: Freshness,
        now_ms: u64,
    ) -> Visit {
        // 1. Freshness — skip the fetch entirely if still fresh.
        if self.cache.lock().await.is_fresh(url, now_ms, freshness) {
            return Visit::ServedFromCache;
        }

        // 2. Fetch under a network permit (wide, I/O-bound tier).
        let html = match self.pool.with_net_permit(|| fetcher.fetch_html(url)).await {
            Ok(h) => h,
            Err(e) => return Visit::Failed(format!("{e:#}")),
        };

        // 3. Triage (cheap, extracts visible text in the same pass).
        let report: TriageReport = triage::triage(&html);
        let extracted_len = report.extracted_text.len();

        // 5. (reserved) JS pass under the narrow JS permit — a no-op today (see module docs),
        //    but acquired so the scheduling shape is already correct for when JS lands.
        if report.needs_js {
            self.pool
                .with_js_permit(|| async { /* per-page JS execution slots here */ })
                .await;
        }

        // 4. Record, keyed by extracted content — updates learned volatility.
        let outcome = self
            .cache
            .lock()
            .await
            .record_fetch(url, &report.extracted_text, now_ms);

        Visit::Fetched {
            outcome,
            needs_js: report.needs_js,
            extracted_len,
        }
    }

    /// Visit a batch concurrently. The network tier fans out wide (bounded by the net permit)
    /// while the cache prunes redundant fetches. Returns a per-URL map of outcomes.
    pub async fn visit_all<F: Fetch + ?Sized>(
        &self,
        fetcher: &F,
        urls: &[String],
        freshness: Freshness,
        now_ms: u64,
    ) -> HashMap<String, Visit> {
        let jobs = urls.iter().map(|u| async move {
            let v = self.visit(fetcher, u, freshness, now_ms).await;
            (u.clone(), v)
        });
        futures_util::future::join_all(jobs)
            .await
            .into_iter()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A fixture fetcher that counts how many times each URL was actually fetched — so a test
    /// can prove the cache suppressed a redundant fetch.
    struct FixtureFetch {
        pages: HashMap<String, String>,
        fetches: AtomicUsize,
    }

    impl FixtureFetch {
        fn new(pages: &[(&str, &str)]) -> Self {
            FixtureFetch {
                pages: pages
                    .iter()
                    .map(|(u, h)| (u.to_string(), h.to_string()))
                    .collect(),
                fetches: AtomicUsize::new(0),
            }
        }
        fn fetch_count(&self) -> usize {
            self.fetches.load(Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl Fetch for FixtureFetch {
        async fn fetch_html(&self, url: &str) -> Result<String> {
            self.fetches.fetch_add(1, Ordering::Relaxed);
            self.pages
                .get(url)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("404 {url}"))
        }
    }

    fn ssr_page(body: &str) -> String {
        format!(
            "<html><body><article><p>{}</p></article></html>",
            body.repeat(1)
        )
    }

    /// The headline integration property: a fresh cache entry suppresses the network fetch
    /// entirely.
    #[tokio::test]
    async fn a_fresh_url_is_served_from_cache_without_fetching() {
        let long = "This is a server-rendered article with plenty of real text content to read. "
            .repeat(4);
        let f = FixtureFetch::new(&[("https://a.test/", &ssr_page(&long))]);
        let t = Traversal::new(ConcurrencyLimits { net: 8, js: 2 });

        // First visit fetches.
        let v1 = t
            .visit(&f, "https://a.test/", Freshness::MaxAge(10_000), 0)
            .await;
        assert!(matches!(
            v1,
            Visit::Fetched {
                outcome: FetchOutcome::New,
                needs_js: false,
                ..
            }
        ));
        assert_eq!(f.fetch_count(), 1);

        // Second visit within the TTL is served from cache — no new fetch.
        let v2 = t
            .visit(&f, "https://a.test/", Freshness::MaxAge(10_000), 1_000)
            .await;
        assert_eq!(v2, Visit::ServedFromCache);
        assert_eq!(
            f.fetch_count(),
            1,
            "the cache must suppress the redundant fetch"
        );

        // Past the TTL it fetches again; identical extraction ⇒ Unchanged, volatility decays.
        let v3 = t
            .visit(&f, "https://a.test/", Freshness::MaxAge(10_000), 20_000)
            .await;
        assert!(matches!(
            v3,
            Visit::Fetched {
                outcome: FetchOutcome::Unchanged,
                ..
            }
        ));
        assert_eq!(f.fetch_count(), 2);
    }

    /// Triage rides along: an SSR page reports needs_js=false, an empty SPA shell true.
    #[tokio::test]
    async fn triage_classifies_each_visited_page() {
        let long = "Real server-rendered prose that the traversal can read directly. ".repeat(4);
        let f = FixtureFetch::new(&[
            ("https://ssr.test/", &ssr_page(&long)),
            (
                "https://spa.test/",
                "<html><body><div id=\"root\"></div><script src=\"/b.js\"></script></body></html>",
            ),
        ]);
        let t = Traversal::new(ConcurrencyLimits::default());

        let ssr = t.visit(&f, "https://ssr.test/", Freshness::Always, 0).await;
        assert!(
            matches!(
                ssr,
                Visit::Fetched {
                    needs_js: false,
                    ..
                }
            ),
            "{ssr:?}"
        );

        let spa = t.visit(&f, "https://spa.test/", Freshness::Always, 0).await;
        assert!(
            matches!(spa, Visit::Fetched { needs_js: true, .. }),
            "{spa:?}"
        );
    }

    /// A batch re-traversal of a mostly-unchanged frontier does almost no network work: the
    /// second pass serves every fresh URL from cache.
    #[tokio::test]
    async fn a_second_batch_pass_is_mostly_cache_hits() {
        let long =
            "Frontier page body text that is long enough to be meaningful content here. ".repeat(3);
        let pages: Vec<(String, String)> = (0..10)
            .map(|i| (format!("https://n{i}.test/"), ssr_page(&long)))
            .collect();
        let refs: Vec<(&str, &str)> = pages
            .iter()
            .map(|(u, h)| (u.as_str(), h.as_str()))
            .collect();
        let f = FixtureFetch::new(&refs);
        let urls: Vec<String> = pages.iter().map(|(u, _)| u.clone()).collect();

        let t = Traversal::new(ConcurrencyLimits { net: 16, js: 2 });

        let first = t.visit_all(&f, &urls, Freshness::MaxAge(10_000), 0).await;
        assert!(first.values().all(|v| v.was_fetched()));
        assert_eq!(f.fetch_count(), 10);

        let second = t
            .visit_all(&f, &urls, Freshness::MaxAge(10_000), 1_000)
            .await;
        assert!(
            second.values().all(|v| *v == Visit::ServedFromCache),
            "every fresh URL should be a cache hit on the second pass"
        );
        assert_eq!(
            f.fetch_count(),
            10,
            "no extra network work on the second pass"
        );
    }

    /// A failed fetch is isolated: the run continues and the URL reports Failed.
    #[tokio::test]
    async fn a_failed_fetch_does_not_abort_the_batch() {
        let f = FixtureFetch::new(&[("https://ok.test/", &ssr_page(&"good content ".repeat(30)))]);
        let t = Traversal::new(ConcurrencyLimits::default());
        let urls = vec![
            "https://ok.test/".to_string(),
            "https://missing.test/".to_string(),
        ];

        let out = t.visit_all(&f, &urls, Freshness::Always, 0).await;
        assert!(out["https://ok.test/"].was_fetched());
        assert!(matches!(out["https://missing.test/"], Visit::Failed(_)));
    }
}
