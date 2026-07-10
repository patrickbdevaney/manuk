//! The browser's tab model: a set of tabs over the compositor's [`TabManager`],
//! which assigns each tab a render/memory tier (focused GPU vs background CPU vs
//! hibernated) per CLAUDE.md's per-tab memory targets.
//!
//! **C1 hibernation actions** are realized here (where the heavy `Page` lives):
//! - **freeze** (background-CPU tier): keep the DOM + layout resident but mark the tab
//!   frozen so its JS timer/task queue is throttled (≤1/min);
//! - **discard** (hibernated tier): drop the `Page` (fragment tree + computed styles +
//!   parsed DOM) — the real RAM reclaim — retaining only the URL + source HTML so a
//!   **wake** re-lays-out (or, deeper, re-fetches) on demand.
//!
//! Per-tab retained memory is estimated ([`manuk_page::Page::estimated_bytes`]) and
//! reported to the compositor's [`TabManager`], which sums it for the eviction budget.
//!
//! Some accessors here are the tab-management API for the multi-tab UI (which the
//! single-window `browse` command does not exercise yet) and are covered by the unit
//! tests below, so dead-code is permitted at the module level.
#![allow(dead_code)]

use manuk_compositor::{RenderTier, TabId, TabManager};
use manuk_page::Page;
use manuk_text::FontContext;

/// A tab's heavy retained render state.
enum Retained {
    /// Full state resident (focused, or a frozen background tab). `frozen` throttles
    /// the JS timer/task queue. The `Page` is boxed so the `Discarded` variant stays
    /// small (an evicted tab holds no fragment tree).
    Live { page: Box<Page>, frozen: bool },
    /// Discarded to reclaim memory — the `Page` was dropped; only the source HTML
    /// (retained on the [`Tab`]) is kept for re-layout on wake.
    Discarded,
}

/// A single browser tab.
pub struct Tab {
    pub id: TabId,
    pub url: String,
    pub title: String,
    pub content_height: f32,
    /// Source HTML retained across a discard, so a wake can re-lay-out without a
    /// re-fetch. (A deeper reclaim would drop this too and re-fetch from `url`.)
    source: String,
    retained: Retained,
}

impl Tab {
    /// Estimated retained bytes: the `Page`'s heap (when live) plus the source HTML.
    fn retained_bytes(&self) -> usize {
        let page = match &self.retained {
            Retained::Live { page, .. } => page.estimated_bytes(),
            Retained::Discarded => 0,
        };
        page + self.source.len()
    }

    pub fn is_frozen(&self) -> bool {
        matches!(self.retained, Retained::Live { frozen: true, .. })
    }

    pub fn is_discarded(&self) -> bool {
        matches!(self.retained, Retained::Discarded)
    }

    pub fn page(&self) -> Option<&Page> {
        match &self.retained {
            Retained::Live { page, .. } => Some(page.as_ref()),
            Retained::Discarded => None,
        }
    }
}

/// The set of open tabs plus tier management + C1 hibernation.
pub struct Browser {
    tabs: Vec<Tab>,
    manager: TabManager,
    next_id: u64,
    active: Option<TabId>,
}

impl Browser {
    pub fn new(max_background: usize) -> Self {
        Browser {
            tabs: Vec::new(),
            manager: TabManager::new(max_background),
            next_id: 0,
            active: None,
        }
    }

    /// Open a new (not-yet-loaded, discarded) tab at `url`, focus it, return its id.
    pub fn open(&mut self, url: impl Into<String>) -> TabId {
        let id = TabId(self.next_id);
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            url: url.into(),
            title: "…".to_string(),
            content_height: 0.0,
            source: String::new(),
            retained: Retained::Discarded,
        });
        self.manager.add_tab(id);
        self.focus(id);
        id
    }

    /// Record navigation metadata only (the single-window `browse` UI owns its `Page`
    /// separately and does not yet exercise the multi-tab C1 path). For hibernation
    /// accounting use [`load`](Self::load), which takes ownership of the `Page`.
    pub fn set_loaded(&mut self, id: TabId, url: String, title: String, content_height: f32) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.url = url;
            t.title = title;
            t.content_height = content_height;
        }
    }

    /// Record a completed load: store the `Page` + its source and refresh accounting.
    pub fn load(&mut self, id: TabId, page: Page, source: String) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.url = page.final_url.clone();
            t.title = page.title.clone();
            t.content_height = page.content_height;
            t.source = source;
            t.retained = Retained::Live {
                page: Box::new(page),
                frozen: false,
            };
        }
        self.refresh_mem(id);
        self.apply_tiers();
    }

    pub fn focus(&mut self, id: TabId) {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active = Some(id);
            self.manager.focus(id);
            self.apply_tiers();
        }
    }

    pub fn close(&mut self, id: TabId) {
        self.tabs.retain(|t| t.id != id);
        self.manager.remove_tab(id);
        if self.active == Some(id) {
            self.active = self.tabs.first().map(|t| t.id);
            if let Some(a) = self.active {
                self.manager.focus(a);
            }
        }
        self.apply_tiers();
    }

    /// Enforce the C1 actions to match each tab's compositor tier: focused → live +
    /// unfrozen; background-CPU → frozen (keep the `Page`); hibernated → discarded
    /// (drop the `Page`).
    pub fn apply_tiers(&mut self) {
        let ids: Vec<TabId> = self.tabs.iter().map(|t| t.id).collect();
        for id in ids {
            match self.manager.tier(id) {
                Some(RenderTier::FocusedGpu) => self.unfreeze(id),
                Some(RenderTier::BackgroundCpu) => self.freeze(id),
                Some(RenderTier::Hibernated) => self.discard(id),
                None => {}
            }
        }
    }

    /// Freeze a tab: keep the `Page` resident but throttle its JS (≤1/min). The
    /// event-loop driver consults [`Tab::is_frozen`] to slow that tab's `setTimeout`
    /// queue.
    fn freeze(&mut self, id: TabId) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            if let Retained::Live { frozen, .. } = &mut t.retained {
                *frozen = true;
            }
        }
    }

    fn unfreeze(&mut self, id: TabId) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            if let Retained::Live { frozen, .. } = &mut t.retained {
                *frozen = false;
            }
        }
    }

    /// Discard a tab: drop its `Page` (fragment tree + styles + DOM) to reclaim RAM,
    /// keeping the URL + source for a later wake. A no-op if already discarded.
    fn discard(&mut self, id: TabId) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            if matches!(t.retained, Retained::Live { .. }) {
                t.retained = Retained::Discarded;
            }
        }
        self.refresh_mem(id);
    }

    /// Wake a discarded tab by re-laying-out its retained source HTML. Returns whether
    /// a wake occurred (false if not discarded or no source). (A deeper wake would
    /// re-fetch from `url` when the source was also dropped.)
    pub fn wake(&mut self, id: TabId, fonts: &FontContext, width: f32) -> bool {
        let woke = if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            if matches!(t.retained, Retained::Discarded) && !t.source.is_empty() {
                let page = Page::load(&t.source, &t.url, fonts, width);
                t.content_height = page.content_height;
                t.retained = Retained::Live {
                    page: Box::new(page),
                    frozen: false,
                };
                true
            } else {
                false
            }
        } else {
            false
        };
        if woke {
            self.refresh_mem(id);
        }
        woke
    }

    fn refresh_mem(&mut self, id: TabId) {
        if let Some(t) = self.tabs.iter().find(|t| t.id == id) {
            let bytes = t.retained_bytes();
            self.manager.set_mem(id, bytes);
        }
    }

    // -- queries ------------------------------------------------------------

    pub fn active(&self) -> Option<TabId> {
        self.active
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn tier(&self, id: TabId) -> Option<RenderTier> {
        self.manager.tier(id)
    }

    /// Retained bytes for a tab (C1 accounting).
    pub fn tab_mem(&self, id: TabId) -> usize {
        self.manager.mem(id)
    }

    /// Total retained bytes across all tabs.
    pub fn total_mem(&self) -> usize {
        self.manager.total_mem()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_tab(b: &mut Browser, id: TabId, fonts: &FontContext, html: &str) {
        let page = Page::load(html, "http://t.test/", fonts, 800.0);
        b.load(id, page, html.to_string());
    }

    #[test]
    fn open_focus_close_flow() {
        let mut b = Browser::new(2);
        let a = b.open("https://a.test/");
        let c = b.open("https://c.test/");
        assert_eq!(b.active(), Some(c));
        assert_eq!(b.tier(c), Some(RenderTier::FocusedGpu));
        assert_eq!(b.tier(a), Some(RenderTier::BackgroundCpu));

        b.close(c);
        assert_eq!(b.active(), Some(a));
        assert_eq!(b.tier(a), Some(RenderTier::FocusedGpu));
    }

    #[test]
    fn discard_reclaims_memory_and_wake_restores() {
        let fonts = FontContext::new();
        // max_background=1: focused + 1 background stay live; the rest hibernate.
        let mut b = Browser::new(1);
        let html = format!("<body>{}</body>", "<p>lots of content here</p>".repeat(80));

        let t0 = b.open("http://t.test/0");
        load_tab(&mut b, t0, &fonts, &html);
        let t1 = b.open("http://t.test/1");
        load_tab(&mut b, t1, &fonts, &html);
        let t2 = b.open("http://t.test/2");
        load_tab(&mut b, t2, &fonts, &html);

        // t2 focused, t1 background (frozen), t0 hibernated (discarded).
        assert_eq!(b.tier(t2), Some(RenderTier::FocusedGpu));
        assert_eq!(b.tier(t1), Some(RenderTier::BackgroundCpu));
        assert_eq!(b.tier(t0), Some(RenderTier::Hibernated));

        assert!(!b.tab(t2).unwrap().is_frozen(), "focused tab runs JS");
        assert!(b.tab(t1).unwrap().is_frozen(), "background tab is frozen");
        assert!(
            b.tab(t0).unwrap().is_discarded(),
            "hibernated tab is discarded"
        );

        // The discarded tab's retained memory dropped to just its source; the live
        // tabs retain their full Page.
        let live = b.tab_mem(t2);
        let discarded = b.tab_mem(t0);
        assert!(
            discarded * 3 < live,
            "discard should sharply cut retained memory: discarded={discarded}, live={live}"
        );

        // Waking t0 re-lays-out from source → memory returns to the live order.
        assert!(b.wake(t0, &fonts, 800.0), "wake a discarded tab");
        assert!(!b.tab(t0).unwrap().is_discarded());
        assert!(
            b.tab_mem(t0) > discarded * 3,
            "woken tab reclaims its Page memory"
        );
    }
}
