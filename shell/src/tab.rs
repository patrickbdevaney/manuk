//! The browser's tab model: a set of tabs over the compositor's [`TabManager`],
//! which assigns each tab a render/memory tier (focused GPU vs background CPU vs
//! hibernated) per CLAUDE.md's per-tab memory targets.
//!
//! Some accessors here are the tab-management API for the multi-tab UI (which the
//! single-window `browse` command does not exercise yet) and are covered by the
//! unit tests below, so dead-code is permitted at the module level.
#![allow(dead_code)]

use manuk_compositor::{RenderTier, TabId, TabManager};

/// A single browser tab. The heavy `Page` (DOM/layout) is owned by the shell and
/// keyed by `id`; this is the lightweight chrome-facing record.
#[derive(Clone, Debug)]
pub struct Tab {
    pub id: TabId,
    pub url: String,
    pub title: String,
    pub content_height: f32,
}

/// The set of open tabs plus tier management.
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

    /// Open a new tab at `url`, focus it, and return its id.
    pub fn open(&mut self, url: impl Into<String>) -> TabId {
        let id = TabId(self.next_id);
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            url: url.into(),
            title: "…".to_string(),
            content_height: 0.0,
        });
        self.manager.add_tab(id);
        self.focus(id);
        id
    }

    /// Record the result of a navigation/load on a tab.
    pub fn set_loaded(&mut self, id: TabId, url: String, title: String, content_height: f32) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.url = url;
            t.title = title;
            t.content_height = content_height;
        }
    }

    pub fn focus(&mut self, id: TabId) {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active = Some(id);
            self.manager.focus(id);
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
    }

    pub fn active(&self) -> Option<TabId> {
        self.active
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn tier(&self, id: TabId) -> Option<RenderTier> {
        self.manager.tier(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_focus_close_flow() {
        let mut b = Browser::new(2);
        let a = b.open("https://a.test/");
        let c = b.open("https://c.test/");
        assert_eq!(b.active(), Some(c));
        assert_eq!(b.tier(c), Some(RenderTier::FocusedGpu));
        assert_eq!(b.tier(a), Some(RenderTier::BackgroundCpu));

        b.set_loaded(c, "https://c.test/x".into(), "C".into(), 1234.0);
        assert_eq!(b.tabs().iter().find(|t| t.id == c).unwrap().title, "C");

        b.close(c);
        assert_eq!(b.active(), Some(a));
        assert_eq!(b.tier(a), Some(RenderTier::FocusedGpu));
    }
}
