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
    /// §5 — user-pinned. Persisted across a session save/restore; UI-only otherwise.
    pinned: bool,
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

    /// Mutable access to a live (non-discarded) tab's page — e.g. to deliver a cross-window
    /// `postMessage` to a background tab that is the message's target.
    pub fn page_mut(&mut self) -> Option<&mut Page> {
        match &mut self.retained {
            Retained::Live { page, .. } => Some(page.as_mut()),
            Retained::Discarded => None,
        }
    }

    /// §5 — whether the user pinned this tab.
    pub fn is_pinned(&self) -> bool {
        self.pinned
    }
}

/// The set of open tabs plus tier management + C1 hibernation.
pub struct Browser {
    tabs: Vec<Tab>,
    manager: TabManager,
    next_id: u64,
    active: Option<TabId>,
    /// ⚠⚠⚠ **PAGES THAT ARE LOGICALLY GONE AND HAVE NOT BEEN FREED YET — AND THE REASON THIS
    /// QUEUE EXISTS IS THAT FREEING ONE COSTS A FRAME.**
    ///
    /// A `Page` is a DOM arena, a `StyleMap`, a fragment tree and (with scripts) a JS context.
    /// Dropping one is thousands of individual `free()` calls, and it was measured at
    /// **2.8–6.3 ms** for a 300-item article — three to six times a 60 fps frame's whole budget.
    /// Every tier decision that hibernates a tab used to pay that inline, on the UI thread, inside
    /// the very operation the user had just performed: `G_INTERACT`'s 30-tab session measured
    /// `open` at a **median of 4.7 ms** because opening the thirty-first tab evicts the seventh,
    /// and evicting it freed a whole page before `open` returned. Under load the same operation
    /// reached 22 ms and dropped the frame outright.
    ///
    /// So the eviction is split from the free. The tab's `Retained` flips to `Discarded`
    /// immediately — it *is* hibernated the instant the policy says so, and every query reads that
    /// — while the page itself moves here and is dropped by [`Browser::reap_pending`] after the
    /// frame is on screen. **Nothing is skipped**: the same bytes are freed and the same
    /// `malloc_trim` still runs, one frame later, off the path the user is waiting on.
    reap: Vec<Box<Page>>,
    /// A page has been freed since the last trim, so the allocator still owes the kernel its
    /// pages. Kept separate from `reap` so a burst of evictions pays **one** trim rather than one
    /// per tab (`malloc_trim` walks the free lists — see [`Browser::reap_pending`]).
    trim_owed: bool,
}

impl Browser {
    pub fn new(max_background: usize) -> Self {
        Browser {
            tabs: Vec::new(),
            manager: TabManager::new(max_background),
            next_id: 0,
            active: None,
            reap: Vec::new(),
            trim_owed: false,
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
            pinned: false,
            source: String::new(),
            retained: Retained::Discarded,
        });
        self.manager.add_tab(id);
        self.focus(id);
        id
    }

    /// §5 — **restore a tab hibernated**, from a persisted session or collection. The tab
    /// is created `Discarded` (no `Page`, no fetch) with its saved title/pinned metadata,
    /// and is **not** focused — the caller focuses exactly the one tab that should load
    /// eagerly. Reopening a 40-tab session this way costs 40 URLs' worth of metadata, not 40
    /// page loads, which is the whole point of hibernation-by-default.
    pub fn open_restored(
        &mut self,
        url: impl Into<String>,
        title: impl Into<String>,
        pinned: bool,
    ) -> TabId {
        let id = TabId(self.next_id);
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            url: url.into(),
            title: title.into(),
            content_height: 0.0,
            pinned,
            source: String::new(),
            retained: Retained::Discarded,
        });
        self.manager.add_tab(id);
        // Deliberately no focus() here: the caller chooses the single eager tab.
        self.apply_tiers();
        id
    }

    /// §5 — set a tab's pinned flag.
    pub fn set_pinned(&mut self, id: TabId, pinned: bool) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.pinned = pinned;
        }
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
    ///
    /// A tab that already held a page is **navigating**, and the outgoing page is queued for
    /// [`reap_pending`](Self::reap_pending) rather than freed here: the assignment below would
    /// otherwise drop it inline, spending a frame's budget on the old document at the exact moment
    /// the new one wants to paint.
    pub fn load(&mut self, id: TabId, page: Page, source: String) {
        let outgoing = self.take_page(id);
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
        self.queue_reap(outgoing);
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
        // Take the page out BEFORE the tab leaves the vec: `retain` would drop the `Tab`, and with
        // it a live `Page`, inside the click that closed it. Same cost, same fix as `discard`.
        let doomed = self.take_page(id);
        self.queue_reap(doomed);
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
    ///
    /// **The drop is only half of it, and for ~350 ticks this method did only that half.**
    /// The 100-tab benchmark (`manuk-wpt memtabs`) measured it: dropping every `Page` after
    /// loading wix.com returned **0%** of the 1.3 GB the load had cost, because `free()`
    /// hands memory to glibc's arenas, not to the kernel. RSS is what the OOM killer and
    /// the user's task manager read, so a tab that is "hibernated" but whose memory the
    /// process still holds is hibernated in name only. Trimming after the drop returned
    /// **92%** of that same 1.3 GB. See [`manuk_compositor::mem::release_free_memory_to_os`].
    ///
    /// ⚠ **AND THAT COMMENT NAMED THE WRONG HALF AS THE EXPENSIVE ONE.** It read: *"the trim is
    /// deliberately here — on discard, the rare event — and not on every frame: it walks the
    /// allocator's free lists, so paying it per frame would trade the memory win for jank."* The
    /// jank was real and the attribution was not. Measured on the `G_INTERACT` document — 300
    /// flex rows, ~1,800 nodes:
    ///
    /// ```text
    ///     drop(Page)   2.827  3.654  6.255  4.053  ms
    ///     malloc_trim  0.952  0.778  1.332  0.810  ms
    /// ```
    ///
    /// The trim is the **cheap** half; the drop it was guarding is three to six times larger. A
    /// comment that blames the smaller cost buys a guard against the wrong thing — and here it
    /// bought the belief that discard was affordable inline, which is why `open` cost a median of
    /// 4.7 ms in a 30-tab session and why one contended `focus` reached 22 ms.
    ///
    /// **Discard is now an unlink, not a free**, and *both* halves are deferred together to
    /// [`reap_pending`](Self::reap_pending) — the trim still follows the drop, and still only
    /// on discard, but neither lands on the operation the user is waiting for. It is *also*
    /// still not on freeze: a frozen tab keeps its `Page`, and handing back pages it is about to
    /// fault straight in was correct then and is correct now.
    fn discard(&mut self, id: TabId) {
        let doomed = self.take_page(id);
        self.queue_reap(doomed);
        self.refresh_mem(id);
    }

    /// Take a tab's `Page` out, leaving the tab `Discarded`. The caller owns the box — and
    /// therefore owns *when* it is freed, which is the whole point.
    fn take_page(&mut self, id: TabId) -> Option<Box<Page>> {
        let t = self.tabs.iter_mut().find(|t| t.id == id)?;
        match std::mem::replace(&mut t.retained, Retained::Discarded) {
            Retained::Live { page, .. } => Some(page),
            Retained::Discarded => None,
        }
    }

    /// Queue a page for the off-frame free. A `None` (the tab held no page) queues nothing and,
    /// crucially, owes no trim — an eviction that freed nothing must not make the next frame walk
    /// the allocator's free lists for it.
    fn queue_reap(&mut self, page: Option<Box<Page>>) {
        if let Some(p) = page {
            self.reap.push(p);
        }
    }

    /// **Do ONE unit of the deferred free, and say whether more is owed.**
    ///
    /// Called by the host after a frame is on screen (`RedrawRequested`, past the present). One
    /// unit is one `Page` dropped, or — once the queue is empty — the single
    /// [`release_free_memory_to_os`](manuk_compositor::mem::release_free_memory_to_os) that hands
    /// the freed pages back to the kernel. Bounded on purpose: draining twenty evicted tabs in one
    /// callback would simply move the stall rather than remove it, so each frame pays for one and
    /// asks for another.
    ///
    /// Returns `true` while work remains, so the host can request the next redraw. A host that
    /// never calls this does not leak — `Browser` owns the queue and drops it — but it does hold
    /// evicted pages resident, which is the thing hibernation exists to prevent, so
    /// **`G_TAB_REAP` gates that the drain runs.**
    pub fn reap_pending(&mut self) -> bool {
        if let Some(page) = self.reap.pop() {
            drop(page);
            self.trim_owed = true;
            return true;
        }
        if self.trim_owed {
            self.trim_owed = false;
            manuk_compositor::mem::release_free_memory_to_os();
        }
        false
    }

    /// Run the deferred free to completion. For hosts with no frame loop (session restore, tests,
    /// headless drivers) — the interactive path uses [`reap_pending`](Self::reap_pending).
    pub fn drain_reaps(&mut self) {
        while self.reap_pending() {}
    }

    /// How many evicted pages are still holding memory. The witness `G_TAB_REAP` reads to prove
    /// the deferral is a deferral and not a leak.
    pub fn pending_reaps(&self) -> usize {
        self.reap.len()
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

    /// Mutable access to a live tab's page by id (for cross-window `postMessage` delivery to a
    /// background tab). `None` if the tab is unknown or discarded.
    pub fn page_mut(&mut self, id: TabId) -> Option<&mut Page> {
        self.tabs
            .iter_mut()
            .find(|t| t.id == id)
            .and_then(|t| t.page_mut())
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

// ---------------------------------------------------------------------------
// G-e — instant per-tab resource honesty
// ---------------------------------------------------------------------------

/// What a tab is currently costing, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct TabResource {
    pub id: TabId,
    pub url: String,
    pub title: String,
    pub tier: RenderTier,
    pub state: TabState,
    /// Retained bytes this tab would return if it were discarded. A **proxy**, not an
    /// RSS reading: it is the `Page`'s estimated heap plus the retained source HTML.
    pub retained_bytes: usize,
    /// Per-tab JS heap. Always `None` today — see the note on [`resource_report`].
    pub js_heap_bytes: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabState {
    /// Focused or otherwise fully live.
    Active,
    /// Live, but its JS timer/task queue is throttled.
    Frozen,
    /// The `Page` was dropped; a wake re-lays-out from the retained source.
    Discarded,
}

impl std::fmt::Display for TabState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TabState::Active => "active",
            TabState::Frozen => "frozen",
            TabState::Discarded => "discarded",
        })
    }
}

/// A whole-process snapshot: every tab, plus the real resident-set size.
#[derive(Clone, Debug, PartialEq)]
pub struct ResourceReport {
    pub tabs: Vec<TabResource>,
    /// Sum of the per-tab proxies. **Not** the process's memory: shared allocations
    /// (fonts, the JS runtime, wgpu) are counted once by the OS and not at all here.
    pub total_retained_bytes: usize,
    /// The process's real RSS, when the OS exposes it (Linux `/proc/self/status`).
    pub process_rss_bytes: Option<usize>,
}

impl Browser {
    /// G-e — an honest, instant accounting of what each tab costs.
    ///
    /// "Honest" is the whole point of the item, so the numbers are labelled for what
    /// they are: `retained_bytes` is a **proxy** (what a discard would reclaim), while
    /// `process_rss_bytes` is the OS's real figure for the whole process. They do not
    /// sum to each other and the report says so rather than implying a false precision.
    ///
    /// **Documented gap (not faked):** `js_heap_bytes` is always `None`. Per-tab JS heap
    /// needs SpiderMonkey's per-compartment memory reporters, which in turn needs one JS
    /// realm per tab (the C1/§7 model). That is engine work, not accounting work, and
    /// reporting a fabricated number here would be exactly the dishonesty this item
    /// exists to avoid.
    pub fn resource_report(&self) -> ResourceReport {
        let tabs: Vec<TabResource> = self
            .tabs()
            .iter()
            .map(|t| TabResource {
                id: t.id,
                url: t.url.clone(),
                title: t.title.clone(),
                tier: self.tier(t.id).unwrap_or(RenderTier::Hibernated),
                state: if t.is_discarded() {
                    TabState::Discarded
                } else if t.is_frozen() {
                    TabState::Frozen
                } else {
                    TabState::Active
                },
                retained_bytes: self.tab_mem(t.id),
                js_heap_bytes: None,
            })
            .collect();

        ResourceReport {
            total_retained_bytes: tabs.iter().map(|t| t.retained_bytes).sum(),
            process_rss_bytes: manuk_compositor::mem::process_rss_bytes(),
            tabs,
        }
    }
}

impl ResourceReport {
    /// A task-manager rendering, one line per tab.
    pub fn to_table(&self) -> String {
        use std::fmt::Write as _;
        let mb = |b: usize| b as f64 / (1024.0 * 1024.0);
        let mut s = String::new();
        let _ = writeln!(
            s,
            "{:<4} {:<10} {:<14} {:>10}  TITLE / URL",
            "TAB", "STATE", "TIER", "RETAINED"
        );
        for t in &self.tabs {
            let _ = writeln!(
                s,
                "{:<4} {:<10} {:<14} {:>9.2}M  {}",
                t.id.0,
                t.state.to_string(),
                format!("{:?}", t.tier),
                mb(t.retained_bytes),
                if t.title.is_empty() { &t.url } else { &t.title }
            );
        }
        let _ = writeln!(
            s,
            "\nretained (proxy, sums the column above): {:.2} MB",
            mb(self.total_retained_bytes)
        );
        match self.process_rss_bytes {
            Some(rss) => {
                let _ = writeln!(
                    s,
                    "process RSS (real, OS-reported):        {:.2} MB",
                    mb(rss)
                );
            }
            None => {
                let _ = writeln!(s, "process RSS: unavailable on this platform");
            }
        }
        let _ = writeln!(
            s,
            "per-tab JS heap: not reported (needs SpiderMonkey per-compartment reporters)"
        );
        s
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;
    use manuk_text::FontContext;

    fn page(html: &str, fonts: &FontContext) -> Page {
        Page::load(html, "https://ex.test/", fonts, 800.0)
    }

    /// G-e acceptance: every tab is accounted for, states are reported truthfully, and
    /// discarding a tab is visible as reclaimed retained bytes.
    #[test]
    fn the_report_tells_the_truth_about_each_tab() {
        let fonts = FontContext::new();
        let mut b = Browser::new(8);

        let a = b.open("https://a.test/");
        b.load(
            a,
            page("<title>A</title><body><p>aaa</p></body>", &fonts),
            "<p>aaa</p>".into(),
        );
        let c = b.open("https://c.test/");
        b.load(
            c,
            page("<title>C</title><body><p>ccc</p></body>", &fonts),
            "<p>ccc</p>".into(),
        );
        b.focus(a);

        let r = b.resource_report();
        assert_eq!(r.tabs.len(), 2);
        assert!(
            r.tabs.iter().all(|t| t.retained_bytes > 0),
            "live tabs cost something"
        );
        assert_eq!(
            r.total_retained_bytes,
            r.tabs.iter().map(|t| t.retained_bytes).sum::<usize>()
        );

        // The focused tab is active; the JS heap is honestly absent.
        let ta = r.tabs.iter().find(|t| t.id == a).unwrap();
        assert_eq!(ta.state, TabState::Active);
        assert_eq!(ta.tier, RenderTier::FocusedGpu);
        assert_eq!(ta.js_heap_bytes, None, "we do not invent a JS heap figure");

        // Discarding a tab must show up as reclaimed memory, not as a silent no-op.
        let before = b.resource_report();
        let c_before = before
            .tabs
            .iter()
            .find(|t| t.id == c)
            .unwrap()
            .retained_bytes;
        b.discard(c);
        let after = b.resource_report();
        let tc = after.tabs.iter().find(|t| t.id == c).unwrap();
        assert_eq!(tc.state, TabState::Discarded);
        assert!(
            tc.retained_bytes < c_before,
            "a discard must reclaim: {c_before} -> {}",
            tc.retained_bytes
        );
        assert!(after.total_retained_bytes < before.total_retained_bytes);
    }

    #[test]
    fn the_table_labels_the_proxy_and_the_real_rss_separately() {
        let fonts = FontContext::new();
        let mut b = Browser::new(8);
        let a = b.open("https://a.test/");
        b.load(
            a,
            page("<title>A</title><body>x</body>", &fonts),
            "x".into(),
        );

        let table = b.resource_report().to_table();
        assert!(table.contains("RETAINED"));
        assert!(
            table.contains("proxy"),
            "the proxy must be labelled as such"
        );
        assert!(
            table.contains("process RSS"),
            "the real OS figure must be reported separately"
        );
        assert!(
            table.contains("per-tab JS heap: not reported"),
            "the missing JS heap must be stated, not hidden"
        );
    }
}

/// **G_INTERACT — the UI thread must never stall on a tab operation.**
///
/// Every gate this project has added came after a user felt something a green gate could not see.
/// The scroll freeze was invisible to G1/G2/G3 because none of them measured per-event cost. The
/// frozen tab was invisible because none of them measured a page with a dead subresource. The
/// pattern never changes: *a gate that does not measure what the user feels reports green while the
/// user suffers.*
///
/// So this measures what a person actually does to a browser dozens of times an hour — open a tab,
/// switch tabs, close a tab — and asserts that none of it lands on the UI thread as a stall.
#[cfg(test)]
mod g_interact {
    use super::*;
    use std::time::{Duration, Instant};

    /// Not two tabs. The tab bar is where people accumulate.
    const TABS: usize = 30;

    fn median(mut v: Vec<Duration>) -> Duration {
        v.sort();
        v[v.len() / 2]
    }
    fn worst(v: &[Duration]) -> Duration {
        *v.iter().max().unwrap()
    }

    #[test]
    fn tab_operations_stay_far_under_one_frame() {
        // **The floor is one frame, and it is deliberately generous.** These operations touch a Vec
        // of tabs and a tier policy; they have no business being anywhere near 16ms. The floor is
        // not a target, it is a tripwire: crossing it means something has quietly started doing real
        // work — cloning a `Page`, re-laying-out, walking the DOM — inside an operation whose cost
        // the user believes is zero.
        let frame = Duration::from_millis(16);

        // **With real pages in them.** An empty `Browser` measures a `Vec<TabId>` and proves
        // nothing: the cost that bites is `apply_tiers` walking every tab and freezing/discarding
        // its `Page`, and a tab with no page has no page to walk. So each tab gets a document of
        // roughly the size of a real article.
        let fonts = manuk_text::FontContext::new();
        let html = {
            let mut h = String::from("<style>.c{display:flex}.i{flex:1;padding:8px}</style><body>");
            for i in 0..300 {
                h.push_str(&format!(
                    "<div class=c><div class=i><h3>Item {i}</h3><p>Some body text for item {i} \
                     that wraps across a couple of lines like real prose does.</p></div></div>"
                ));
            }
            h.push_str("</body>");
            h
        };

        let mut b = Browser::new(6);
        let mut opens = Vec::new();
        let mut ids = Vec::new();
        for i in 0..TABS {
            let t = Instant::now();
            let id = b.open(format!("https://example.com/{i}"));
            opens.push(t.elapsed());
            let page = Page::load(&html, &format!("https://example.com/{i}"), &fonts, 1200.0);
            b.load(id, page, html.clone());
            ids.push(id);
        }

        // Focusing re-runs the tier policy across every tab — the operation most likely to quietly
        // acquire O(tabs × page) cost as the browser grows features.
        let mut switches = Vec::new();
        for &id in &ids {
            let t = Instant::now();
            b.focus(id);
            switches.push(t.elapsed());
        }

        let mut closes = Vec::new();
        for &id in &ids {
            let t = Instant::now();
            b.close(id);
            closes.push(t.elapsed());
        }

        for (name, v) in [("open", &opens), ("switch", &switches), ("close", &closes)] {
            println!(
                "  {name:<7} median {:>7.3}ms   worst {:>7.3}ms   (one frame = 16ms)",
                median(v.to_vec()).as_secs_f64() * 1000.0,
                worst(v).as_secs_f64() * 1000.0
            );
        }

        for (name, v) in [("open", &opens), ("switch", &switches), ("close", &closes)] {
            let w = worst(v);
            assert!(
                w < frame,
                "the WORST {name} took {:.1}ms — over a frame. A tab operation stalling the UI \
                 thread is exactly the 'the browser feels laggy' report that no rendering gate can \
                 see.",
                w.as_secs_f64() * 1000.0
            );
        }

        // Scaling, not just the absolute number: closing the thirtieth tab must not cost more than
        // closing the first. A per-operation cost that GROWS with the tab count is the shape of the
        // bug this gate exists for, and a fixed ceiling would not notice it until the user had 200
        // tabs open and the browser was already unusable.
        let first: Duration = closes[..5].iter().sum();
        let last: Duration = closes[closes.len() - 5..].iter().sum();
        assert!(
            last <= first * 4 + Duration::from_micros(300),
            "closing the LAST tabs ({last:?}) costs far more than closing the FIRST ({first:?}) — \
             the per-tab cost is growing with the number of tabs open"
        );
    }

    /// **G_TAB_REAP — a tab operation must not FREE a page, and the page must still get freed.**
    ///
    /// The sibling gate above sets its tripwire at one frame, and for ~1,300 ticks the browser
    /// passed it while doing the very thing it was written to forbid. The numbers say why: with 30
    /// tabs and a 6-tab budget, `open` ran at a **median of 4.7 ms** and a contended `focus` at
    /// **22 ms** — because every eviction freed a whole `Page` (DOM arena + `StyleMap` + fragment
    /// tree, thousands of `free()` calls, measured at 2.8–6.3 ms) inside the operation. A 16 ms bar
    /// cannot see a 4.7 ms defect; it can only be tripped by it once the machine is busy, which is
    /// how this arrived as an undiagnosable intermittent RED rather than as a bug report.
    ///
    /// So the bar here is **2 ms**, and it is not a performance target — it is a *structural*
    /// assertion. Nothing a tab operation legitimately does (push a `Tab`, walk 30 tier entries)
    /// is within two orders of magnitude of it. Only freeing a page can cross it, so crossing it
    /// means the deferral has been undone.
    ///
    /// ⚠ **AND A GATE THAT ONLY MEASURED THE CLOCK WOULD REWARD THE LEAK.** "Don't free the page"
    /// is trivially satisfiable by never freeing it, which is the same memory bug hibernation
    /// exists to fix, hidden behind a faster number. So the second half asserts the pages are
    /// really queued (`pending_reaps() > 0` — deferred, not skipped) and that the drain really
    /// empties (`drain_reaps()` → 0). Both halves must hold, or this is a stopwatch, not a gate.
    #[test]
    fn tab_operations_defer_the_page_free_and_the_drain_actually_frees_it() {
        // Two orders of magnitude below a frame: only a page free can cross it.
        let bar = Duration::from_millis(2);

        let fonts = manuk_text::FontContext::new();
        let mut html = String::from("<style>.c{display:flex}.i{flex:1;padding:8px}</style><body>");
        for i in 0..300 {
            html.push_str(&format!(
                "<div class=c><div class=i><h3>Item {i}</h3><p>Some body text for item {i} \
                 that wraps across a couple of lines like real prose does.</p></div></div>"
            ));
        }
        html.push_str("</body>");

        let mut b = Browser::new(6);
        let mut ops: Vec<(&str, Duration)> = Vec::new();
        let mut ids = Vec::new();
        for i in 0..TABS {
            let t = Instant::now();
            let id = b.open(format!("https://example.com/{i}"));
            ops.push(("open", t.elapsed()));
            let page = Page::load(&html, &format!("https://example.com/{i}"), &fonts, 1200.0);
            b.load(id, page, html.clone());
            ids.push(id);
        }
        for &id in &ids {
            let t = Instant::now();
            b.focus(id);
            ops.push(("focus", t.elapsed()));
        }

        // The queue must be non-empty BEFORE the closes: 30 tabs against a 6-tab budget means the
        // policy has hibernated most of them, and every one of those pages is owed a free.
        let queued = b.pending_reaps();
        assert!(
            queued > 0,
            "30 tabs on a 6-tab budget evicted nothing — either the tier policy stopped hibernating \
             or `discard` went back to freeing inline. Both make the timings below meaningless."
        );

        for &id in &ids {
            let t = Instant::now();
            b.close(id);
            ops.push(("close", t.elapsed()));
        }

        let (name, w) = ops
            .iter()
            .max_by_key(|(_, d)| *d)
            .map(|(n, d)| (*n, *d))
            .unwrap();
        println!(
            "  reap: worst {name} {:>7.3}ms (bar 2ms) · {queued} pages queued before close, {} after",
            w.as_secs_f64() * 1000.0,
            b.pending_reaps()
        );
        assert!(
            w < bar,
            "the WORST {name} took {:.1}ms against a 2ms bar. A tab operation has started FREEING a \
             page on the UI thread again: that is 3-6ms of `free()` storm inside the click, and it \
             is invisible to the one-frame gate until the machine is busy.",
            w.as_secs_f64() * 1000.0
        );

        // The other half: deferred, not skipped.
        assert!(
            b.pending_reaps() > 0,
            "every page was freed somewhere else after all — the deferral is not being exercised"
        );
        b.drain_reaps();
        assert_eq!(
            b.pending_reaps(),
            0,
            "the drain did not empty the queue — an evicted tab whose page the process still holds \
             is hibernated in name only, which is the exact defect `discard` exists to fix"
        );
    }
}

/// **G_RUNTIME_COUNT — one runtime, one pool, for the life of the process (METHODOLOGY Part 25.2).**
///
/// G_SPAWN governs how many *tasks* a click creates. This governs something categorically worse: how
/// many long-lived *runtimes* exist at all. A task on a shared runtime is fine; a new runtime per
/// action is not.
///
/// The canonical failure — a Tokio runtime built per navigation or per search — is invisible at idle,
/// invisible in a profile of any single action, and lethal after an hour of browsing. That is exactly
/// the shape of the wheel-event clone regression that this project already learned once, one layer
/// further down the stack. So the gate does not check "is there one runtime"; it checks that the
/// count stays FLAT while a scripted session does the things a person does.
#[cfg(test)]
mod g_runtime_count {
    use super::*;

    #[test]
    fn runtime_instantiations_stay_flat_across_a_whole_session() {
        use std::sync::atomic::Ordering;

        // Touch it once so the singleton exists, then take the baseline.
        let _ = manuk_net::runtime();
        let base = manuk_net::RUNTIME_INSTANTIATIONS.load(Ordering::Relaxed);
        assert_eq!(
            base, 1,
            "there must be exactly ONE async runtime for the process, got {base}"
        );

        // A session: navigations, searches, tab opens, tab closes. Repeatedly.
        let mut b = Browser::new(6);
        for round in 0..25 {
            let id = b.open(format!("https://example.com/{round}"));
            let _ = manuk_net::runtime(); // what a navigation/search does
            b.focus(id);
            let _ = manuk_net::runtime();
            if round % 3 == 0 {
                b.close(id);
            }
        }

        let after = manuk_net::RUNTIME_INSTANTIATIONS.load(Ordering::Relaxed);
        assert_eq!(
            after, base,
            "the runtime count ROSE from {base} to {after} across a scripted session — something is \
             building a runtime per user action. That is invisible at idle and lethal after an hour \
             of browsing, and it is the exact shape of the wheel-event clone regression one layer up."
        );
    }

    /// **G_TAB_DISCARD_RELEASES_TO_OS** — evicting a tab must return its memory to the
    /// *kernel*, not merely to the allocator.
    ///
    /// This gate exists because the 100-tab RSS benchmark (`manuk-wpt memtabs`, tick 571)
    /// measured the gap for the first time and it was total: after loading wix.com — 3 MB of
    /// HTML — the process held **1.31 GB**, and dropping the `Page` returned **0%** of it.
    /// The retained heap was only ~11 MB; the rest was the transient spike of parse+cascade,
    /// freed to glibc and then kept in its arenas forever. A single `malloc_trim` returned
    /// **92%**. RSS is the number the OOM killer and the user's task manager read, so
    /// without the trim a "hibernated" tab is hibernated in name only.
    ///
    /// It lives in this crate rather than in `manuk-compositor` because [`Browser::discard`]
    /// is the caller the fix protects, and because this crate's test binary is one the wall
    /// actually runs — a gate the wall does not execute is documentation.
    #[test]
    fn discarding_a_tab_returns_memory_to_the_os_not_just_the_allocator() {
        let Some(before) = manuk_compositor::mem::process_footprint() else {
            return; // Non-Linux: honestly unmeasured, see release_free_memory_to_os.
        };

        // ~256 MB in 64 KiB chunks. Two details are load-bearing, and the first draft of
        // this gate got the second one wrong and passed under its own RED patch.
        //
        // 1. **Chunk size.** glibc services anything over M_MMAP_THRESHOLD (128 KiB by
        //    default) with its own `mmap` and gives THAT straight back on free, so a single
        //    big block would prove nothing. 64 KiB comes off the heap.
        //
        // 2. **Fragmentation.** glibc *does* already shrink the top of the heap when the
        //    free run there passes M_TRIM_THRESHOLD. So freeing one contiguous slab is
        //    returned automatically and needs no trim — which is why a test that allocates
        //    and frees everything is green even with the trim stubbed out. The real browser
        //    case is not that shape: the load spike is freed *around* the data the page
        //    keeps, leaving free pages stranded in the middle of the heap where the
        //    top-of-heap shrink can never reach them. `PINNED` reproduces exactly that by
        //    keeping every 64th chunk alive.
        const CHUNK: usize = 64 * 1024;
        const CHUNKS: usize = 4096;
        const PIN_EVERY: usize = 64;
        let mut hog: Vec<Vec<u8>> = (0..CHUNKS).map(|_| vec![1u8; CHUNK]).collect();
        // Touch every chunk so the pages are genuinely resident, not just reserved.
        for c in hog.iter_mut() {
            c[0] = 2;
            c[CHUNK - 1] = 2;
        }
        let peak = manuk_compositor::mem::process_footprint().unwrap();
        let grew = peak.rss_bytes as i64 - before.rss_bytes as i64;
        // If the allocation did not move RSS at all, this machine is not measuring what we
        // think it is; skip rather than assert a conclusion we did not earn.
        if grew < (128 << 20) {
            return;
        }

        // Free everything except the pins — the stranded-free-pages shape.
        let mut pinned: Vec<Vec<u8>> = Vec::new();
        for (i, c) in hog.drain(..).enumerate() {
            if i % PIN_EVERY == 0 {
                pinned.push(c);
            }
        }
        drop(hog);
        let freed = manuk_compositor::mem::process_footprint().unwrap();
        let by_drop = peak.rss_bytes as i64 - freed.rss_bytes as i64;

        manuk_compositor::mem::release_free_memory_to_os();
        let trimmed = manuk_compositor::mem::process_footprint().unwrap();
        let by_trim = freed.rss_bytes as i64 - trimmed.rss_bytes as i64;
        let returned = by_drop + by_trim;

        assert!(
            returned * 2 >= grew,
            "G_TAB_DISCARD_RELEASES_TO_OS: {} MB became resident, drop() returned {} MB and the \
             trim returned {} MB — {} MB of {} MB never went back to the kernel.\n\n  \
             This is the defect the 100-tab benchmark found: an evicted tab whose memory the \
             process still holds is not evicted, it is relabelled. At 100 tabs that is the \
             difference between a session that fits in RAM and one the OOM killer ends.",
            grew >> 20,
            by_drop >> 20,
            by_trim >> 20,
            (grew - returned) >> 20,
            grew >> 20,
        );
        // Keep the pins alive until after the measurement — dropping them earlier would
        // un-fragment the heap and hand the trim a win it did not have to earn.
        drop(pinned);
    }
}
