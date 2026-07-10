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
