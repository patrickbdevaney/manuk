//! manuk-compositor — layer compositing, scroll, damage tracking, hibernation.
//!
//! Two responsibilities from CLAUDE.md live here:
//!
//! 1. **Damage tracking** ([`Damage`]) — the set of dirty regions since the last
//!    frame, so paint/compositing only touches what changed (click-to-navigate and
//!    frame-timing targets).
//! 2. **Per-tab memory tiers** ([`TabManager`]) — the isolate-per-tab memory model:
//!    the focused tab gets the full Vello-GPU + active-JS tier; background tabs drop
//!    to the CPU tier with JS frozen; least-recently-used tabs beyond a budget
//!    hibernate (evicted to disk, no active renderer).
//!
//! This crate is the *policy/state* layer. The actual GPU surface + present lives
//! in the shell (winit/wgpu); the actual raster lives in `manuk-paint`.

use manuk_layout::Rect;

/// Process resident-memory (RSS) probe — the ground-truth counterpart to the per-tab
/// [`TabManager::total_mem`] heap estimate (CLAUDE.md §8 metric #3, per-tab baseline
/// RSS). Because tabs share one process, this is *whole-process* RSS; per-tab
/// attribution is the estimate, this is the reality check on it.
pub mod mem {
    /// Current process resident set size, in bytes, or `None` if unavailable on this
    /// platform.
    ///
    /// **Linux** reads `VmRSS` from `/proc/self/status` (pure `std`, verified here).
    /// **macOS** would use `getrusage(RUSAGE_SELF).ru_maxrss` / `task_info`, **Windows**
    /// `GetProcessMemoryInfo` — both need a platform crate (`libc`/`windows`), so they
    /// are not wired in this Linux environment and return `None` (engineered for
    /// portability, unverified elsewhere — CLAUDE.md platform discipline).
    pub fn process_rss_bytes() -> Option<usize> {
        #[cfg(target_os = "linux")]
        {
            let status = std::fs::read_to_string("/proc/self/status").ok()?;
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("VmRSS:") {
                    // Format: "VmRSS:\t   12345 kB".
                    let kb: usize = rest.split_whitespace().next()?.parse().ok()?;
                    return Some(kb * 1024);
                }
            }
            None
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }
}

/// Rolling frame-time instrument (CLAUDE.md §8 metric #4). Records per-frame
/// durations over a sliding window and reports last / average / p95 / FPS / jank —
/// so the present loop (or the headless raster loop) can surface smoothness.
///
/// The **GPU present** time needs a display (the shell's `gui` feature); this
/// instrument is display-agnostic — the headless `render` path times the CPU raster,
/// the current stand-in for per-frame paint cost.
pub struct FrameTimer {
    samples: std::collections::VecDeque<std::time::Duration>,
    cap: usize,
    frame_start: Option<std::time::Instant>,
}

impl FrameTimer {
    /// A timer keeping the last `window` frames (min 1).
    pub fn new(window: usize) -> Self {
        FrameTimer {
            samples: std::collections::VecDeque::new(),
            cap: window.max(1),
            frame_start: None,
        }
    }

    /// Mark the start of a frame.
    pub fn begin(&mut self) {
        self.frame_start = Some(std::time::Instant::now());
    }

    /// Mark the end of a frame; records elapsed since [`begin`](Self::begin) and
    /// returns it (`None` if `begin` wasn't called).
    pub fn end(&mut self) -> Option<std::time::Duration> {
        let d = self.frame_start.take()?.elapsed();
        self.record(d);
        Some(d)
    }

    /// Record a pre-measured frame duration (evicting the oldest past the window).
    pub fn record(&mut self, dur: std::time::Duration) {
        if self.samples.len() == self.cap {
            self.samples.pop_front();
        }
        self.samples.push_back(dur);
    }

    pub fn last(&self) -> Option<std::time::Duration> {
        self.samples.back().copied()
    }

    /// Mean frame duration over the window.
    pub fn average(&self) -> Option<std::time::Duration> {
        if self.samples.is_empty() {
            return None;
        }
        let sum: std::time::Duration = self.samples.iter().sum();
        Some(sum / self.samples.len() as u32)
    }

    /// 95th-percentile frame duration (the tail that causes visible jank).
    pub fn p95(&self) -> Option<std::time::Duration> {
        if self.samples.is_empty() {
            return None;
        }
        let mut v: Vec<_> = self.samples.iter().copied().collect();
        v.sort_unstable();
        let idx = ((v.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
        Some(v[idx.min(v.len() - 1)])
    }

    /// Frames per second implied by the average frame duration.
    pub fn fps(&self) -> Option<f64> {
        self.average()
            .filter(|d| !d.is_zero())
            .map(|d| 1.0 / d.as_secs_f64())
    }

    /// Number of frames that missed a `budget` (e.g. 16.67 ms for 60 fps) — the jank
    /// count over the window.
    pub fn janky(&self, budget: std::time::Duration) -> usize {
        self.samples.iter().filter(|&&d| d > budget).count()
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// A 60-fps frame budget (~16.67 ms), the usual smoothness target.
pub const FRAME_BUDGET_60FPS: std::time::Duration = std::time::Duration::from_micros(16_667);

/// Render/JS tier of a tab, from heaviest (focused) to lightest (hibernated).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderTier {
    /// Focused tab: full Vello GPU renderer + active JS.
    FocusedGpu,
    /// Visible-but-unfocused / recent tab: CPU raster tier, JS frozen.
    BackgroundCpu,
    /// Evicted to disk: no active renderer, JS frozen, minimal resident memory.
    Hibernated,
}

impl RenderTier {
    pub fn js_frozen(self) -> bool {
        !matches!(self, RenderTier::FocusedGpu)
    }
    pub fn is_evicted(self) -> bool {
        matches!(self, RenderTier::Hibernated)
    }
}

/// Opaque tab identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

#[derive(Clone, Copy, Debug)]
struct TabEntry {
    id: TabId,
    tier: RenderTier,
    /// Monotonic focus recency; higher = more recently focused. Avoids wall-clock.
    recency: u64,
    /// Estimated retained bytes for this tab (fragment tree + tiles + JS), reported by
    /// the tab owner via [`TabManager::set_mem`]. Drops sharply when a tab is
    /// discarded (C1) — the per-tab memory accounting.
    mem_bytes: usize,
}

/// Owns the set of tabs and assigns render tiers per the memory budget.
///
/// Policy: exactly one `FocusedGpu` tab; up to `max_background` most-recently-used
/// others stay `BackgroundCpu`; the rest hibernate.
pub struct TabManager {
    tabs: Vec<TabEntry>,
    max_background: usize,
    seq: u64,
}

impl TabManager {
    /// `max_background` is how many unfocused tabs stay warm (CPU tier) before the
    /// least-recently-used ones hibernate.
    pub fn new(max_background: usize) -> Self {
        TabManager {
            tabs: Vec::new(),
            max_background,
            seq: 0,
        }
    }

    /// Add a tab (initially hibernated until focused/reflowed).
    pub fn add_tab(&mut self, id: TabId) {
        self.tabs.push(TabEntry {
            id,
            tier: RenderTier::Hibernated,
            recency: 0,
            mem_bytes: 0,
        });
        self.retier();
    }

    pub fn remove_tab(&mut self, id: TabId) {
        self.tabs.retain(|t| t.id != id);
        self.retier();
    }

    /// Focus a tab, bumping its recency and recomputing all tiers.
    pub fn focus(&mut self, id: TabId) {
        self.seq += 1;
        let seq = self.seq;
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.recency = seq;
        }
        self.retier();
    }

    pub fn tier(&self, id: TabId) -> Option<RenderTier> {
        self.tabs.iter().find(|t| t.id == id).map(|t| t.tier)
    }

    pub fn count(&self, tier: RenderTier) -> usize {
        self.tabs.iter().filter(|t| t.tier == tier).count()
    }

    /// Record a tab's estimated retained memory (the tab owner recomputes this after
    /// load / discard / wake). Part of the C1 per-tab memory accounting.
    pub fn set_mem(&mut self, id: TabId, bytes: usize) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.mem_bytes = bytes;
        }
    }

    /// A tab's last-reported retained bytes.
    pub fn mem(&self, id: TabId) -> usize {
        self.tabs
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.mem_bytes)
            .unwrap_or(0)
    }

    /// Total retained bytes across all tabs — the process-wide tab-memory figure the
    /// eviction budget is measured against.
    pub fn total_mem(&self) -> usize {
        self.tabs.iter().map(|t| t.mem_bytes).sum()
    }

    pub fn focused(&self) -> Option<TabId> {
        self.tabs
            .iter()
            .find(|t| t.tier == RenderTier::FocusedGpu)
            .map(|t| t.id)
    }

    /// Recompute tiers: most-recent = focused, next `max_background` = CPU, rest
    /// hibernate.
    fn retier(&mut self) {
        // Order by recency descending (most recently focused first). Stable so ties
        // (never-focused tabs at recency 0) keep insertion order.
        let mut order: Vec<usize> = (0..self.tabs.len()).collect();
        order.sort_by(|&a, &b| self.tabs[b].recency.cmp(&self.tabs[a].recency));

        for (rank, &idx) in order.iter().enumerate() {
            self.tabs[idx].tier = if rank == 0 {
                RenderTier::FocusedGpu
            } else if rank <= self.max_background {
                RenderTier::BackgroundCpu
            } else {
                RenderTier::Hibernated
            };
        }
    }
}

/// Accumulated dirty regions since the last composite. A `full` flag short-circuits
/// to "repaint everything" once damage covers the viewport.
#[derive(Clone, Debug, Default)]
pub struct Damage {
    rects: Vec<Rect>,
    full: bool,
}

impl Damage {
    pub fn new() -> Self {
        Damage::default()
    }

    pub fn is_empty(&self) -> bool {
        !self.full && self.rects.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.full
    }

    /// Mark a rectangular region dirty.
    pub fn add(&mut self, rect: Rect) {
        if rect.width > 0.0 && rect.height > 0.0 {
            self.rects.push(rect);
        }
    }

    /// Mark the whole surface dirty (e.g. on scroll or resize).
    pub fn mark_full(&mut self) {
        self.full = true;
        self.rects.clear();
    }

    /// The bounding box of all damage, or `None` if empty. Callers repaint this box.
    pub fn bounding(&self) -> Option<Rect> {
        if self.rects.is_empty() {
            return None;
        }
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for r in &self.rects {
            min_x = min_x.min(r.x);
            min_y = min_y.min(r.y);
            max_x = max_x.max(r.x + r.width);
            max_y = max_y.max(r.y + r.height);
        }
        Some(Rect {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        })
    }

    pub fn clear(&mut self) {
        self.rects.clear();
        self.full = false;
    }
}

/// A scrollable viewport over a page of `content_height` px. Tracks scroll offset
/// and produces damage on scroll (the whole viewport is dirty after a scroll).
#[derive(Clone, Debug)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub content_height: f32,
    pub scroll_y: f32,
}

impl Viewport {
    pub fn new(width: f32, height: f32) -> Self {
        Viewport {
            width,
            height,
            content_height: height,
            scroll_y: 0.0,
        }
    }

    pub fn max_scroll(&self) -> f32 {
        (self.content_height - self.height).max(0.0)
    }

    /// Scroll by `dy` px (clamped), marking `damage` full if the offset changed.
    /// Returns the new scroll offset.
    pub fn scroll_by(&mut self, dy: f32, damage: &mut Damage) -> f32 {
        let old = self.scroll_y;
        self.scroll_y = (self.scroll_y + dy).clamp(0.0, self.max_scroll());
        if self.scroll_y != old {
            damage.mark_full();
        }
        self.scroll_y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_focused_rest_background_then_hibernate() {
        let mut tm = TabManager::new(2); // keep 2 warm background tabs
        let ids: Vec<TabId> = (0..5).map(TabId).collect();
        for id in &ids {
            tm.add_tab(*id);
        }
        // Focus in order 0,1,2,3,4 -> 4 is focused, 3 & 2 background, 1 & 0 hibernate.
        for id in &ids {
            tm.focus(*id);
        }
        assert_eq!(tm.focused(), Some(TabId(4)));
        assert_eq!(tm.count(RenderTier::FocusedGpu), 1);
        assert_eq!(tm.count(RenderTier::BackgroundCpu), 2);
        assert_eq!(tm.count(RenderTier::Hibernated), 2);
        // Focused tab runs JS; hibernated does not.
        assert!(!tm.tier(TabId(4)).unwrap().js_frozen());
        assert!(tm.tier(TabId(0)).unwrap().js_frozen());
        assert!(tm.tier(TabId(0)).unwrap().is_evicted());
    }

    #[test]
    fn frame_timer_stats_and_jank() {
        use std::time::Duration;
        let mut ft = FrameTimer::new(4); // window of 4 frames
                                         // Feed 8ms, 12ms, 40ms (jank), 8ms.
        for ms in [8u64, 12, 40, 8] {
            ft.record(Duration::from_millis(ms));
        }
        assert_eq!(ft.len(), 4);
        assert_eq!(ft.last(), Some(Duration::from_millis(8)));
        // average = (8+12+40+8)/4 = 17ms.
        assert_eq!(ft.average(), Some(Duration::from_millis(17)));
        // p95 over these 4 → the max (40ms) at the tail.
        assert_eq!(ft.p95(), Some(Duration::from_millis(40)));
        // One frame (40ms) missed the 60fps budget.
        assert_eq!(ft.janky(FRAME_BUDGET_60FPS), 1);
        // ~1/0.017s ≈ 58.8 fps.
        assert!(
            (ft.fps().unwrap() - 58.8).abs() < 1.0,
            "fps ~58.8, got {:?}",
            ft.fps()
        );

        // The window evicts oldest: a 5th frame drops the first 8ms sample.
        ft.record(Duration::from_millis(20));
        assert_eq!(ft.len(), 4);

        // begin/end times a real interval (non-zero, small).
        let mut t2 = FrameTimer::new(1);
        t2.begin();
        let d = t2.end().unwrap();
        assert!(d < Duration::from_secs(1));
    }

    #[test]
    fn rss_probe_reads_a_plausible_value() {
        match mem::process_rss_bytes() {
            Some(rss) => {
                // A running test process holds at least ~1 MB and far less than 100 GB.
                assert!(rss > 1 << 20, "RSS implausibly small: {rss}");
                assert!(rss < 100 << 30, "RSS implausibly large: {rss}");
            }
            None => {
                // Only acceptable off Linux (this environment is Linux).
                assert!(!cfg!(target_os = "linux"), "RSS probe should work on Linux");
            }
        }
    }

    #[test]
    fn per_tab_memory_accounting_totals() {
        let mut tm = TabManager::new(2);
        for i in 0..3 {
            tm.add_tab(TabId(i));
        }
        tm.set_mem(TabId(0), 1_000_000); // a live tab
        tm.set_mem(TabId(1), 800_000);
        tm.set_mem(TabId(2), 5_000); // a discarded tab
        assert_eq!(tm.mem(TabId(0)), 1_000_000);
        assert_eq!(tm.total_mem(), 1_805_000);
        // Discarding tab 0 (its owner drops the Page and re-reports) cuts the total.
        tm.set_mem(TabId(0), 5_000);
        assert_eq!(tm.total_mem(), 810_000);
    }

    #[test]
    fn refocusing_revives_a_hibernated_tab() {
        let mut tm = TabManager::new(1);
        for i in 0..4 {
            tm.add_tab(TabId(i));
        }
        for i in 0..4 {
            tm.focus(TabId(i));
        }
        assert!(tm.tier(TabId(0)).unwrap().is_evicted());
        tm.focus(TabId(0));
        assert_eq!(tm.tier(TabId(0)), Some(RenderTier::FocusedGpu));
    }

    #[test]
    fn damage_bounding_unions_rects() {
        let mut d = Damage::new();
        assert!(d.is_empty());
        d.add(Rect {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
        });
        d.add(Rect {
            x: 50.0,
            y: 5.0,
            width: 10.0,
            height: 10.0,
        });
        let b = d.bounding().unwrap();
        assert_eq!(b.x, 10.0);
        assert_eq!(b.y, 5.0);
        assert_eq!(b.x + b.width, 60.0);
        assert_eq!(b.y + b.height, 30.0);
    }

    #[test]
    fn scroll_clamps_and_damages() {
        let mut vp = Viewport::new(800.0, 600.0);
        vp.content_height = 1000.0;
        let mut d = Damage::new();
        assert_eq!(vp.scroll_by(300.0, &mut d), 300.0);
        assert!(d.is_full());
        d.clear();
        // Clamped at max_scroll = 400.
        assert_eq!(vp.scroll_by(999.0, &mut d), 400.0);
        // Already at bottom: no change, no damage.
        d.clear();
        assert_eq!(vp.scroll_by(10.0, &mut d), 400.0);
        assert!(d.is_empty());
    }
}
