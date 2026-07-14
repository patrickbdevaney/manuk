//! INFERENCE.MD §4 — **multi-page concurrency within one agent process**.
//!
//! Traversal at scale has two resources with *different* ceilings, and the whole point of
//! this module is to refuse to conflate them:
//!
//! - **Network fetch is I/O-bound.** Thousands of in-flight requests are fine; the limit is
//!   politeness and sockets, not CPU. So the fetch tier runs wide.
//! - **JS execution is CPU-bound.** Each page that needs scripts occupies a SpiderMonkey
//!   realm running real work on a core. Oversubscribing here just thrashes; the ceiling is
//!   near the core count. So the JS tier runs narrow.
//!
//! A single "max concurrency" knob would either throttle the network to the JS ceiling
//! (wasting the I/O headroom) or flood the CPU to the network ceiling (thrashing). This
//! [`TraversalPool`] holds **two independent semaphores** so each tier runs at its own right
//! width. The [`triage`](crate::triage) fast path is what makes the split pay off: most
//! pages never acquire a JS permit at all.
//!
//! **Documented gap (not faked):** the JS tier's permit models the *concurrency discipline*
//! — a bounded, core-count-sized pool distinct from the fetch pool — but this build's page
//! path is JS-less (SpiderMonkey realm-per-worker execution is the C1/§7 engine track, not
//! wired here). The permit is real and enforced; what runs under it today is the CPU-bound
//! parse/triage stage, which is the correct thing to bound the same way. When per-page JS
//! execution lands, it slots under this exact permit with no change to the scheduling shape.

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::Semaphore;

/// A sensible wide default for the I/O-bound fetch tier.
pub const DEFAULT_NET_CONCURRENCY: usize = 64;

/// Concurrency budget for a traversal. The two limits are deliberately separate.
#[derive(Clone, Copy, Debug)]
pub struct ConcurrencyLimits {
    /// In-flight network fetches. Wide — this tier is I/O-bound.
    pub net: usize,
    /// Concurrent JS-execution (CPU-bound) slots. Narrow — near the core count.
    pub js: usize,
}

impl ConcurrencyLimits {
    /// Derive limits from the machine: JS near the core count, network far wider.
    pub fn for_machine() -> Self {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        ConcurrencyLimits {
            net: (cores * 16).max(DEFAULT_NET_CONCURRENCY),
            js: cores,
        }
    }
}

impl Default for ConcurrencyLimits {
    fn default() -> Self {
        ConcurrencyLimits::for_machine()
    }
}

/// A two-tier work pool: a wide network semaphore and a narrow JS-execution semaphore, plus
/// live counters so a caller can observe that the two tiers really do run at different
/// widths.
#[derive(Clone)]
pub struct TraversalPool {
    net: Arc<Semaphore>,
    js: Arc<Semaphore>,
    limits: ConcurrencyLimits,
    net_peak: Arc<AtomicUsize>,
    js_peak: Arc<AtomicUsize>,
}

impl TraversalPool {
    pub fn new(limits: ConcurrencyLimits) -> Self {
        TraversalPool {
            net: Arc::new(Semaphore::new(limits.net.max(1))),
            js: Arc::new(Semaphore::new(limits.js.max(1))),
            limits,
            net_peak: Arc::new(AtomicUsize::new(0)),
            js_peak: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn limits(&self) -> ConcurrencyLimits {
        self.limits
    }

    /// Run `f` while holding a **network** permit (the wide tier). Awaits a permit if the
    /// fetch tier is saturated.
    pub async fn with_net_permit<F, Fut, T>(&self, f: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let _permit = self.net.acquire().await.expect("net semaphore open");
        let in_flight = self.limits.net - self.net.available_permits();
        bump_peak(&self.net_peak, in_flight);
        f().await
    }

    /// Run `f` while holding a **JS-execution** permit (the narrow, CPU-bound tier). Awaits a
    /// permit if the JS tier is at its ceiling.
    pub async fn with_js_permit<F, Fut, T>(&self, f: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let _permit = self.js.acquire().await.expect("js semaphore open");
        let in_flight = self.limits.js - self.js.available_permits();
        bump_peak(&self.js_peak, in_flight);
        f().await
    }

    /// The high-water mark of concurrent network permits observed. For asserting the fetch
    /// tier actually ran wider than the JS tier.
    pub fn net_peak(&self) -> usize {
        self.net_peak.load(Ordering::Relaxed)
    }

    /// The high-water mark of concurrent JS permits observed — must stay within the JS limit.
    pub fn js_peak(&self) -> usize {
        self.js_peak.load(Ordering::Relaxed)
    }
}

fn bump_peak(peak: &AtomicUsize, value: usize) {
    let mut cur = peak.load(Ordering::Relaxed);
    while value > cur {
        match peak.compare_exchange_weak(cur, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(observed) => cur = observed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn limits_keep_js_narrow_and_network_wide() {
        let l = ConcurrencyLimits::for_machine();
        assert!(
            l.net > l.js,
            "network tier must be wider than JS: net={} js={}",
            l.net,
            l.js
        );
        assert!(l.js >= 1);
    }

    /// The JS tier never exceeds its ceiling even under a flood of work, while the network
    /// tier runs much wider concurrently — proving the two limits are not conflated.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn the_two_tiers_run_at_different_widths() {
        let pool = TraversalPool::new(ConcurrencyLimits { net: 32, js: 2 });

        // 32 network tasks that all park briefly while holding a net permit — they should be
        // able to overlap widely.
        let net_tasks: Vec<_> = (0..32)
            .map(|_| {
                let p = pool.clone();
                tokio::spawn(async move {
                    p.with_net_permit(|| async {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    })
                    .await
                })
            })
            .collect();

        // 16 JS tasks; the pool must never run more than 2 at once.
        let js_tasks: Vec<_> = (0..16)
            .map(|_| {
                let p = pool.clone();
                tokio::spawn(async move {
                    p.with_js_permit(|| async {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    })
                    .await
                })
            })
            .collect();

        for t in net_tasks {
            t.await.unwrap();
        }
        for t in js_tasks {
            t.await.unwrap();
        }

        assert!(
            pool.js_peak() <= 2,
            "JS tier exceeded its ceiling: {}",
            pool.js_peak()
        );
        assert!(
            pool.net_peak() > pool.js_peak(),
            "network tier ({}) should have run wider than the JS tier ({})",
            pool.net_peak(),
            pool.js_peak()
        );
        assert!(
            pool.net_peak() > 4,
            "network tier should have overlapped widely: {}",
            pool.net_peak()
        );
    }

    /// A permit is released when its guarded future completes, so a second wave can proceed.
    #[tokio::test]
    async fn permits_release_after_use() {
        let pool = TraversalPool::new(ConcurrencyLimits { net: 1, js: 1 });
        pool.with_js_permit(|| async {}).await;
        // If the permit had leaked, this second acquire would deadlock the test.
        pool.with_js_permit(|| async {}).await;
    }
}
