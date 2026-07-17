//! The interactive GPU window (feature `gui`): `winit` for the window/event loop,
//! `wgpu` for the GPU surface.
//!
//! CLAUDE.md's paint target is Vello (GPU-compute) on `wgpu`. Vello is alpha, so
//! this window presents the CPU raster tier's canvas as a GPU-sampled fullscreen
//! quad — a real `wgpu` present path into which a `VelloGpuPainter` slots later for
//! the focused tab. Scroll re-rasterizes the visible viewport; resize reflows.

use std::sync::Arc;

use anyhow::{Context, Result};
use manuk_compositor::Viewport;
use manuk_css::Rgba;
use manuk_layout::{BoxContent, TextStyle};
use manuk_paint::{Canvas, CpuPainter};
use manuk_text::{FontContext, FontFamily, FontKey};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::chrome::{self, Bookmarks, History, Settings};
use crate::find::{self, FindSession};
use crate::panel::{self, AgentPanel, HandoffConsent, PanelScope};
use crate::prerender;
use crate::session::{self, SessionStore};
use crate::tab::Browser;
use manuk_agent::Handoff;
use manuk_compositor::TabId;
use manuk_page::Page;

/// A hamburger-menu action. The menu is a fixed list of these; [`App::run_menu_action`] maps
/// each to the same operation its keyboard shortcut performs.
#[derive(Clone, Copy)]
enum MenuAction {
    NewTab,
    DuplicateTab,
    Bookmark,
    History,
    Downloads,
    Find,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

/// The hamburger menu's items, top to bottom.
const MENU: &[(&str, MenuAction)] = &[
    ("New tab", MenuAction::NewTab),
    ("Duplicate tab", MenuAction::DuplicateTab),
    ("Bookmark this page", MenuAction::Bookmark),
    ("History", MenuAction::History),
    ("Downloads", MenuAction::Downloads),
    ("Find in page", MenuAction::Find),
    ("Zoom in", MenuAction::ZoomIn),
    ("Zoom out", MenuAction::ZoomOut),
    ("Reset zoom", MenuAction::ZoomReset),
];
/// The number of hamburger-menu items — bound by the G3 affordance gate (ADR-010) so a new menu
/// item cannot ship without declaring its observable effect.
pub(crate) const MENU_LEN: usize = MENU.len();

const MENU_W: f32 = 210.0;
const MENU_ITEM_H: f32 = 30.0;
const SUGGEST_ITEM_H: f32 = 28.0;
const SCROLLBAR_W: f32 = 12.0;

/// Height of the toolbar band (nav buttons + address field), in physical px.
const CHROME_HEIGHT: f32 = 44.0;
/// Height of the tab strip drawn above the toolbar.
const TAB_STRIP_H: f32 = 32.0;
/// Total top chrome height — where the page content begins (tab strip + toolbar).
const CHROME_TOP: f32 = TAB_STRIP_H + CHROME_HEIGHT;
/// Layout of the tab strip: per-tab width cap, the `+` new-tab button width.
const TAB_MAX_W: f32 = 210.0;
const TAB_MIN_W: f32 = 90.0;
const NEWTAB_W: f32 = 30.0;

const WGSL: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    // Fullscreen triangle.
    var pts = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    let xy = pts[i];
    var out: VsOut;
    out.pos = vec4(xy, 0.0, 1.0);
    out.uv = vec2((xy.x + 1.0) * 0.5, (1.0 - xy.y) * 0.5);
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv);
}
"#;

/// Launch the browser window pointed at `url`, with an initial content width.
/// R1 — a message delivered back to the UI thread when off-thread navigation work finishes.
/// The `gen` tags the navigation it belongs to, so a result from a superseded/cancelled load
/// is ignored (the user navigated again before it returned).
enum NavEvent {
    /// The main document finished fetching off-thread: a rendered document or a download, or an
    /// error string.
    Fetched {
        gen: u64,
        result: std::result::Result<manuk_page::Loaded, String>,
    },
    /// L32 — a speculatively-prewarmed document finished fetching off-thread. `url` is the
    /// *requested* URL (the key a future click will look up). Built into the bfcache, not shown.
    Prewarmed {
        url: String,
        result: std::result::Result<manuk_page::Loaded, String>,
    },
    /// DEBT-1 — a page-issued `fetch()`/XHR completed **off-thread**. Settled on the UI thread.
    /// `gen` guards against a stale response landing in a page the user has since navigated away
    /// from. This used to `block_on` the UI thread: a slow API call froze the whole browser.
    PageFetch {
        gen: u64,
        id: u32,
        status: u16,
        body: String,
    },
    /// An `<iframe>`'s document arrived — AFTER first paint, for the same reason images do. A heavy
    /// third-party embed must not hold the parent's article hostage.
    IframeReady {
        gen: u64,
        tab: manuk_compositor::TabId,
        node: usize,
        html: String,
        url: String,
    },
    /// **Images arrived AFTER first paint.** The document is already on screen; these fill it in.
    ///
    /// The load path used to fetch and decode every image before the shell was handed anything, so the
    /// window stayed blank until the last tracking pixel on a news front page had arrived or timed out.
    /// Measured on nytimes.com: the document was parsed, cascaded and laid out — everything needed to
    /// paint — in **1.7s**, and the user saw it at **14s**. No browser a person would use does that.
    ImagesReady {
        gen: u64,
        tab: manuk_compositor::TabId,
        images: std::collections::HashMap<String, manuk_paint::DecodedImage>,
    },
}

/// How a native `<form>` submission navigates: a **GET** query URL, or an **urlencoded POST** body.
enum FormSubmission {
    /// `method=get` — the fields are in the URL; navigate there.
    Get(String),
    /// `method=post` — the fields are an `application/x-www-form-urlencoded` request body.
    Post(manuk_agent::forms::UrlencodedPost),
}

/// A completed download, shown in the hamburger menu's Downloads section.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct DownloadRecord {
    pub(crate) filename: String,
    pub(crate) path: std::path::PathBuf,
    pub(crate) bytes: usize,
}

pub fn run(url: String, width: u32, measure_frames: Option<usize>) -> Result<()> {
    let event_loop = EventLoop::<NavEvent>::with_user_event()
        .build()
        .context("creating winit event loop")?;
    let proxy = event_loop.create_proxy();
    let mut app = App::new(url, width, measure_frames, proxy);
    event_loop.run_app(&mut app).context("running event loop")?;
    // **Return normally.** This used to `std::process::exit(0)` to skip SpiderMonkey's teardown
    // crash — and in doing so it skipped every destructor and every line of `main`'s ordered
    // shutdown, which is where the cookie jar and `localStorage` are flushed to the profile. The
    // browser was discarding the user's data on every quit and the exit code said 0.
    //
    // The teardown crash is fixed at its cause (`JS_ShutDown` now runs, in order). A workaround
    // that hides a crash is a data-loss bug wearing a disguise, and this one was.
    drop(app);
    Ok(())
}

/// A back-forward-cached page: the fully constructed [`Page`] plus the scroll offset to
/// restore. Held in [`App::bfcache`] so Back/Forward is instant (R2).
struct BfEntry {
    page: Page,
    scroll: f32,
}

/// Max pages kept in the back-forward cache (a small retained tier — bounded memory).
const BFCACHE_CAP: usize = 6;

struct App {
    url: String,
    width: u32,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    fonts: FontContext,
    /// One process-lifetime async runtime, shared by every navigation and agent run.
    /// Rebuilding a runtime per navigation (the old behaviour) dropped the hyper
    /// connection pool, DNS cache, and TLS session cache on every load — so each
    /// navigation paid a cold DNS + TCP + TLS handshake. Keeping one runtime alive lets
    /// the process-global pooled client actually reuse warm connections.
    rt: &'static tokio::runtime::Runtime,
    page: Option<Page>,
    viewport: Viewport,
    scroll_y: f32,
    /// Set when the on-screen content is stale (scroll/edit/relayout). The actual CPU paint
    /// + texture upload is deferred to the next `RedrawRequested`, so a burst of input events
    /// in one frame coalesces into a single paint instead of re-rasterizing per event.
    needs_paint: bool,
    browser: Browser,
    tab_id: TabId,
    /// Rolling GPU-present frame timer (§8 metric #4) — real on-screen frames.
    frame: manuk_compositor::FrameTimer,
    /// If set, render this many frames back-to-back, print GPU stats, then exit.
    measure_frames: Option<usize>,
    frames_done: usize,

    // ---- E1 chrome UI state ----
    modifiers: ModifiersState,
    history: History,
    /// L04 — completed downloads (most recent last), surfaced in the hamburger menu.
    downloads: Vec<DownloadRecord>,
    /// L32 — the URL currently being speculatively prewarmed (at most one in flight), so a hover
    /// doesn't spawn duplicate prerenders.
    prewarming: Option<String>,
    /// L03 — cross-window messaging routing. Each tab has a process-unique JS window id; a
    /// `postMessage` targets a window id, which these maps resolve back to the tab to deliver to.
    /// `tab_opener` records which window opened a tab (its `window.opener`).
    win_to_tab: std::collections::HashMap<u64, TabId>,
    tab_win: std::collections::HashMap<TabId, u64>,
    tab_opener: std::collections::HashMap<TabId, u64>,
    /// R2 back-forward cache: recently-navigated-away pages kept fully constructed (DOM +
    /// layout + scroll) so Back/Forward restores instantly instead of re-running the whole
    /// pipeline. Bounded LRU (most-recent last).
    bfcache: Vec<(String, BfEntry)>,
    /// R1 — proxy to wake the event loop when off-thread navigation work completes.
    proxy: winit::event_loop::EventLoopProxy<NavEvent>,
    /// R1 — the current navigation generation. Incremented per navigation; a `NavEvent` with
    /// a stale `gen` (the user navigated again first) is discarded, giving free cancellation.
    nav_gen: u64,
    /// R1 — a navigation's main-document fetch is in flight off-thread (chrome stays live).
    loading: bool,
    bookmarks: Bookmarks,
    settings: Settings,
    zoom: f32,
    /// Find-in-page. `find_open` drives whether typed characters go to the find bar.
    find_open: bool,
    find_query: String,
    find_session: FindSession,
    /// Keyboard-driven address bar (Ctrl+L). There is no chrome *widget* yet — the
    /// query and its suggestions are surfaced through `tracing` — but the resolution,
    /// suggestion ranking, and navigation are real. The rendered widget is the
    /// tracked follow-up.
    omnibox_open: bool,
    omnibox_input: String,
    /// Hamburger menu open state (the settings/actions dropdown).
    menu_open: bool,
    /// EPOCH-1 (COMPLETENESS): the Downloads panel. The menu item used to only log to stderr —
    /// a user who clicked it saw NOTHING. A dead affordance is a product bug, not backlog.
    downloads_open: bool,
    /// A brief confirmation shown to the user (e.g. "Bookmarked"). An action the user takes must be
    /// *observable* — §1.8.
    toast: Option<(String, std::time::Instant)>,
    /// While dragging the scrollbar thumb: the grab offset (cursor y − thumb top).
    scrollbar_drag: Option<f32>,
    /// Text selection anchor and cursor in **document** coordinates (`None` = no selection).
    sel_start: Option<(f32, f32)>,
    sel_end: Option<(f32, f32)>,
    /// True while a drag-select is in progress (mouse held down over the page).
    selecting: bool,

    // ---- §5 session persistence ----
    /// The on-disk tab store (session + collections), outside the repo. `None` if the state
    /// directory could not be resolved — persistence is then silently disabled.
    store: Option<SessionStore>,

    // ---- §3 in-browser agent panel ----
    /// Ctrl+J opens a task prompt; typed text goes to the agent, not the page.
    agent_open: bool,
    agent_input: String,

    /// Last known cursor position in physical window pixels (for click hit-testing).
    cursor: (f32, f32),
    /// The text `<input>`/`<textarea>` node currently focused for typing, if any.
    focused_input: Option<manuk_dom::NodeId>,
    /// Whether the cursor is currently over a clickable link (drives the hand cursor).
    over_link: bool,
    /// A scroll happened and the page has not been told yet. Coalesced to one notification per
    /// frame — a trackpad delivers dozens of wheel events per frame.
    scroll_dirty: bool,
    /// The previous frame's canvas bytes (size + RGBA), for a row-level damage diff so
    /// `paint_and_upload` uploads only the rows that actually changed (#2). Correctness is
    /// exact — the uploaded rows are precisely those that differ — so it can't corrupt the
    /// texture; a small change (caret, hover, form edit) uploads a small band, an unchanged
    /// frame uploads nothing, and a scroll (whole canvas differs) falls back to a full upload.
    prev_canvas: Option<(u32, u32, Vec<u8>)>,
}

/// What a click on the page should do, decided from an immutable hit-test so the mutable
/// action can follow without a borrow conflict.
enum PageAction {
    /// Follow a link to this absolute URL.
    Link(String),
    /// Focus this text field for typing.
    FocusInput(manuk_dom::NodeId),
    /// Submit the form owning this button/submit node.
    Submit(manuk_dom::NodeId),
    /// Toggle this checkbox/radio.
    Toggle(manuk_dom::NodeId),
    /// Nothing actionable — clear focus.
    Clear,
}

impl App {
    fn new(
        url: String,
        width: u32,
        measure_frames: Option<usize>,
        proxy: winit::event_loop::EventLoopProxy<NavEvent>,
    ) -> Self {
        let mut browser = Browser::new(8);

        // §5 — restore the prior session **hibernated** (no fetches), unless this is a
        // `--frames` benchmark run (which must not inherit or clobber a real session).
        let store = if measure_frames.is_none() {
            SessionStore::open().ok()
        } else {
            None
        };
        if let Some(store) = &store {
            match store.load_session() {
                Ok(Some(prior)) => {
                    let n = prior.tabs.len();
                    session::restore_into(&mut browser, &prior);
                    tracing::info!(
                        tabs = n,
                        "restored prior session (hibernated; only the focused tab loads)"
                    );
                }
                Ok(None) => {}
                Err(e) => tracing::warn!("session restore skipped: {e:#}"),
            }
        }

        // §5 — restore saved bookmarks (they used to evaporate on every quit: the ★ toggle
        // wrote to an in-memory `Bookmarks` that was re-created empty each launch).
        let bookmarks = store
            .as_ref()
            .and_then(|s| match s.load_bookmarks() {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!("bookmark restore skipped: {e:#}");
                    None
                }
            })
            .unwrap_or_else(Bookmarks::new);

        // §5 — restore the completed-downloads list (it used to evaporate on quit: the menu's
        // Downloads section showed only the current session's saves).
        let downloads = store
            .as_ref()
            .and_then(|s| match s.load_downloads() {
                Ok(d) => d,
                Err(e) => {
                    tracing::warn!("download-list restore skipped: {e:#}");
                    None
                }
            })
            .unwrap_or_default();

        // The CLI target is the active, eagerly-loaded tab: reuse a restored tab with the
        // same URL if present, else open a fresh focused one.
        let existing = browser.tabs().iter().find(|t| t.url == url).map(|t| t.id);
        let tab_id = match existing {
            Some(id) => {
                browser.focus(id);
                id
            }
            None => browser.open(url.clone()),
        };
        App {
            url,
            width,
            window: None,
            gpu: None,
            fonts: FontContext::new(),
            // ONE runtime for the process (Part 25.1) — this used to build a second multi-threaded
            // runtime alongside `main`'s, doubling the worker-thread pool for no reason.
            rt: manuk_net::runtime(),
            page: None,
            viewport: Viewport::new(width as f32, 768.0),
            scroll_y: 0.0,
            needs_paint: true,
            scroll_dirty: false,
            browser,
            tab_id,
            frame: manuk_compositor::FrameTimer::new(240),
            measure_frames,
            frames_done: 0,
            modifiers: ModifiersState::empty(),
            history: History::new(),
            downloads,
            prewarming: None,
            win_to_tab: std::collections::HashMap::new(),
            tab_win: std::collections::HashMap::new(),
            tab_opener: std::collections::HashMap::new(),
            bfcache: Vec::new(),
            proxy,
            nav_gen: 0,
            loading: false,
            bookmarks,
            settings: Settings::default(),
            zoom: 1.0,
            find_open: false,
            find_query: String::new(),
            find_session: FindSession::default(),
            omnibox_open: false,
            omnibox_input: String::new(),
            menu_open: false,
            downloads_open: false,
            toast: None,
            scrollbar_drag: None,
            sel_start: None,
            sel_end: None,
            selecting: false,
            store,
            agent_open: false,
            agent_input: String::new(),
            cursor: (0.0, 0.0),
            focused_input: None,
            over_link: false,
            prev_canvas: None,
        }
    }

    /// Handle a left click at the current cursor: chrome (toolbar) if it lands in the band,
    /// else follow a link under it on the page.
    fn handle_click(&mut self) {
        let (cx, cy) = self.cursor;
        let w = self.viewport.width;

        // Open overlays intercept the click first.
        if self.downloads_open {
            self.downloads_open = false;
            self.rerender();
            return;
        }
        if self.menu_open {
            if let Some(action) = self.menu_item_at(cx, cy, w) {
                self.menu_open = false;
                self.run_menu_action(action);
            } else {
                self.menu_open = false;
            }
            self.rerender();
            return;
        }
        if self.omnibox_open {
            if let Some(url) = self.suggestion_at(cx, cy, w) {
                self.omnibox_open = false;
                self.omnibox_input.clear();
                self.goto(&url);
                return;
            }
        }

        // Scrollbar grab (right edge, below the chrome).
        let win_h = self.viewport.height + CHROME_TOP;
        if self.scrollbar_press(cx, cy, w, win_h) {
            return;
        }

        if cy < CHROME_TOP {
            if cy < TAB_STRIP_H {
                self.handle_tab_strip_click(cx);
            } else {
                self.handle_chrome_click(cx);
            }
            return;
        }
        // Page mouse-down: begin a potential drag-selection. The click *action* (link/submit)
        // is deferred to mouse-up, so a drag selects text and a tap follows the link.
        let (doc_x, doc_y) = (cx, cy - CHROME_TOP + self.scroll_y);
        self.menu_open = false;
        self.sel_start = Some((doc_x, doc_y));
        self.sel_end = None;
        self.selecting = true;
        self.rerender();
    }

    /// Mouse-up on the page: if it was a tap (no meaningful drag), clear any selection and
    /// perform the click action; if it was a drag, keep the selection.
    fn finish_page_interaction(&mut self) {
        if !self.selecting {
            return;
        }
        self.selecting = false;
        let dragged = match (self.sel_start, self.sel_end) {
            (Some((sx, sy)), Some((ex, ey))) => (sx - ex).abs() > 3.0 || (sy - ey).abs() > 3.0,
            _ => false,
        };
        if dragged {
            self.rerender(); // finalize the highlighted selection
            return;
        }
        // A tap: no selection, run the click action at the anchor.
        if let Some((doc_x, doc_y)) = self.sel_start.take() {
            self.sel_end = None;
            self.perform_page_click(doc_x, doc_y);
        }
    }

    /// The page-click action: fire JS listeners on the hit node (bubbling handles delegation),
    /// and unless a handler called `preventDefault`, perform the element's default action
    /// (follow a link, focus a field, submit, toggle).
    fn perform_page_click(&mut self, doc_x: f32, doc_y: f32) {
        let hit = self
            .page
            .as_ref()
            .and_then(|p| p.a11y_tree().hit_test(doc_x, doc_y).map(|n| n.node));
        let mut prevented = false;
        if let Some(hit) = hit {
            let width = self.viewport.width;
            let (sy, focus) = (self.scroll_y, self.focused_input);
            if let Some(page) = self.page.as_mut() {
                // The handler is about to read `window.scrollY` and `document.activeElement`; give
                // it the CURRENT ones, not the ones from page load.
                page.publish_view_state(0.0, sy, focus);
                prevented = !page.dispatch_click(hit, &self.fonts, width);
            }
        }
        // A handler may have scrolled the page (`scrollTo`, `scrollIntoView`) or moved focus.
        self.apply_view_requests();
        // A handler may have called window.open — open those as new tabs.
        self.handle_window_opens();
        // A handler may have issued fetch/XHR — perform them and settle the page's Promises.
        self.pump_fetches();
        // A handler may have routed client-side (history.pushState) — reflect it in the chrome.
        self.handle_history_ops();
        // A handler may have posted a cross-window message — route it to the target tab.
        self.pump_messages();
        if prevented {
            tracing::info!("click: default action prevented by page JS");
            self.rerender();
            return;
        }

        match self.classify_page_click(doc_x, doc_y) {
            PageAction::Link(url) => {
                self.focused_input = None;
                tracing::info!(url = %url, "click: follow link");
                self.goto(&url);
            }
            PageAction::FocusInput(node) => {
                self.focused_input = Some(node);
                self.omnibox_open = false;
                tracing::info!("click: focused a text field");
                self.rerender();
            }
            PageAction::Submit(node) => {
                self.focused_input = None;
                self.submit_owning_form(node);
            }
            PageAction::Toggle(node) => {
                self.toggle_checkbox(node);
            }
            PageAction::Clear => {
                self.focused_input = None;
                self.rerender();
            }
        }
    }

    /// Decide what a page click does by hit-testing and walking up to the nearest actionable
    /// element (link / text field / button / checkbox). Immutable so the action can follow.
    fn classify_page_click(&self, doc_x: f32, doc_y: f32) -> PageAction {
        let Some(page) = self.page.as_ref() else {
            return PageAction::Clear;
        };
        let Some(hit) = page.a11y_tree().hit_test(doc_x, doc_y).map(|n| n.node) else {
            return PageAction::Clear;
        };
        let dom = page.dom();
        let mut cur = Some(hit);
        while let Some(n) = cur {
            match dom.tag_name(n) {
                Some("a") => {
                    if let Some(href) = dom.element(n).and_then(|e| e.attr("href")) {
                        if let Some(u) = resolve_href(&page.final_url, href) {
                            return PageAction::Link(u);
                        }
                    }
                }
                Some("input") => {
                    let ty = dom
                        .element(n)
                        .and_then(|e| e.attr("type"))
                        .unwrap_or("text")
                        .to_ascii_lowercase();
                    return match ty.as_str() {
                        "submit" | "button" | "image" => PageAction::Submit(n),
                        "checkbox" | "radio" => PageAction::Toggle(n),
                        "hidden" | "file" | "range" | "color" => PageAction::Clear,
                        _ => PageAction::FocusInput(n),
                    };
                }
                Some("textarea") => return PageAction::FocusInput(n),
                Some("button") => return PageAction::Submit(n),
                _ => {}
            }
            cur = dom.parent(n);
        }
        PageAction::Clear
    }

    /// Toggle a checkbox/radio's `checked` attribute, relayout, repaint. A radio is
    /// exclusive: checking it clears every other radio with the same `name`.
    fn toggle_checkbox(&mut self, node: manuk_dom::NodeId) {
        let width = self.viewport.width;
        if let Some(page) = self.page.as_mut() {
            let dom = page.dom();
            let is_radio = dom
                .element(node)
                .and_then(|e| e.attr("type"))
                .is_some_and(|t| t.eq_ignore_ascii_case("radio"));
            if is_radio {
                let name = dom
                    .element(node)
                    .and_then(|e| e.attr("name"))
                    .map(str::to_string);
                // Clear the whole radio group, then check this one.
                let group: Vec<manuk_dom::NodeId> = dom
                    .descendants(dom.root())
                    .filter(|&n| {
                        dom.tag_name(n) == Some("input")
                            && dom
                                .element(n)
                                .and_then(|e| e.attr("type"))
                                .is_some_and(|t| t.eq_ignore_ascii_case("radio"))
                            && dom
                                .element(n)
                                .and_then(|e| e.attr("name"))
                                .map(str::to_string)
                                == name
                    })
                    .collect();
                for n in group {
                    page.dom_mut().remove_attr(n, "checked");
                }
                page.dom_mut().set_attr(node, "checked", "");
            } else {
                let checked = dom
                    .element(node)
                    .is_some_and(|e| e.attr("checked").is_some());
                if checked {
                    page.dom_mut().remove_attr(node, "checked");
                } else {
                    page.dom_mut().set_attr(node, "checked", "");
                }
            }
            page.relayout_zoomed(&self.fonts, width, self.zoom);
        }
        self.rerender();
    }

    /// Submit the form owning `node` (a button / submit input): build the GET URL from the
    /// form's successful controls and navigate. `method=post` and formless buttons are no-ops
    /// (logged), matching the agent's form model.
    fn submit_owning_form(&mut self, node: manuk_dom::NodeId) {
        let Some(form) = self
            .page
            .as_ref()
            .and_then(|page| manuk_agent::forms::owning_form(page.dom(), node))
        else {
            tracing::info!("submit: the control is in no form (ignored)");
            return;
        };

        // **Fire `submit` FIRST, and honour `preventDefault()`.**
        //
        // This was missing, and its absence broke essentially every modern form on the web. A form on a
        // React/Vue/Svelte page is not submitted by the browser at all: the page listens for `submit`,
        // cancels the default, and does its own `fetch`. With no event, that handler never ran — so we
        // performed the **full GET navigation the author had explicitly cancelled**, throwing away the
        // page and everything the user had typed. The site appeared to reload itself whenever anyone
        // pressed a button.
        let w = self.viewport.width;
        let proceed = match self.page.as_mut() {
            Some(p) => p.dispatch_submit(form, &self.fonts, w),
            None => true,
        };
        // The handler may have re-rendered the page — that is the entire point of intercepting submit.
        self.pump_fetches();
        self.pump_messages();
        self.rerender();

        if !proceed {
            tracing::info!("submit: the page called preventDefault() — no navigation, as intended");
            return;
        }

        match self.form_submission(form) {
            Some(FormSubmission::Get(url)) => {
                tracing::info!(url = %url, "submit: form GET");
                self.goto(&url);
            }
            Some(FormSubmission::Post(post)) => {
                tracing::info!(url = %post.url, "submit: form POST");
                self.post_navigate(post);
            }
            None => {} // no page, or a non-navigable form — already logged by form_submission
        }
    }

    /// Decide how to submit `form`: a **GET** URL to navigate to, or an **urlencoded POST** body
    /// (the classic login/checkout case). `None` when there is no page, or the form can't be
    /// submitted here — a genuine error, or a `multipart/form-data` file upload, which goes through
    /// the OS file-picker path instead and must not be silently urlencoded (that would drop the
    /// files). Every `None` is logged so a form that does nothing never does so silently.
    fn form_submission(&self, form: manuk_dom::NodeId) -> Option<FormSubmission> {
        let page = self.page.as_ref()?;
        let base = &page.final_url;
        match manuk_agent::forms::submission_url(page.dom(), form, base) {
            Ok(url) => Some(FormSubmission::Get(url)),
            Err(manuk_agent::forms::SubmitError::PostUnsupported) => {
                // A POST with a file input is a multipart upload — handled by the file-picker path,
                // not here. Urlencoding it would drop the file, so refuse LOUDLY rather than send a
                // broken body.
                if !manuk_agent::forms::file_inputs(page.dom(), form).is_empty() {
                    tracing::info!(
                        "submit: form has a file input — a multipart upload is the picker's job, not a urlencoded POST"
                    );
                    return None;
                }
                match manuk_agent::forms::urlencoded_submission(page.dom(), form, base) {
                    Ok(p) => Some(FormSubmission::Post(p)),
                    Err(e) => {
                        tracing::warn!("submit: could not build the form POST body: {e}");
                        None
                    }
                }
            }
            Err(e) => {
                tracing::warn!("submit: {e}");
                None
            }
        }
    }

    /// Perform a top-level **POST navigation** (a native `<form method=post>` submission): stash the
    /// current page into the bfcache, point the URL bar at the action, record history, and fetch the
    /// action with the POST body off-thread. The result lands via `NavEvent::Fetched` and swaps in
    /// exactly as a GET navigation's page does.
    fn post_navigate(&mut self, post: manuk_agent::forms::UrlencodedPost) {
        // The submitting page is the SameSite initiator — captured BEFORE the URL bar is repointed,
        // so a cross-site auto-submitted form POST withholds the target's session cookie (CSRF).
        let initiator = self.url.clone();
        self.stash_current();
        self.url = post.url.clone();
        self.history.push(post.url.clone());
        self.start_post_nav(post.content_type, post.body, initiator);
    }

    /// Drain `form.submit()` / `form.requestSubmit()` calls that scripts queued.
    ///
    /// `submit()` does NOT fire the event (a script calling it has already decided); `requestSubmit()`
    /// does. The spec draws that distinction and it is not pedantry — one is "submit this", the other is
    /// "ask the page whether to submit this".
    fn pump_form_submits(&mut self) {
        let (direct, requested) = match self.page.as_ref() {
            Some(p) => p.take_form_submits(),
            None => (Vec::new(), Vec::new()),
        };
        let w = self.viewport.width;
        for form in requested {
            let proceed = match self.page.as_mut() {
                Some(p) => p.dispatch_submit(form, &self.fonts, w),
                None => true,
            };
            if proceed {
                self.navigate_form(form);
            }
        }
        for form in direct {
            self.navigate_form(form);
        }
    }

    /// Perform the navigation for a form, with no event — the caller has already decided.
    fn navigate_form(&mut self, form: manuk_dom::NodeId) {
        match self.form_submission(form) {
            Some(FormSubmission::Get(url)) => self.goto(&url),
            Some(FormSubmission::Post(post)) => self.post_navigate(post),
            None => {} // already logged by form_submission
        }
    }

    /// Edit the focused text field: append `ch` (or handle backspace when `ch` is empty and
    /// `backspace` is set), update the DOM `value`, relayout, repaint.
    fn edit_focused_input(&mut self, ch: &str, backspace: bool) {
        let Some(node) = self.focused_input else {
            return;
        };
        let width = self.viewport.width;
        if let Some(page) = self.page.as_mut() {
            let mut val = page
                .dom()
                .element(node)
                .and_then(|e| e.attr("value"))
                .unwrap_or("")
                .to_string();
            if backspace {
                val.pop();
            } else {
                val.push_str(ch);
            }
            page.dom_mut().set_attr(node, "value", val);
            page.relayout_zoomed(&self.fonts, width, self.zoom);
        }
        self.rerender();
    }

    /// Submit the form owning the currently focused field (Enter in a text input).
    fn submit_focused_form(&mut self) {
        if let Some(node) = self.focused_input {
            self.submit_owning_form(node);
        }
    }

    /// Toolbar geometry, shared by `draw_chrome` and `handle_chrome_click` so the pixels a user
    /// sees and the regions that respond can never drift apart. Right-to-left: hamburger, bookmark
    /// star, zoom [+ / % / −]. Returns `(minus_x, pct_x, plus_x, star_x, hamburger_x, field_w)`.
    fn toolbar_geom(w: f32) -> (f32, f32, f32, f32, f32, f32) {
        let hamburger_x = w - 30.0;
        let star_x = w - 62.0;
        let plus_x = w - 92.0;
        let pct_x = w - 138.0;
        let minus_x = w - 162.0;
        // The address field ends where the right-hand controls begin.
        let field_w = (minus_x - 12.0 - 100.0).max(20.0);
        (minus_x, pct_x, plus_x, star_x, hamburger_x, field_w)
    }

    /// Whether the current page is bookmarked (drives the ★/☆ toggle).
    fn is_bookmarked(&self) -> bool {
        self.bookmarks.contains(&self.url)
    }

    /// Reload the current page (toolbar ○, Ctrl+R, F5).
    fn reload(&mut self) {
        let u = self.url.clone();
        if !u.is_empty() && u != "about:blank" {
            self.goto_no_history(&u);
        }
    }

    /// Toggle the bookmark for the current page **and show it** — the star flips immediately.
    /// (Before EPOCH-1/Tick-18 this only wrote a log line: the user saw nothing, so it read as
    /// broken. §1.8: a log line is not a UI.)
    fn toggle_bookmark(&mut self) {
        let title = self
            .page
            .as_ref()
            .map(|p| p.title.clone())
            .unwrap_or_default();
        let url = self.url.clone();
        if url.is_empty() || url == "about:blank" {
            return;
        }
        let on = self.bookmarks.toggle(&url, &title);
        self.toast = Some((
            if on {
                "Bookmarked".to_string()
            } else {
                "Bookmark removed".to_string()
            },
            std::time::Instant::now(),
        ));
        self.rerender();
        self.persist_bookmarks();
        tracing::info!(bookmarked = on, url = %url, "bookmark toggled");
    }

    /// Write the bookmarks to disk now — called on every toggle so a bookmark survives even a
    /// crash, not just a clean quit. Best-effort: a write failure is logged, never fatal.
    fn persist_bookmarks(&self) {
        if let Some(store) = &self.store {
            if let Err(e) = store.save_bookmarks(&self.bookmarks) {
                tracing::warn!("bookmark save failed: {e:#}");
            }
        }
    }

    /// Write the completed-downloads list to disk now — called after each save so the Downloads
    /// menu survives a crash, not just a clean quit. Best-effort: a write failure is logged.
    fn persist_downloads(&self) {
        if let Some(store) = &self.store {
            if let Err(e) = store.save_downloads(&self.downloads) {
                tracing::warn!("download-list save failed: {e:#}");
            }
        }
    }

    /// A click within the chrome band: back / forward / reload, zoom, bookmark, menu, or the field.
    fn handle_chrome_click(&mut self, x: f32) {
        let w = self.viewport.width;
        let (minus_x, _pct_x, plus_x, star_x, hamburger_x, _fw) = Self::toolbar_geom(w);

        // Right-hand controls first (they sit inside the old "address field" region).
        if x >= hamburger_x - 10.0 {
            self.menu_open = !self.menu_open;
            self.rerender();
            return;
        }
        if x >= star_x - 8.0 && x < star_x + 20.0 {
            self.toggle_bookmark();
            return;
        }
        // Chromium-style zoom: − and + side by side, click either repeatedly.
        if x >= plus_x - 6.0 && x < plus_x + 20.0 {
            self.apply_zoom(chrome::zoom_in(self.zoom));
            self.rerender();
            return;
        }
        if x >= minus_x - 6.0 && x < minus_x + 20.0 {
            self.apply_zoom(chrome::zoom_out(self.zoom));
            self.rerender();
            return;
        }
        if x < 30.0 {
            if let Some(u) = self.history.back().map(str::to_string) {
                // R2: instant restore from bfcache, else a fresh load.
                if !self.restore_from_bfcache(&u) {
                    self.goto_no_history(&u);
                }
            }
        } else if x < 56.0 {
            if let Some(u) = self.history.forward().map(str::to_string) {
                if !self.restore_from_bfcache(&u) {
                    self.goto_no_history(&u);
                }
            }
        } else if x < 92.0 {
            self.reload();
        } else {
            // Focus the address field: open the omnibox pre-filled with the current URL.
            self.omnibox_open = true;
            self.omnibox_input = if self.url == "about:blank" {
                String::new()
            } else {
                self.url.clone()
            };
            self.rerender();
        }
    }

    /// R1 — begin loading `self.url`. The **main-document fetch** (the dominant blocking wait:
    /// DNS + TLS + TTFB + download) runs **off-thread** on the shared runtime; the UI thread
    /// keeps rendering the current page and stays live to chrome input. On completion a
    /// `NavEvent::Fetched` wakes the event loop and [`Self::finish_load`] builds the page. The
    /// blank/home page has no document and is handled inline (instant). Each fetch is tagged
    /// with `nav_gen`, so navigating again cancels the stale one (its result is discarded).
    fn start_fetch(&mut self) {
        let (w, h) = match &self.gpu {
            Some(g) => (g.config.width, g.config.height),
            None => (self.width, 768),
        };
        if self.url.is_empty() || self.url == "about:blank" {
            self.page = None;
            self.loading = false;
            self.viewport = Viewport::new(w as f32, (h as f32 - CHROME_TOP).max(1.0));
            if let Some(win) = &self.window {
                win.set_title("New Tab — manuk");
            }
            self.scroll_y = 0.0;
            self.rerender();
            return;
        }
        self.nav_gen += 1;
        let gen = self.nav_gen;
        self.loading = true;
        let url = self.url.clone();
        let proxy = self.proxy.clone();
        self.rt.spawn(async move {
            // DEBT-1: fetch the document AND all its subresources (scripts, CSS, images) here,
            // off the UI thread. The UI thread then builds the page with zero network calls.
            let result = manuk_page::prefetch_document(&url)
                .await
                .map_err(|e| format!("{e:#}"));
            let _ = proxy.send_event(NavEvent::Fetched { gen, result });
        });
        self.rerender(); // keep the old page visible; chrome stays responsive during the fetch
    }

    /// Start a top-level **POST navigation** off-thread (a native `<form method=post>` submission).
    /// Mirrors [`start_fetch`](Self::start_fetch), but issues a POST with `content_type` + `body`
    /// (`prefetch_document_post`, which follows the login flow's POST→redirect→GET). The result
    /// flows through the same `NavEvent::Fetched` path, so the page swaps in identically to a GET.
    /// The caller has already stashed the outgoing page, set `self.url`, and recorded history.
    fn start_post_nav(&mut self, content_type: String, body: String, initiator: String) {
        self.nav_gen += 1;
        let gen = self.nav_gen;
        self.loading = true;
        let url = self.url.clone();
        let proxy = self.proxy.clone();
        self.rt.spawn(async move {
            let result = manuk_page::prefetch_document_post(
                &url,
                &content_type,
                body.into_bytes(),
                Some(&initiator),
            )
            .await
            .map_err(|e| format!("{e:#}"));
            let _ = proxy.send_event(NavEvent::Fetched { gen, result });
        });
        self.rerender(); // keep the old page visible; chrome stays responsive during the POST
    }

    /// R1 — build the page from the off-thread-fetched document and swap it in (on the UI
    /// thread, since `Page`/`FontContext` are `!Send`). External-stylesheet/image fetches
    /// still `block_on` here (a shorter, HTTP-cached + preload-warmed wait — off-threading
    /// that phase too is the documented follow-on).
    fn finish_load(&mut self, html: String, final_url: String) {
        let (w, h) = match &self.gpu {
            Some(g) => (g.config.width, g.config.height),
            None => (self.width, 768),
        };
        let Some(page) = self.build_page_contained(&html, &final_url, w) else {
            // The page's own content brought its build down. Show that, and keep the browser — and
            // every other tab — alive. This is the Bar 0 contract.
            tracing::error!(%final_url, "navigation panicked and was CONTAINED — showing an error page");
            self.show_error(
                &final_url,
                "This page could not be displayed (the renderer failed on it).",
            );
            return;
        };
        if let Some(win) = &self.window {
            win.set_title(&format!("{} — manuk", page.title));
        }
        self.viewport = Viewport::new(w as f32, (h as f32 - CHROME_TOP).max(1.0));
        self.viewport.content_height = page.content_height;
        self.browser.set_loaded(
            self.tab_id,
            page.final_url.clone(),
            page.title.clone(),
            page.content_height,
        );
        self.page = Some(page);
        self.scroll_y = 0.0;
        self.loading = false;
        // Seed this tab's window identity (own id + opener) so postMessage source + window.opener
        // resolve before any load-time script posts a message.
        let win = self.win_id_for(self.tab_id);
        let opener = self.tab_opener.get(&self.tab_id).copied().unwrap_or(0);
        if let Some(p) = self.page.as_ref() {
            p.set_identity(win, opener);
        }
        // The page's load scripts may have kicked off fetch/XHR (SPA data hydration) — perform
        // and settle them so the first paint reflects the fetched data.
        self.pump_fetches();
        // Load scripts may also have routed (history.replaceState on boot) — reflect it.
        self.handle_history_ops();
        // ...and may have posted to their opener (the OAuth popup pattern) — route it.
        self.pump_messages();
        self.rerender();
    }

    /// L04 — the navigation resolved to a **download** (server said attachment / binary). The net
    /// layer already **streamed the file to disk** (at `path`, size `bytes`) — never buffering it in
    /// RAM nor holding it to the document deadline — so here we only record it for the menu and
    /// restore the page the user was on: a download must not replace the current page or leave the
    /// URL bar pointing at the file. Best-effort restore via the previous history entry (re-fetched
    /// from the HTTP cache); if there's none, fall back to a blank page.
    fn finish_download(&mut self, filename: String, path: std::path::PathBuf, bytes: u64) {
        tracing::info!(file = %path.display(), bytes, "download saved");
        self.downloads.push(DownloadRecord {
            filename: path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or(filename),
            path,
            bytes: bytes as usize,
        });
        self.persist_downloads();
        // Undo the navigation the download rode in on: go back to the prior page.
        if let Some(prev) = self.history.back().map(str::to_string) {
            self.goto_no_history(&prev);
        } else {
            self.url = "about:blank".to_string();
            self.start_fetch();
        }
    }

    /// Build a fully-laid-out [`Page`] from fetched HTML (parse → external CSS/images → relayout
    /// at the current zoom). Shared by [`finish_load`](Self::finish_load) (which then displays it)
    /// and the L32 prerender path (which stashes it into the bfcache). `Page`/`FontContext` are
    /// `!Send`, so this runs on the UI thread; only the network fetch that produced `html` was
    /// off-thread.
    /// DEBT-1 — the navigation completion path. Identical to [`finish_load`] except the page is
    /// built from already-fetched subresources, so **the UI thread never blocks on the network**.
    fn finish_load_prefetched(&mut self, pre: manuk_page::Prefetched) {
        let (w, h) = match &self.gpu {
            Some(g) => (g.config.width, g.config.height),
            None => (self.width, 768),
        };
        let page = self.build_prefetched(pre, w);
        if let Some(win) = &self.window {
            win.set_title(&format!("{} — manuk", page.title));
        }
        self.viewport = Viewport::new(w as f32, (h as f32 - CHROME_TOP).max(1.0));
        self.viewport.content_height = page.content_height;
        self.browser.set_loaded(
            self.tab_id,
            page.final_url.clone(),
            page.title.clone(),
            page.content_height,
        );
        self.page = Some(page);
        self.scroll_y = 0.0;
        self.loading = false;
        let win = self.win_id_for(self.tab_id);
        let opener = self.tab_opener.get(&self.tab_id).copied().unwrap_or(0);
        if let Some(p) = self.page.as_ref() {
            p.set_identity(win, opener);
        }
        self.pump_fetches();
        self.handle_history_ops();
        self.pump_messages();
        self.pump_form_submits();
        self.rerender();
        // The document is on screen NOW. Only then do the deferred scripts run and the images load.
        self.run_deferred_scripts();
        self.spawn_image_load();
        self.spawn_iframe_load();
    }

    /// Fetch each `<iframe>`'s document on a background task. **After first paint**, always: an iframe is
    /// the single most likely thing on a page to be slow, and 23% of pages have one.
    fn spawn_iframe_load(&mut self) {
        let Some(page) = self.page.as_ref() else {
            return;
        };
        let frames = page.pending_iframes();
        if frames.is_empty() {
            return;
        }
        let gen = self.nav_gen;
        let tab = self.tab_id;
        for (node, url, _w, _h) in frames {
            let proxy = self.proxy.clone();
            let id = node.0 as usize;
            self.rt.spawn(async move {
                match manuk_net::fetch(&url).await {
                    Ok(resp) => {
                        let html = resp.decoded_text();
                        let final_url = resp.final_url.to_string();
                        let _ = proxy.send_event(NavEvent::IframeReady {
                            gen,
                            tab,
                            node: id,
                            html,
                            url: final_url,
                        });
                    }
                    // Named, never swallowed (G_SILENT_FAIL). An embed that silently shows nothing is
                    // indistinguishable from a page that has no embed.
                    Err(e) => tracing::warn!(%url, "iframe document fetch failed: {e}"),
                }
            });
        }
    }

    /// An iframe's document arrived: render it into its box and repaint.
    fn finish_iframe(
        &mut self,
        gen: u64,
        tab: manuk_compositor::TabId,
        node: usize,
        html: String,
        url: String,
    ) {
        if gen != self.nav_gen || tab != self.tab_id {
            return; // a stale embed must not land in a page the user has navigated away from
        }
        if let Some(p) = self.page.as_mut() {
            p.render_iframe(manuk_dom::NodeId(node as u64), &html, &url, &self.fonts, 0);
        }
        self.rerender();
    }

    /// Run the page's `defer` / `async` / `module` scripts — **after** it has been painted — and
    /// repaint if they changed anything.
    ///
    /// This is synchronous on the UI thread, which is where script execution has to happen (the JS
    /// context owns the DOM). It is not free — on a news front page it is seconds of ad and analytics
    /// JavaScript — but the user is now *reading the article* while it runs, instead of watching a blank
    /// window, and that is the entire point. Making the execution itself yield is a separate problem
    /// with a separate name (a cancellable long task), and it is not this one.
    fn run_deferred_scripts(&mut self) {
        let (w, _h) = match &self.gpu {
            Some(g) => (g.config.width, g.config.height),
            None => (self.width, 768),
        };
        let ran = match self.page.as_mut() {
            Some(p) => p.run_deferred_scripts(&self.fonts, w as f32),
            None => 0,
        };
        if ran == 0 {
            return;
        }
        if let Some(p) = self.page.as_ref() {
            self.viewport.content_height = p.content_height;
        }
        self.pump_fetches();
        self.handle_history_ops();
        self.pump_messages();
        self.rerender();
    }

    /// Fetch the page's images on a background task and apply them when they land.
    ///
    /// Called only *after* the page has been painted. The URL list is taken on the UI thread (a cheap
    /// DOM walk, no network); the fetch and decode happen off-thread on owned data — not one `Rc`
    /// crosses the boundary, because an `Rc` held across an `.await` would make the future `!Send` and
    /// pin it right back to the UI thread, which is the mistake this exists to undo.
    fn spawn_image_load(&mut self) {
        let Some(page) = self.page.as_ref() else {
            return;
        };
        let urls = page.pending_image_urls();
        if urls.is_empty() {
            return;
        }
        let gen = self.nav_gen;
        let tab = self.tab_id;
        let proxy = self.proxy.clone();
        self.rt.spawn(async move {
            let images = manuk_page::fetch_image_urls(urls).await;
            if !images.is_empty() {
                let _ = proxy.send_event(NavEvent::ImagesReady { gen, tab, images });
            }
        });
    }

    /// Apply images that landed after first paint, then repaint ONCE.
    fn finish_images(
        &mut self,
        gen: u64,
        tab: manuk_compositor::TabId,
        images: std::collections::HashMap<String, manuk_paint::DecodedImage>,
    ) {
        // A stale response must not land in a page the user has since navigated away from.
        if gen != self.nav_gen || tab != self.tab_id {
            return;
        }
        let (w, h) = match &self.gpu {
            Some(g) => (g.config.width, g.config.height),
            None => (self.width, 768),
        };
        let filled = match self.page.as_mut() {
            Some(p) => p.apply_images_by_url(images, &self.fonts, w as f32),
            None => 0,
        };
        if filled == 0 {
            return;
        }
        // The layout reflowed (an `<img>` without intrinsic dimensions changes the box it sits in —
        // which is what happens in a real browser too), so the scroll extent moved with it.
        if let Some(p) = self.page.as_ref() {
            self.viewport.content_height = p.content_height;
        }
        let _ = h;
        self.rerender();
    }

    /// **DEBT-1: builds a page with ZERO network calls** — the subresources were already fetched
    /// off-thread into [`manuk_page::Prefetched`]. This used to `block_on` twice (external scripts,
    /// then external CSS) *on the UI thread*, freezing the window for the whole round-trip. That is
    /// what made the reload button lag.
    fn build_prefetched(&self, pre: manuk_page::Prefetched, w: u32) -> Page {
        // **Only the scripts that BLOCK paint.** The deferred ones — `defer`, `async`, and
        // `type="module"` (deferred by default in every real browser, and what every Vite bundle ships
        // as) — run in `spawn_deferred_scripts`, AFTER this page is on the screen.
        //
        // Measured on nytimes.com: ~1MB of JavaScript was executing while the window sat blank, with the
        // document already parsed, cascaded and laid out. That JS has no business being between the user
        // and the article, and no browser a person would use puts it there.
        let mut page = Page::from_prefetched_blocking_only(pre, &self.fonts, w as f32);
        page.relayout_zoomed(&self.fonts, w as f32, self.zoom);
        page
    }

    /// **The supervised navigation boundary (Part 23.2).** Building a page runs the site's own
    /// scripts against our parser, cascade and layout — which is to say it runs OUR code on
    /// adversarial input, on every navigation, forever. A panic in there used to abort the process
    /// and take every other tab with it. Now it takes the page.
    /// The graceful half of Bar 0. A contained failure the user cannot see is just a blank window;
    /// the contract is "this tab tells you it failed and the browser keeps working", not "this tab
    /// silently shows nothing".
    fn show_error(&mut self, url: &str, message: &str) {
        let html = format!(
            "<html><body style=\"font:16px system-ui;padding:48px;color:#333\">\
             <h1 style=\"font-size:22px\">This page could not be displayed</h1>\
             <p>{message}</p>\
             <p style=\"color:#777;font-size:13px\">{url}</p>\
             <p style=\"color:#777;font-size:13px\">The browser is fine — only this page failed. \
             Other tabs are unaffected.</p></body></html>"
        );
        let (w, h) = match &self.gpu {
            Some(g) => (g.config.width, g.config.height),
            None => (self.width, 768),
        };
        if let Some(page) = self.build_page_contained(&html, url, w) {
            self.viewport = Viewport::new(w as f32, (h as f32 - CHROME_TOP).max(1.0));
            self.viewport.content_height = page.content_height;
            self.page = Some(page);
        }
        self.rerender();
    }

    fn build_page_contained(&self, html: &str, final_url: &str, w: u32) -> Option<Page> {
        let fonts = &self.fonts;
        let zoom = self.zoom;
        manuk_page::contained("navigation", || {
            let mut page = Page::load(html, final_url, fonts, w as f32);
            page.relayout_zoomed(fonts, w as f32, zoom);
            page
        })
    }

    /// Fallback for a plain (non-prefetched) document — self-contained HTML only, still no network.
    fn build_page(&self, html: &str, final_url: &str, w: u32) -> Page {
        let mut page = Page::load(html, final_url, &self.fonts, w as f32);
        page.relayout_zoomed(&self.fonts, w as f32, self.zoom);
        page
    }

    /// L32 — speculatively prewarm `url` (the predicted next navigation): fetch it off-thread so
    /// a `Prewarmed` event can build it into the bfcache for an instant click. Bounded to one
    /// in-flight prewarm; skips URLs already cached or currently loading. The predictor
    /// ([`prerender::predict_next`]) has already enforced same-origin `http(s)`.
    fn prewarm(&mut self, url: String) {
        if self.prewarming.is_some()
            || url == self.url
            || self.bfcache.iter().any(|(u, _)| u == &url)
        {
            return;
        }
        self.prewarming = Some(url.clone());
        let proxy = self.proxy.clone();
        tracing::debug!(%url, "prerender: prewarming predicted next navigation");
        self.rt.spawn(async move {
            let result = manuk_page::prefetch_document(&url)
                .await
                .map_err(|e| format!("{e:#}"));
            let _ = proxy.send_event(NavEvent::Prewarmed { url, result });
        });
    }

    /// L32 — a prewarm fetch landed: build the page and stash it into the bfcache keyed by the
    /// *requested* URL (what a click will look up), without disturbing the current page. Ignores
    /// downloads and errors.
    fn finish_prewarm(
        &mut self,
        url: String,
        result: std::result::Result<manuk_page::Loaded, String>,
    ) {
        if self.prewarming.as_deref() == Some(url.as_str()) {
            self.prewarming = None;
        }
        let (w, _) = match &self.gpu {
            Some(g) => (g.config.width, g.config.height),
            None => (self.width, 768),
        };
        match result {
            Ok(manuk_page::Loaded::Prefetched(pre)) => {
                if url == self.url || self.bfcache.iter().any(|(u, _)| u == &url) {
                    return; // navigated there already, or a duplicate — nothing to cache
                }
                let page = self.build_prefetched(*pre, w);
                self.bfcache
                    .push((url.clone(), BfEntry { page, scroll: 0.0 }));
                while self.bfcache.len() > BFCACHE_CAP {
                    self.bfcache.remove(0);
                }
                tracing::info!(%url, "prerender: page built into bfcache (next click is instant)");
            }
            Ok(manuk_page::Loaded::Document { html, final_url }) => {
                if url == self.url || self.bfcache.iter().any(|(u, _)| u == &url) {
                    return; // navigated there already, or a duplicate — nothing to cache
                }
                let page = self.build_page(&html, &final_url, w);
                self.bfcache
                    .push((url.clone(), BfEntry { page, scroll: 0.0 }));
                while self.bfcache.len() > BFCACHE_CAP {
                    self.bfcache.remove(0);
                }
                tracing::info!(%url, "prerender: page built into bfcache (next click is instant)");
            }
            Ok(manuk_page::Loaded::Download { .. }) => {} // never prewarm a download
            Err(e) => tracing::debug!(%url, "prerender: prewarm failed: {e}"),
        }
    }

    /// Mark the frame stale and ask winit for a redraw; the paint itself happens in
    /// `RedrawRequested` (see [`Self::paint_and_upload`]). Cheap — safe to call per event.
    fn rerender(&mut self) {
        self.needs_paint = true;
        if let Some(win) = &self.window {
            win.request_redraw();
        }
    }

    /// Do the actual CPU paint of the current page/scroll/overlays and upload it to the GPU
    /// texture. Called once per frame from `RedrawRequested` when `needs_paint` is set.
    fn paint_and_upload(&mut self) {
        // Once per frame, tell the page it scrolled (if it asked to know). Doing this here rather
        // than per wheel event is the difference between one JS re-entry per painted frame and
        // dozens per frame that no one will ever see.
        self.flush_scroll_notification();
        let Some(gpu) = &self.gpu else {
            return;
        };
        let (w, h) = (gpu.config.width, gpu.config.height);
        // The page is painted **below** the chrome band: shifting the scroll by
        // -CHROME_HEIGHT moves page content down so its top sits just under the toolbar.
        let mut canvas = match &self.page {
            // Route through the page's own painter so decoded <img> bitmaps are blitted.
            Some(page) => page.paint_scrolled(&self.fonts, w, h, self.scroll_y - CHROME_TOP),
            None => Canvas::new(w, h, Rgba::WHITE),
        };

        // Text selection highlight (offset into the page region).
        if self.sel_start.is_some() && self.sel_end.is_some() {
            const SEL: Rgba = Rgba {
                r: 66,
                g: 133,
                b: 244,
                a: 90,
            };
            let dy = CHROME_TOP - self.scroll_y;
            let (_, rects) = self.selection_text_and_rects();
            for (x, y, rw, rh) in rects {
                canvas.fill_rect_blended(x, y + dy, rw, rh, SEL);
            }
        }

        // E1 find-in-page highlights (offset into the page region).
        if self.find_open && !self.find_session.is_empty() {
            const HIGHLIGHT: Rgba = Rgba {
                r: 255,
                g: 235,
                b: 59,
                a: 110,
            };
            const ACTIVE: Rgba = Rgba {
                r: 255,
                g: 145,
                b: 0,
                a: 255,
            };
            let dy = CHROME_TOP - self.scroll_y;
            for r in self.find_session.all_rects() {
                canvas.fill_rect_blended(r.x, r.y + dy, r.width, r.height, HIGHLIGHT);
            }
            if let Some(m) = self.find_session.active_match() {
                let b = m.bounds();
                canvas.stroke_rect(b.x, b.y + dy, b.width, b.height, ACTIVE, 2.0);
            }
        }

        self.draw_focus_caret(&mut canvas);
        self.draw_scrollbar(&mut canvas, w as f32, h as f32);
        self.draw_chrome(&mut canvas, w);
        self.draw_overlays(&mut canvas, w as f32);

        // #2: upload only the rows that changed since last frame (exact row diff), so a
        // small update touches a small band and an unchanged frame uploads nothing.
        let (cw, ch) = (canvas.width(), canvas.height());
        let bytes = canvas.rgba_bytes();
        let damage = match &self.prev_canvas {
            Some((pw, ph, prev)) if *pw == cw && *ph == ch => row_damage(prev, bytes, cw),
            _ => Some((0, ch)), // size changed or first frame → full upload
        };
        if let Some(gpu) = &mut self.gpu {
            match damage {
                Some((y0, h)) => gpu.upload_damage(&canvas, y0, h),
                None => {} // nothing changed
            }
        }
        self.prev_canvas = Some((cw, ch, bytes.to_vec()));
    }

    /// Draw a text caret at the end of the focused field's value (a thin dark bar), in the
    /// page region (offset by the chrome band and scroll).
    fn draw_focus_caret(&self, canvas: &mut Canvas) {
        let Some(node) = self.focused_input else {
            return;
        };
        let Some(page) = self.page.as_ref() else {
            return;
        };
        let rects = page.root_box.node_rects(page.dom());
        let Some(r) = rects.get(&node) else { return };
        const INK: Rgba = Rgba {
            r: 30,
            g: 30,
            b: 30,
            a: 255,
        };
        // Page content is drawn shifted down by the chrome band and up by the scroll.
        let dy = CHROME_TOP - self.scroll_y;

        // Prefer the field's actual value run: the caret sits at the end of the glyphs,
        // spanning the text's own line box, so it tracks the text baseline instead of the
        // box centre (which diverge — the value is top-aligned in the content box, not
        // box-centred). Fall back to the content edge for an empty field.
        let (caret_x, top, h) = match page.root_box.value_run(node) {
            Some((end_x, line_top, line_h)) => {
                let caret_x = end_x.min(r.x + r.width - 3.0);
                let h = (line_h - 2.0).max(10.0);
                (caret_x, line_top + 1.0 + dy, h)
            }
            None => {
                // Empty field: a short caret near the content top-left, box-centred.
                let h = 14.0_f32.min((r.height - 6.0).max(10.0));
                let caret_x = r.x + 6.0;
                (caret_x, r.y + (r.height - h) / 2.0 + dy, h)
            }
        };
        canvas.fill_rect(caret_x, top, 1.5, h, INK);
    }

    /// Per-tab strip layout: `(id, x, width)` packed left-to-right, each width in
    /// `[TAB_MIN_W, TAB_MAX_W]` and shrunk to fit. Shared by the painter and click hit-test so
    /// they never disagree.
    fn tab_layout(&self, w: f32) -> Vec<(TabId, f32, f32)> {
        let tabs = self.browser.tabs();
        let n = tabs.len().max(1) as f32;
        let avail = (w - NEWTAB_W - 2.0).max(1.0);
        let tw = (avail / n).clamp(TAB_MIN_W.min(avail), TAB_MAX_W);
        tabs.iter()
            .enumerate()
            .map(|(i, t)| (t.id, i as f32 * tw, tw))
            .collect()
    }

    /// The x of the `+` new-tab button (just past the last tab, clamped into the window).
    fn new_tab_button_x(&self, w: f32) -> f32 {
        let end = self
            .tab_layout(w)
            .last()
            .map(|(_, x, tw)| x + tw)
            .unwrap_or(0.0);
        end.min(w - NEWTAB_W).max(0.0)
    }

    /// Draw the full top chrome: the tab strip, then the toolbar (nav buttons + address field
    /// + hamburger). Coordinates for the toolbar are offset below the strip by `TAB_STRIP_H`.
    fn draw_chrome(&self, canvas: &mut Canvas, w: u32) {
        const STRIP_BG: Rgba = Rgba {
            r: 222,
            g: 223,
            b: 227,
            a: 255,
        };
        const TAB_BG: Rgba = Rgba {
            r: 236,
            g: 237,
            b: 240,
            a: 255,
        };
        const TAB_ACTIVE: Rgba = Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        const BAND: Rgba = Rgba {
            r: 240,
            g: 240,
            b: 242,
            a: 255,
        };
        const FIELD: Rgba = Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        const BORDER: Rgba = Rgba {
            r: 205,
            g: 205,
            b: 210,
            a: 255,
        };
        const INK: Rgba = Rgba {
            r: 40,
            g: 40,
            b: 45,
            a: 255,
        };
        const HINT: Rgba = Rgba {
            r: 150,
            g: 150,
            b: 155,
            a: 255,
        };
        let w = w as f32;

        let font = |size: f32, color: Rgba| TextStyle {
            decoration: Default::default(),
            font_key: FontKey {
                family: FontFamily::SansSerif,
                bold: false,
                italic: false,
            },
            font_size: size,
            color,
            line_height: size + 3.0,
        };

        // --- Tab strip ---
        canvas.fill_rect(0.0, 0.0, w, TAB_STRIP_H, STRIP_BG);
        let active = self.browser.active();
        let tab_base = TAB_STRIP_H / 2.0 + 4.0;
        for (i, (id, x, tw)) in self.tab_layout(w).into_iter().enumerate() {
            let is_active = active == Some(id);
            let bg = if is_active { TAB_ACTIVE } else { TAB_BG };
            canvas.fill_rect(x + 1.0, 3.0, tw - 2.0, TAB_STRIP_H - 3.0, bg);
            if !is_active {
                canvas.fill_rect(x + tw - 1.0, 7.0, 1.0, TAB_STRIP_H - 12.0, BORDER);
            }
            let title = self
                .browser
                .tabs()
                .get(i)
                .map(|t| match (t.title.trim().is_empty(), t.url.is_empty()) {
                    (false, _) => t.title.clone(),
                    (true, false) => t.url.clone(),
                    (true, true) => "New Tab".to_string(),
                })
                .unwrap_or_else(|| "Tab".to_string());
            canvas.draw_text(
                &self.fonts,
                x + 10.0,
                tab_base,
                &clip_text(&title, tw - 30.0),
                &font(13.0, INK),
            );
            canvas.draw_text(
                &self.fonts,
                x + tw - 16.0,
                tab_base,
                "\u{00D7}",
                &font(14.0, HINT),
            ); // × close
        }
        let ntx = self.new_tab_button_x(w);
        canvas.draw_text(&self.fonts, ntx + 9.0, tab_base, "+", &font(18.0, INK));

        // --- Toolbar (below the strip) ---
        let top = TAB_STRIP_H;
        canvas.fill_rect(0.0, top, w, CHROME_HEIGHT, BAND);
        canvas.fill_rect(0.0, CHROME_TOP - 1.0, w, 1.0, BORDER);
        let baseline = top + CHROME_HEIGHT / 2.0 + 5.0;
        let back_ink = if self.page.is_some() { INK } else { HINT };
        canvas.draw_text(
            &self.fonts,
            14.0,
            baseline,
            "\u{2039}",
            &font(15.0, back_ink),
        ); // ‹
        canvas.draw_text(
            &self.fonts,
            40.0,
            baseline,
            "\u{203A}",
            &font(15.0, back_ink),
        ); // ›
        canvas.draw_text(
            &self.fonts,
            68.0,
            baseline - 1.0,
            "\u{25CB}",
            &font(15.0, INK),
        ); // ○ reload

        // Address/search field — width comes from the shared geometry so it never overlaps the
        // right-hand controls.
        let (minus_x, pct_x, plus_x, star_x, _hx, field_w) = Self::toolbar_geom(w);
        let field_x = 100.0;
        canvas.fill_rect(field_x, top + 7.0, field_w, CHROME_HEIGHT - 14.0, FIELD);
        canvas.stroke_rect(
            field_x,
            top + 7.0,
            field_w,
            CHROME_HEIGHT - 14.0,
            BORDER,
            1.0,
        );
        let (text, ink) = if self.omnibox_open {
            (format!("{}\u{2502}", self.omnibox_input), INK)
        } else if self.url.is_empty() || self.url == "about:blank" {
            ("Search or enter address".to_string(), HINT)
        } else {
            (self.url.clone(), INK)
        };
        canvas.draw_text(
            &self.fonts,
            field_x + 10.0,
            baseline,
            &clip_text(&text, field_w - 20.0),
            &font(15.0, ink),
        );

        // Chromium-style zoom controls: − [ 100% ] +, side by side, clickable repeatedly.
        const ACCENT: Rgba = Rgba {
            r: 26,
            g: 115,
            b: 232,
            a: 255,
        };
        let zoom_ink = if (self.zoom - 1.0).abs() > 0.01 {
            ACCENT
        } else {
            INK
        };
        canvas.draw_text(
            &self.fonts,
            minus_x,
            baseline,
            "\u{2212}",
            &font(16.0, zoom_ink),
        ); // −
        canvas.draw_text(
            &self.fonts,
            pct_x,
            baseline,
            &format!("{}%", (self.zoom * 100.0).round() as i32),
            &font(12.0, zoom_ink),
        );
        canvas.draw_text(&self.fonts, plus_x, baseline, "+", &font(16.0, zoom_ink)); // +

        // Bookmark star — FILLED when the page is bookmarked. The user must be able to SEE the
        // state, not infer it from a log line (§1.8).
        let (star, star_ink) = if self.is_bookmarked() {
            ("\u{2605}", ACCENT) // ★
        } else {
            ("\u{2606}", INK) // ☆
        };
        canvas.draw_text(&self.fonts, star_x, baseline, star, &font(16.0, star_ink));

        // Hamburger menu (three bars) at the right edge.
        let hx = w - 30.0;
        for k in 0..3 {
            canvas.fill_rect(hx, top + 15.0 + k as f32 * 6.0, 16.0, 2.0, INK);
        }
    }

    /// Re-run find over the current fragment tree (after a query edit, zoom, or resize).
    fn refresh_find(&mut self) {
        self.find_session = match &self.page {
            Some(p) => find::find(&p.root_box, &self.find_query, false),
            None => FindSession::default(),
        };
        self.scroll_to_active_match();
    }

    fn scroll_to_active_match(&mut self) {
        if let Some(m) = self.find_session.active_match() {
            let b = m.bounds();
            self.scroll_y = b.y - self.viewport.height / 3.0;
            self.clamp_scroll();
        }
    }

    /// Rank and surface omnibox suggestions for the current input.
    fn log_suggestions(&self) {
        let s = chrome::suggestions(
            &self.omnibox_input,
            self.history.entries(),
            &self.bookmarks,
            6,
        );
        if s.is_empty() {
            tracing::info!(input = %self.omnibox_input, "omnibox: no suggestions");
        } else {
            for (i, sug) in s.iter().enumerate() {
                tracing::info!(
                    "omnibox {}: [{:?}] {} — {}",
                    i + 1,
                    sug.source,
                    sug.url,
                    sug.title
                );
            }
        }
    }

    /// Suggestions to show under the omnibox. With text typed, the ranked matches; with the
    /// box empty (e.g. opened via the History menu item), the most recent history, newest
    /// first — so the dropdown doubles as an accessible history list.
    fn current_suggestions(&self) -> Vec<chrome::Suggestion> {
        if self.omnibox_input.trim().is_empty() {
            self.history
                .entries()
                .iter()
                .rev()
                .take(8)
                .map(|u| chrome::Suggestion {
                    url: u.clone(),
                    title: u.clone(),
                    source: chrome::SuggestionSource::History,
                })
                .collect()
        } else {
            chrome::suggestions(
                &self.omnibox_input,
                self.history.entries(),
                &self.bookmarks,
                8,
            )
        }
    }

    /// Draw the open overlays (omnibox suggestions dropdown, hamburger menu) above the page.
    fn draw_overlays(&self, canvas: &mut Canvas, w: f32) {
        const WHITE: Rgba = Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        const BORDER: Rgba = Rgba {
            r: 205,
            g: 205,
            b: 210,
            a: 255,
        };
        const INK: Rgba = Rgba {
            r: 40,
            g: 40,
            b: 45,
            a: 255,
        };
        const HINT: Rgba = Rgba {
            r: 130,
            g: 130,
            b: 138,
            a: 255,
        };
        let font = |size: f32, color: Rgba| TextStyle {
            decoration: Default::default(),
            font_key: FontKey {
                family: FontFamily::SansSerif,
                bold: false,
                italic: false,
            },
            font_size: size,
            color,
            line_height: size + 3.0,
        };

        // FIND BAR (Tick 18 CRITICAL fix). Ctrl+F used to only set a flag and log "type to search"
        // — no UI was ever drawn, so the user pressed it and saw NOTHING, i.e. it read as broken.
        // A real bar: the query, a live match count, and the keys that drive it.
        if self.find_open {
            const ACCENT: Rgba = Rgba {
                r: 26,
                g: 115,
                b: 232,
                a: 255,
            };
            let bw = 380.0f32.min(w - 24.0);
            let x = w - bw - 12.0;
            let y = CHROME_TOP + 8.0;
            let bh = 38.0;
            canvas.fill_rect(x, y, bw, bh, WHITE);
            canvas.stroke_rect(x, y, bw, bh, BORDER, 1.0);

            let n = self.find_session.len();
            let cur = self.find_session.active_index();
            let q = &self.find_query;
            let label = if q.is_empty() {
                "Find in page…".to_string()
            } else {
                format!("{q}\u{2502}")
            };
            let ink = if q.is_empty() { HINT } else { INK };
            canvas.draw_text(
                &self.fonts,
                x + 12.0,
                y + 24.0,
                &clip_text(&label, bw - 150.0),
                &font(14.0, ink),
            );

            // Match count — the feedback that tells the user it is actually working.
            let count = if q.is_empty() {
                String::new()
            } else if n == 0 {
                "No results".to_string()
            } else {
                format!("{cur}/{n}")
            };
            let count_ink = if n == 0 && !q.is_empty() {
                Rgba {
                    r: 200,
                    g: 60,
                    b: 60,
                    a: 255,
                }
            } else {
                ACCENT
            };
            canvas.draw_text(
                &self.fonts,
                x + bw - 132.0,
                y + 24.0,
                &count,
                &font(13.0, count_ink),
            );
            canvas.draw_text(
                &self.fonts,
                x + bw - 66.0,
                y + 24.0,
                "\u{2039} \u{203A}  Esc",
                &font(12.0, HINT),
            );
        }

        // A brief toast (e.g. "Bookmarked") — an action the user takes must be observable.
        if let Some((msg, at)) = &self.toast {
            if at.elapsed() < std::time::Duration::from_millis(1600) {
                let tw = 150.0f32;
                let x = (w - tw) / 2.0;
                let y = CHROME_TOP + 10.0;
                canvas.fill_rect(
                    x,
                    y,
                    tw,
                    30.0,
                    Rgba {
                        r: 45,
                        g: 45,
                        b: 50,
                        a: 235,
                    },
                );
                canvas.draw_text(
                    &self.fonts,
                    x + 14.0,
                    y + 20.0,
                    msg,
                    &font(
                        13.0,
                        Rgba {
                            r: 255,
                            g: 255,
                            b: 255,
                            a: 255,
                        },
                    ),
                );
            }
        }

        // Downloads panel (EPOCH-1 COMPLETENESS fix): a real, visible surface for the menu item.
        if self.downloads_open {
            let pw = 460.0f32.min(w - 40.0);
            let x = w - pw - 12.0;
            let rows = self.downloads.len().max(1);
            let h = 34.0 + rows as f32 * 34.0;
            canvas.fill_rect(x, CHROME_TOP, pw, h, WHITE);
            canvas.stroke_rect(x, CHROME_TOP, pw, h, BORDER, 1.0);
            canvas.draw_text(
                &self.fonts,
                x + 12.0,
                CHROME_TOP + 22.0,
                "Downloads",
                &font(14.0, INK),
            );
            if self.downloads.is_empty() {
                canvas.draw_text(
                    &self.fonts,
                    x + 12.0,
                    CHROME_TOP + 52.0,
                    &clip_text(
                        &format!(
                            "No downloads yet — saved to {}",
                            manuk_net::downloads::download_dir().display()
                        ),
                        pw - 24.0,
                    ),
                    &font(12.0, HINT),
                );
            } else {
                for (i, d) in self.downloads.iter().rev().enumerate() {
                    let y = CHROME_TOP + 34.0 + i as f32 * 34.0;
                    canvas.draw_text(
                        &self.fonts,
                        x + 12.0,
                        y + 16.0,
                        &clip_text(&d.filename, pw - 24.0),
                        &font(13.0, INK),
                    );
                    let kb = (d.bytes as f64 / 1024.0).max(0.1);
                    canvas.draw_text(
                        &self.fonts,
                        x + 12.0,
                        y + 29.0,
                        &clip_text(&format!("{kb:.1} KB — {}", d.path.display()), pw - 24.0),
                        &font(11.0, HINT),
                    );
                }
            }
        }

        if self.omnibox_open {
            let sugg = self.current_suggestions();
            if !sugg.is_empty() {
                let x = 100.0;
                let iw = (w - x - 44.0).max(20.0);
                let h = sugg.len() as f32 * SUGGEST_ITEM_H + 4.0;
                canvas.fill_rect(x, CHROME_TOP, iw, h, WHITE);
                canvas.stroke_rect(x, CHROME_TOP, iw, h, BORDER, 1.0);
                for (i, s) in sugg.iter().enumerate() {
                    let y = CHROME_TOP + 2.0 + i as f32 * SUGGEST_ITEM_H;
                    canvas.draw_text(
                        &self.fonts,
                        x + 12.0,
                        y + 19.0,
                        &clip_text(&s.title, iw * 0.42),
                        &font(14.0, INK),
                    );
                    canvas.draw_text(
                        &self.fonts,
                        x + iw * 0.45,
                        y + 19.0,
                        &clip_text(&s.url, iw * 0.5),
                        &font(13.0, HINT),
                    );
                }
            }
        }

        if self.menu_open {
            let x = (w - MENU_W - 6.0).max(0.0);
            let h = MENU.len() as f32 * MENU_ITEM_H + 8.0;
            canvas.fill_rect(x, CHROME_TOP, MENU_W, h, WHITE);
            canvas.stroke_rect(x, CHROME_TOP, MENU_W, h, BORDER, 1.0);
            for (i, (label, _)) in MENU.iter().enumerate() {
                let y = CHROME_TOP + 4.0 + i as f32 * MENU_ITEM_H;
                canvas.draw_text(
                    &self.fonts,
                    x + 16.0,
                    y + MENU_ITEM_H / 2.0 + 4.0,
                    label,
                    &font(14.0, INK),
                );
            }
        }
    }

    /// Hit-test a click against the hamburger menu items. Returns the action, or `None` if the
    /// click is outside the menu panel (which the caller treats as "close the menu").
    fn menu_item_at(&self, cx: f32, cy: f32, w: f32) -> Option<MenuAction> {
        let x = (w - MENU_W - 6.0).max(0.0);
        let h = MENU.len() as f32 * MENU_ITEM_H + 8.0;
        if cx < x || cx > x + MENU_W || cy < CHROME_TOP || cy > CHROME_TOP + h {
            return None;
        }
        let idx = ((cy - CHROME_TOP - 4.0) / MENU_ITEM_H).floor();
        if idx < 0.0 {
            return None;
        }
        MENU.get(idx as usize).map(|(_, a)| *a)
    }

    /// Hit-test a click against the suggestions dropdown. Returns the URL to navigate to.
    fn suggestion_at(&self, cx: f32, cy: f32, w: f32) -> Option<String> {
        let sugg = self.current_suggestions();
        if sugg.is_empty() {
            return None;
        }
        let x = 100.0;
        let iw = (w - x - 44.0).max(20.0);
        let h = sugg.len() as f32 * SUGGEST_ITEM_H + 4.0;
        if cx < x || cx > x + iw || cy < CHROME_TOP || cy > CHROME_TOP + h {
            return None;
        }
        let idx = ((cy - CHROME_TOP - 2.0) / SUGGEST_ITEM_H).floor();
        if idx < 0.0 {
            return None;
        }
        sugg.get(idx as usize).map(|s| s.url.clone())
    }

    /// Scrollbar geometry `(track_y, track_h, thumb_y, thumb_h)` at window height `h`, or
    /// `None` when the page fits (nothing to scroll). The track spans below the chrome.
    fn scrollbar_geom(&self, h: f32) -> Option<(f32, f32, f32, f32)> {
        let vp_h = self.viewport.height;
        let content = self.viewport.content_height;
        let max = (content - vp_h).max(0.0);
        if max <= 0.5 || content <= 0.0 {
            return None;
        }
        let track_y = CHROME_TOP;
        let track_h = (h - CHROME_TOP).max(1.0);
        let thumb_h = (vp_h / content * track_h).clamp(24.0, track_h);
        let thumb_y = track_y + (self.scroll_y / max) * (track_h - thumb_h);
        Some((track_y, track_h, thumb_y, thumb_h))
    }

    /// Draw the scrollbar (track + thumb) on the right edge, if the page overflows.
    fn draw_scrollbar(&self, canvas: &mut Canvas, w: f32, h: f32) {
        const TRACK: Rgba = Rgba {
            r: 244,
            g: 244,
            b: 246,
            a: 255,
        };
        const THUMB: Rgba = Rgba {
            r: 178,
            g: 180,
            b: 188,
            a: 255,
        };
        if let Some((track_y, track_h, thumb_y, thumb_h)) = self.scrollbar_geom(h) {
            let x = w - SCROLLBAR_W;
            canvas.fill_rect(x, track_y, SCROLLBAR_W, track_h, TRACK);
            canvas.fill_rect(
                x + 2.0,
                thumb_y + 1.0,
                SCROLLBAR_W - 4.0,
                thumb_h - 2.0,
                THUMB,
            );
        }
    }

    /// If `(cx, cy)` is on the scrollbar, begin a thumb drag (or page-jump toward the click) and
    /// return true so the caller stops further click handling.
    fn scrollbar_press(&mut self, cx: f32, cy: f32, w: f32, h: f32) -> bool {
        if cx < w - SCROLLBAR_W {
            return false;
        }
        let Some((track_y, track_h, thumb_y, thumb_h)) = self.scrollbar_geom(h) else {
            return false;
        };
        if cy >= thumb_y && cy <= thumb_y + thumb_h {
            self.scrollbar_drag = Some(cy - thumb_y); // grab offset within the thumb
        } else {
            // Click on the track above/below the thumb: jump so the thumb centers on the click.
            let max = self.viewport.max_scroll();
            let denom = (track_h - thumb_h).max(1.0);
            self.scroll_y = ((cy - track_y - thumb_h / 2.0) / denom * max).clamp(0.0, max);
            self.scrollbar_drag = Some(thumb_h / 2.0);
            self.rerender();
        }
        true
    }

    /// Continue a scrollbar thumb drag: map the cursor to a scroll offset.
    fn scrollbar_drag_to(&mut self, cy: f32, h: f32) {
        let Some(grab) = self.scrollbar_drag else {
            return;
        };
        let Some((track_y, track_h, _, thumb_h)) = self.scrollbar_geom(h) else {
            return;
        };
        let max = self.viewport.max_scroll();
        let denom = (track_h - thumb_h).max(1.0);
        self.scroll_y = ((cy - grab - track_y) / denom * max).clamp(0.0, max);
        self.rerender();
    }

    /// Put `text` on the system clipboard. Best-effort (a headless/no-clipboard environment
    /// just no-ops with a log).
    fn set_clipboard(&self, text: &str) {
        if text.is_empty() {
            return;
        }
        match arboard::Clipboard::new().and_then(|mut c| c.set_text(text.to_string())) {
            Ok(()) => tracing::info!(bytes = text.len(), "copied to clipboard"),
            Err(e) => tracing::warn!("clipboard write failed: {e}"),
        }
    }

    /// Read text from the system clipboard, if any.
    fn clipboard_text(&self) -> Option<String> {
        arboard::Clipboard::new()
            .and_then(|mut c| c.get_text())
            .ok()
    }

    /// Every laid-out text run in the current page as `(x, top, width, height, text)` in
    /// document coordinates — the basis for selection hit-testing and copy.
    fn page_text_runs(&self) -> Vec<(f32, f32, f32, f32, String)> {
        let mut out = Vec::new();
        if let Some(page) = &self.page {
            page.root_box.walk(&mut |b| {
                if let BoxContent::Inline(frags) = &b.content {
                    for f in frags {
                        if f.text.trim().is_empty() {
                            continue;
                        }
                        let h = (f.baseline - f.line_top).max(f.style.font_size) + 3.0;
                        out.push((f.x, f.line_top, f.width, h, f.text.clone()));
                    }
                }
            });
        }
        out
    }

    /// The selected text and its highlight rects (document coords), for the current
    /// anchor→cursor selection. Flow (line-based) selection: whole lines between the endpoints,
    /// trimmed to the anchor x on the first line and the cursor x on the last.
    fn selection_text_and_rects(&self) -> (String, Vec<(f32, f32, f32, f32)>) {
        let (Some((ax, ay)), Some((bx, by))) = (self.sel_start, self.sel_end) else {
            return (String::new(), Vec::new());
        };
        // Order endpoints top-to-bottom (then left-to-right on the same line).
        let ((sx, sy), (ex, ey)) = if (ay, ax) <= (by, bx) {
            ((ax, ay), (bx, by))
        } else {
            ((bx, by), (ax, ay))
        };

        let mut runs = self.page_text_runs();
        runs.sort_by(|a, b| {
            (a.1, a.0)
                .partial_cmp(&(b.1, b.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Which runs fall in the vertical band (by run vertical midpoint).
        let in_band: Vec<_> = runs
            .into_iter()
            .filter(|(_, y, _, h, _)| {
                let mid = y + h / 2.0;
                mid >= sy - 1.0 && mid <= ey + 1.0
            })
            .collect();
        if in_band.is_empty() {
            return (String::new(), Vec::new());
        }
        let top_line = in_band.iter().map(|r| r.1).fold(f32::MAX, f32::min);
        let bot_line = in_band.iter().map(|r| r.1).fold(f32::MIN, f32::max);
        let single_line = (bot_line - top_line).abs() < 1.0;

        let mut rects = Vec::new();
        let mut lines: Vec<(f32, Vec<String>)> = Vec::new();
        for (x, y, w, h, text) in in_band {
            // Trim the boundary lines to the horizontal selection extent.
            let on_top = (y - top_line).abs() < 1.0;
            let on_bot = (y - bot_line).abs() < 1.0;
            if single_line {
                if x + w < sx || x > ex {
                    continue;
                }
            } else if on_top && x + w < sx {
                continue;
            } else if on_bot && x > ex {
                continue;
            }
            rects.push((x, y, w, h));
            match lines.last_mut() {
                Some((ly, words)) if (*ly - y).abs() < 1.0 => words.push(text),
                _ => lines.push((y, vec![text])),
            }
        }
        let text = lines
            .iter()
            .map(|(_, w)| w.join(" "))
            .collect::<Vec<_>>()
            .join("\n");
        (text, rects)
    }

    /// Select the entire page (Ctrl+A): span from the top-left to below the last run.
    fn select_all(&mut self) {
        let runs = self.page_text_runs();
        if runs.is_empty() {
            return;
        }
        let max_y = runs.iter().map(|r| r.1 + r.3).fold(0.0_f32, f32::max);
        let max_x = runs.iter().map(|r| r.0 + r.2).fold(0.0_f32, f32::max);
        self.sel_start = Some((0.0, 0.0));
        self.sel_end = Some((max_x + 1.0, max_y + 1.0));
        self.rerender();
    }

    /// Copy the current selection to the clipboard (Ctrl+C).
    fn copy_selection(&self) {
        let (text, _) = self.selection_text_and_rects();
        self.set_clipboard(&text);
    }

    /// Open any URLs the page requested via `window.open(...)` as new tabs (resolved against
    /// the current page), focusing the last one — the multi-window/OAuth-popup pattern.
    fn handle_window_opens(&mut self) {
        let opener_tab = self.tab_id;
        let opener_win = self.win_id_for(opener_tab);
        for (win_id, raw) in manuk_js::take_window_opens() {
            let resolved = url::Url::parse(&self.url)
                .ok()
                .and_then(|base| base.join(&raw).ok())
                .map(|u| u.to_string())
                .unwrap_or(raw);
            let id = self.browser.open(resolved.clone());
            // Bind the JS-allocated window id to this new tab so a later postMessage to the
            // returned handle routes here, and record its opener for `window.opener`.
            self.win_to_tab.insert(win_id, id);
            self.tab_win.insert(id, win_id);
            self.tab_opener.insert(id, opener_win);
            self.tab_id = id;
            self.focus_tab(id);
            tracing::info!(url = %resolved, win = win_id, opener = opener_win, "window.open -> new tab");
        }
    }

    /// The process-unique JS window id for `tab`, allocating (and recording the reverse map) on
    /// first use. Every tab that runs JS needs one so `postMessage` routing + `window.opener`
    /// resolve.
    fn win_id_for(&mut self, tab: TabId) -> u64 {
        if let Some(w) = self.tab_win.get(&tab) {
            return *w;
        }
        let w = manuk_js::next_window_id();
        self.tab_win.insert(tab, w);
        self.win_to_tab.insert(w, tab);
        w
    }

    /// L03 — drain page-issued cross-window `postMessage` sends and route each to its target
    /// window's tab (the active page or a background tab), firing a `message` MessageEvent there.
    /// The send queue is a single shared thread-local, so draining via the active page collects
    /// every send regardless of which document posted. Called after dispatch / load / fetches.
    fn pump_messages(&mut self) {
        let msgs = match self.page.as_ref() {
            Some(p) => p.take_messages(),
            None => return,
        };
        if msgs.is_empty() {
            return;
        }
        let w = self.viewport.width;
        for (target_win, json, origin, source_win) in msgs {
            let Some(&tab) = self.win_to_tab.get(&target_win) else {
                tracing::debug!(
                    target_win,
                    "postMessage to an unknown/closed window; dropped"
                );
                continue;
            };
            if tab == self.tab_id {
                if let Some(p) = self.page.as_mut() {
                    p.deliver_message(&json, &origin, source_win, &self.fonts, w);
                }
            } else if let Some(p) = self.browser.page_mut(tab) {
                p.deliver_message(&json, &origin, source_win, &self.fonts, w);
            }
            tracing::info!(target_win, source_win, "postMessage routed");
        }
    }

    /// Drain page-issued `fetch`/`XHR` requests, perform each over the network, and settle the
    /// page's Promise / XHR callbacks — looping so a reaction that issues a follow-on request
    /// (a common SPA pattern) also runs. Bounded to keep a runaway request chain from stalling
    /// the UI thread. Synchronous `block_on` for now (the requests are HTTP-cached); truly
    /// non-blocking async fetch is a logged follow-on.
    fn pump_fetches(&mut self) {
        let base = self
            .page
            .as_ref()
            .map(|p| p.base_url().to_string())
            .unwrap_or_else(|| self.url.clone());
        let mut did_any = false;
        for _ in 0..8 {
            let reqs = match self.page.as_ref() {
                Some(p) => p.take_fetches(),
                None => break,
            };
            if reqs.is_empty() {
                break;
            }
            for (id, raw_url, method, headers, body) in reqs {
                did_any = true;
                let url = url::Url::parse(&base)
                    .ok()
                    .and_then(|b| b.join(&raw_url).ok())
                    .map(|u| u.to_string())
                    .unwrap_or(raw_url);
                // DEBT-1: perform the request OFF the UI thread and settle it when it lands. This
                // used to `block_on` here — a page calling a slow API froze the entire browser.
                let gen = self.nav_gen;
                let proxy = self.proxy.clone();
                self.rt.spawn(async move {
                    // Replay the page's request headers onto the wire — `Authorization` and a
                    // non-JSON `Content-Type` were dropped here too, so an authenticated in-shell
                    // fetch reached the server anonymous and 401'd. Default `Content-Type` only when
                    // the page did not set one (an explicit form encoding must survive).
                    let mut hdrs: Vec<(&str, &str)> = headers
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();
                    let is_get = method.eq_ignore_ascii_case("GET") || method.is_empty();
                    if !is_get
                        && !headers
                            .iter()
                            .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    {
                        hdrs.push(("content-type", "application/json"));
                    }
                    let bytes = manuk_net::Bytes::from(body.into_bytes());
                    let (status, text) = match manuk_net::request(&method, &url, &hdrs, bytes).await
                    {
                        Ok(resp) => (resp.status, resp.text()),
                        Err(e) => {
                            tracing::warn!(url = %url, error = %e, "page fetch failed");
                            (0u16, String::new())
                        }
                    };
                    let _ = proxy.send_event(NavEvent::PageFetch {
                        gen,
                        id,
                        status,
                        body: text,
                    });
                });
            }
            // Nothing to drain synchronously any more — the responses arrive as events.
            break;
        }
        if did_any {
            // A response may have carried Set-Cookie (e.g. a session refresh); persist it.
            manuk_net::save_cookies();
            manuk_net::webstorage::save();
            self.rerender();
        }
    }

    /// Reflect page `history` ops (SPA client-side routing) into the chrome. `pushState`/
    /// `replaceState` update the omnibox URL + the back/forward stack **without a network
    /// navigation**; JS-initiated `back`/`forward`/`go` fall back to the ordinary navigate path
    /// (correct URL, though it reloads rather than firing popstate — same-document back/forward
    /// with per-entry state restore is a logged follow-on).
    fn handle_history_ops(&mut self) {
        let ops = match self.page.as_ref() {
            Some(p) => p.take_history_ops(),
            None => return,
        };
        let mut routed = false;
        for (kind, _state, url) in ops {
            match kind {
                0 => {
                    if !url.is_empty() {
                        self.history.push(url.clone());
                        self.url = url;
                        routed = true;
                    }
                }
                1 => {
                    if !url.is_empty() {
                        self.history.replace_current(url.clone());
                        self.url = url;
                        routed = true;
                    }
                }
                2 => {
                    if let Some(u) = self.history.back().map(str::to_string) {
                        self.goto_no_history(&u);
                    }
                }
                3 => {
                    if let Some(u) = self.history.forward().map(str::to_string) {
                        self.goto_no_history(&u);
                    }
                }
                4 => {
                    if let Ok(n) = url.parse::<i64>() {
                        if let Some(u) = self.history.traverse(n).map(str::to_string) {
                            self.goto_no_history(&u);
                        }
                    }
                }
                _ => {}
            }
        }
        if routed {
            // Keep the omnibox (unless the user is typing) + tab record in sync with the route.
            if !self.omnibox_open {
                self.omnibox_input = self.url.clone();
            }
            let (title, height) = self
                .page
                .as_ref()
                .map(|p| (p.title.clone(), p.content_height))
                .unwrap_or_default();
            self.browser
                .set_loaded(self.tab_id, self.url.clone(), title, height);
            self.rerender();
        }
    }

    /// Run a hamburger-menu action (the same operations as the keyboard shortcuts).
    fn run_menu_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::NewTab => self.new_tab(),
            MenuAction::DuplicateTab => self.duplicate_tab(self.tab_id),
            MenuAction::Bookmark => self.toggle_bookmark(),
            MenuAction::History => {
                // Open the omnibox empty so the dropdown shows recent history.
                self.omnibox_open = true;
                self.omnibox_input.clear();
            }
            MenuAction::Downloads => {
                // Show a real panel. (Before EPOCH-1 this only wrote to the log — the user saw
                // nothing at all, which is a dead affordance.)
                self.downloads_open = !self.downloads_open;
                self.rerender();
            }
            MenuAction::Find => {
                self.find_open = true;
                self.find_query.clear();
                self.find_session = FindSession::default();
            }
            MenuAction::ZoomIn => self.apply_zoom(chrome::zoom_in(self.zoom)),
            MenuAction::ZoomOut => self.apply_zoom(chrome::zoom_out(self.zoom)),
            MenuAction::ZoomReset => self.apply_zoom(chrome::zoom_reset()),
        }
    }

    /// E1 full-page zoom: re-lay-out at the new factor (crisp), not a bitmap scale.
    fn apply_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;
        let width = self.viewport.width;
        if let Some(page) = &mut self.page {
            page.relayout_zoomed(&self.fonts, width, zoom);
            self.viewport.content_height = page.content_height;
        }
        self.clamp_scroll();
        // Rects moved, so any find highlights must be recomputed.
        if self.find_open {
            self.refresh_find();
        }
        self.rerender();
    }

    /// Navigate to `url`, recording it in the history stack. L32: if the page was speculatively
    /// prewarmed it is already in the bfcache — serve it instantly (no fetch/pipeline) instead of
    /// a fresh load.
    fn goto(&mut self, url: &str) {
        if self.bfcache.iter().any(|(u, _)| u == url) && self.restore_from_bfcache(url) {
            tracing::info!(%url, "prerender: instant click served from prewarmed bfcache");
            self.history.push(url.to_string());
            return;
        }
        self.goto_no_history(url);
        self.history.push(url.to_string());
    }

    /// Load `url` without touching the history stack (used by back/forward). Stashes the
    /// outgoing page into the bfcache so a later Back/Forward to it is instant.
    fn goto_no_history(&mut self, url: &str) {
        self.stash_current();
        self.url = url.to_string();
        self.start_fetch(); // off-thread; finish_load swaps the page in when the fetch returns
    }

    /// R2 — move the current page into the bfcache (bounded LRU), keyed by its URL. A no-op
    /// for the blank/home page. Called before every navigation so the page we leave can be
    /// restored instantly on Back/Forward.
    fn stash_current(&mut self) {
        let Some(page) = self.page.take() else { return };
        let url = self.url.clone();
        if url.is_empty() || url == "about:blank" {
            return;
        }
        self.bfcache.retain(|(u, _)| u != &url);
        self.bfcache.push((
            url,
            BfEntry {
                page,
                scroll: self.scroll_y,
            },
        ));
        while self.bfcache.len() > BFCACHE_CAP {
            self.bfcache.remove(0);
        }
    }

    /// R2 — restore `url` from the bfcache instantly (swap in the constructed page + scroll,
    /// no pipeline). Returns `false` if it wasn't cached. Stashes the current page first so
    /// the page being left stays available for the opposite Back/Forward direction.
    fn restore_from_bfcache(&mut self, url: &str) -> bool {
        self.stash_current();
        let Some(pos) = self.bfcache.iter().position(|(u, _)| u == url) else {
            return false;
        };
        let (_, entry) = self.bfcache.remove(pos);
        let (w, h) = match &self.gpu {
            Some(g) => (g.config.width, g.config.height),
            None => (self.width, 768),
        };
        self.url = url.to_string();
        self.viewport = Viewport::new(w as f32, (h as f32 - CHROME_TOP).max(1.0));
        self.viewport.content_height = entry.page.content_height;
        self.scroll_y = entry.scroll;
        self.clamp_scroll();
        if let Some(win) = &self.window {
            win.set_title(&format!("{} — manuk", entry.page.title));
        }
        self.page = Some(entry.page);
        tracing::info!(%url, "restored from bfcache (instant Back/Forward)");
        self.rerender();
        true
    }

    // ---- §5 session persistence + multi-tab navigation ----

    /// Persist the current tab set so the next launch can restore it. Best-effort: a write
    /// failure is logged, never fatal. The active tab's URL/title are already current in the
    /// `Browser` (kept in sync by `load_page`), and secret-bearing URLs are redacted by the
    /// store on save.
    fn save_session(&mut self) {
        if let Some(store) = &self.store {
            let sess = session::session_of(&self.browser);
            match store.save_session(&sess) {
                Ok(()) => tracing::info!(tabs = sess.tabs.len(), "session saved"),
                Err(e) => tracing::warn!("session save failed: {e:#}"),
            }
        }
        // Persist login cookies alongside the session so they survive a restart.
        manuk_net::save_cookies();
        manuk_net::webstorage::save();
        // Bookmarks too — a backstop for the per-toggle save (e.g. a bookmark added, then quit).
        self.persist_bookmarks();
        // ...and the downloads list, a backstop for the per-download save.
        self.persist_downloads();
    }

    /// Flush a pending scroll notification — called once per frame, from the paint path. A page that
    /// registered no `scroll` listener and no observer costs nothing here: `view_changed` asks
    /// before it does any work.
    fn flush_scroll_notification(&mut self) {
        if !std::mem::take(&mut self.scroll_dirty) {
            return;
        }
        self.notify_view_scrolled();
    }

    /// Notify the page that the viewport scrolled, then apply whatever its callbacks asked for
    /// (an infinite-scroll handler routinely scrolls again, or focuses the new content).
    fn notify_view_scrolled(&mut self) {
        let (sy, vw, vh, focus) = (
            self.scroll_y,
            self.viewport.width,
            self.viewport.height,
            self.focused_input,
        );
        if let Some(page) = self.page.as_mut() {
            page.publish_view_state(0.0, sy, focus);
            page.view_changed(sy, vw, vh, true);
        }
        self.apply_view_requests();
        self.handle_window_opens();
        self.pump_fetches();
    }

    /// Apply the view changes a script asked for: scrolling and focus. The host owns the viewport
    /// and the caret, so a script *requests* and the shell *performs* — which is also what keeps a
    /// hostile page from moving the user's view behind their back at will.
    fn apply_view_requests(&mut self) {
        let (scrolls, focuses) = match self.page.as_ref() {
            Some(p) => (p.take_scroll_requests(), p.take_focus_requests()),
            None => return,
        };
        if let Some(&(_, y)) = scrolls.last() {
            let max = (self.viewport.content_height - self.viewport.height).max(0.0);
            self.scroll_y = y.clamp(0.0, max);
            self.needs_paint = true;
        }
        if let Some(&f) = focuses.last() {
            self.focused_input = f;
            self.needs_paint = true;
        }
    }

    /// Focus a tab and **wake it on focus**: a hibernated (restored/background) tab holds no
    /// `Page`, so switching to it eagerly loads its URL now — exactly the hibernation model.
    fn focus_tab(&mut self, id: TabId) {
        self.browser.focus(id);
        self.tab_id = id;
        if let Some(url) = self.browser.tab(id).map(|t| t.url.clone()) {
            // Not a navigation (no history push); just realize the focused tab's page.
            self.goto_no_history(&url);
        }
    }

    /// Cycle the focused tab by `delta` (+1 next, -1 previous), wrapping.
    fn cycle_tab(&mut self, delta: i32) {
        let ids: Vec<TabId> = self.browser.tabs().iter().map(|t| t.id).collect();
        if ids.len() < 2 {
            return;
        }
        let cur = ids.iter().position(|&i| i == self.tab_id).unwrap_or(0);
        let next = (cur as i32 + delta).rem_euclid(ids.len() as i32) as usize;
        let id = ids[next];
        self.focus_tab(id);
        tracing::info!(tab = next + 1, of = ids.len(), url = %self.url, "switched tab");
    }

    /// A click in the tab strip: the `+` button opens a tab; a tab's `×` closes it; elsewhere
    /// on a tab focuses it.
    fn handle_tab_strip_click(&mut self, cx: f32) {
        let w = self.viewport.width;
        let ntx = self.new_tab_button_x(w);
        if cx >= ntx && cx < ntx + NEWTAB_W {
            self.new_tab();
            return;
        }
        for (id, x, tw) in self.tab_layout(w) {
            if cx >= x && cx < x + tw {
                if cx >= x + tw - 22.0 {
                    self.close_tab_by_id(id);
                } else if id != self.tab_id {
                    self.focus_tab(id);
                }
                return;
            }
        }
    }

    /// Close a specific tab by id (from the strip's `×`). Closing the last tab replaces it with
    /// a fresh blank tab so the window stays open; otherwise focus falls to the active tab.
    fn close_tab_by_id(&mut self, id: TabId) {
        // Drop the closed tab's window-id routing entries so the maps don't leak.
        if let Some(w) = self.tab_win.remove(&id) {
            self.win_to_tab.remove(&w);
        }
        self.tab_opener.remove(&id);
        if self.browser.tabs().len() <= 1 {
            self.new_tab();
            self.browser.close(id);
            if let Some(a) = self.browser.active() {
                self.focus_tab(a);
            }
            return;
        }
        let was_active = id == self.tab_id;
        self.browser.close(id);
        match self.browser.active() {
            Some(a) if was_active => self.focus_tab(a),
            _ => self.rerender(),
        }
    }

    /// Duplicate a tab: open a new tab at the same URL, right after it, and focus it.
    fn duplicate_tab(&mut self, id: TabId) {
        if let Some(url) = self.browser.tab(id).map(|t| t.url.clone()) {
            let new = self.browser.open(url.clone());
            self.tab_id = new;
            self.url = url.clone();
            self.focus_tab(new);
        }
    }

    /// Open a new tab and drop into the omnibox to type its destination.
    fn new_tab(&mut self) {
        let id = self.browser.open("about:blank");
        self.tab_id = id;
        self.url = "about:blank".to_string();
        self.page = None;
        self.omnibox_open = true;
        self.omnibox_input.clear();
        tracing::info!("new tab: type a URL or search, Enter to go");
        self.rerender();
    }

    /// Close the focused tab. Closing the last tab saves and exits.
    fn close_tab(&mut self, event_loop: &ActiveEventLoop) {
        if self.browser.tabs().len() <= 1 {
            self.save_session();
            event_loop.exit();
            return;
        }
        let closing = self.tab_id;
        self.browser.close(closing);
        if let Some(id) = self.browser.active() {
            self.focus_tab(id);
        }
    }

    // ---- §3 in-browser agent panel ----

    /// URLs the agent may reopen via `open_tab`: the current tabs plus the persisted session
    /// and any saved collections. Constrains `open_tab` to places the user has actually been.
    fn known_history(&self) -> Vec<crate::session::TabRecord> {
        let mut known: Vec<crate::session::TabRecord> = self
            .browser
            .tabs()
            .iter()
            .map(|t| crate::session::TabRecord::new(t.url.clone(), t.title.clone(), t.is_pinned()))
            .collect();
        if let Some(store) = &self.store {
            if let Ok(Some(s)) = store.load_session() {
                known.extend(s.tabs);
            }
            if let Ok(names) = store.list_collections() {
                for n in names {
                    if let Ok(Some(tabs)) = store.load_collection(&n) {
                        known.extend(tabs);
                    }
                }
            }
        }
        known
    }

    /// Run one agent task over the current page — and, via the H3 tab-control seam, over the
    /// whole tab set — then hand the page back.
    ///
    /// The Ctrl+J keypress that opened the prompt **is** the consent gesture (E6/G-a): the
    /// live page moves into an [`AgentPanel`] under the **assistant** scope (read-only *page*
    /// actions plus `close_tabs`/`open_tab`/`search_tab`); the task runs against both the page
    /// and a [`crate::session::BrowserTabs`] controller over the live `Browser`; the page is
    /// handed straight back. Page content stays untrusted throughout — the panel reuses
    /// `run_task`, whose observation fence is unconditional.
    fn run_agent(&mut self, task: &str) {
        let Some(page) = self.page.take() else {
            tracing::warn!("agent: no page loaded");
            return;
        };
        // Resolve a backend from the environment; if none, tell the user how to get one.
        let llama_port = std::env::var("MANUK_LLAMA_PORT")
            .ok()
            .and_then(|s| s.parse().ok());
        manuk_agent::env::load_dotenv();
        let groq_present = manuk_agent::env::single_key().is_some();
        let Some(kind) = panel::resolve_panel_backend(llama_port, groq_present) else {
            tracing::warn!(
                "agent: no backend configured. Set MANUK_LLAMA_PORT to a local llama-server, \
                 or put GROQ_API_KEY in .env"
            );
            self.page = Some(page); // hand the page straight back, untouched
            return;
        };
        let Some(backend) = panel::build_panel_backend(&kind) else {
            tracing::warn!("agent: backend key disappeared");
            self.page = Some(page);
            return;
        };

        let (w, h) = match &self.gpu {
            Some(g) => (g.config.width, g.config.height),
            None => (self.width, 768),
        };
        let handoff = Handoff {
            page,
            scroll_y: self.scroll_y,
            history: self.history.entries().to_vec(),
        };
        let mut panel = AgentPanel::new(PanelScope::assistant(), w, h);
        panel.take_over(handoff, HandoffConsent::user_approved());

        let known = self.known_history();
        let settings = self.settings.clone();
        tracing::info!(backend = ?kind, task, "agent: running (assistant: read-only page + tab control)");
        let result = {
            let mut tabs = crate::session::BrowserTabs::new(&mut self.browser, known, settings);
            self.rt
                .block_on(panel.run_with_tabs(&backend, task, &mut tabs))
        };
        match result {
            Ok(outcome) => {
                let answer = outcome.answer.unwrap_or_else(|| "(no answer)".to_string());
                tracing::info!(steps = outcome.steps, "agent answer: {answer}");
                println!("\n[agent] {answer}\n");
                if let Some(w) = &self.window {
                    w.set_title(&format!("[agent] {} — manuk", truncate(&answer, 60)));
                }
            }
            Err(e) => tracing::error!("agent task failed: {e:#}"),
        }

        // Hand the live session back and resume the human view on the same page.
        if let Some(h) = panel.hand_back(HandoffConsent::user_approved()) {
            self.scroll_y = h.scroll_y;
            let mut page = h.page;
            page.relayout_zoomed(&self.fonts, w as f32, self.zoom);
            self.viewport.content_height = page.content_height;
            self.page = Some(page);
            self.clamp_scroll();
            self.rerender();
        }
    }

    /// E1 keyboard chrome. Returns true when the key was consumed.
    fn handle_key(&mut self, key: &Key, event_loop: &ActiveEventLoop) -> bool {
        let ctrl = self.modifiers.control_key();
        let alt = self.modifiers.alt_key();
        let shift = self.modifiers.shift_key();

        // The agent prompt captures typed text while open — this is the trusted instruction
        // channel; it never touches the page.
        if self.agent_open {
            match key {
                Key::Named(NamedKey::Escape) => {
                    self.agent_open = false;
                    self.agent_input.clear();
                    return true;
                }
                Key::Named(NamedKey::Enter) => {
                    let task = std::mem::take(&mut self.agent_input);
                    self.agent_open = false;
                    if !task.trim().is_empty() {
                        self.run_agent(task.trim());
                    }
                    return true;
                }
                Key::Named(NamedKey::Backspace) => {
                    self.agent_input.pop();
                    return true;
                }
                Key::Character(c) if !ctrl && !alt => {
                    self.agent_input.push_str(c);
                    return true;
                }
                Key::Named(NamedKey::Space) if !ctrl && !alt => {
                    self.agent_input.push(' ');
                    return true;
                }
                _ => {}
            }
        }

        // The omnibox captures typed text while open.
        if self.omnibox_open {
            match key {
                Key::Named(NamedKey::Escape) => {
                    self.omnibox_open = false;
                    self.omnibox_input.clear();
                    self.rerender();
                    return true;
                }
                Key::Named(NamedKey::Enter) => {
                    let intent = chrome::omnibox_intent(&self.omnibox_input, &self.settings);
                    tracing::info!(input = %self.omnibox_input, resolved = %intent.url(),
                                   search = matches!(intent, chrome::OmniboxIntent::Search(_)),
                                   "omnibox");
                    let url = intent.url().to_string();
                    self.omnibox_open = false;
                    self.omnibox_input.clear();
                    self.goto(&url);
                    return true;
                }
                Key::Named(NamedKey::Backspace) => {
                    self.omnibox_input.pop();
                    self.log_suggestions();
                    self.rerender();
                    return true;
                }
                Key::Character(c) if !ctrl && !alt => {
                    self.omnibox_input.push_str(c);
                    self.log_suggestions();
                    self.rerender();
                    return true;
                }
                Key::Named(NamedKey::Space) if !ctrl && !alt => {
                    self.omnibox_input.push(' ');
                    self.log_suggestions();
                    self.rerender();
                    return true;
                }
                _ => {}
            }
        }

        // While the find bar is open, most keys edit the query.
        if self.find_open {
            match key {
                Key::Named(NamedKey::Escape) => {
                    self.find_open = false;
                    self.find_query.clear();
                    self.find_session = FindSession::default();
                    self.rerender();
                    return true;
                }
                Key::Named(NamedKey::Enter) => {
                    if shift {
                        self.find_session.prev();
                    } else {
                        self.find_session.next();
                    }
                    self.scroll_to_active_match();
                    self.rerender();
                    tracing::info!(
                        "find: {} / {}",
                        self.find_session.active_index(),
                        self.find_session.len()
                    );
                    return true;
                }
                Key::Named(NamedKey::Backspace) => {
                    self.find_query.pop();
                    self.refresh_find();
                    self.rerender();
                    return true;
                }
                Key::Character(c) if !ctrl && !alt => {
                    self.find_query.push_str(c);
                    self.refresh_find();
                    self.rerender();
                    tracing::info!(
                        "find {:?}: {} match(es)",
                        self.find_query,
                        self.find_session.len()
                    );
                    return true;
                }
                _ => {}
            }
        }

        // A focused text field captures typing (unless a Ctrl/Alt chord — those still reach
        // the chrome shortcuts below, e.g. Ctrl+L).
        if self.focused_input.is_some() && !ctrl && !alt {
            match key {
                Key::Named(NamedKey::Enter) => {
                    self.submit_focused_form();
                    return true;
                }
                Key::Named(NamedKey::Escape) => {
                    self.focused_input = None;
                    self.rerender();
                    return true;
                }
                Key::Named(NamedKey::Backspace) => {
                    self.edit_focused_input("", true);
                    return true;
                }
                Key::Named(NamedKey::Space) => {
                    self.edit_focused_input(" ", false);
                    return true;
                }
                Key::Character(c) => {
                    self.edit_focused_input(c, false);
                    return true;
                }
                _ => {}
            }
        }

        match key {
            Key::Character(c) if ctrl => match c.as_str() {
                // Clipboard + selection.
                "c" | "C" => {
                    self.copy_selection();
                    true
                }
                "a" | "A" => {
                    self.select_all();
                    true
                }
                "v" | "V" => {
                    if let Some(text) = self.clipboard_text() {
                        if self.omnibox_open {
                            self.omnibox_input.push_str(&text);
                            self.rerender();
                        } else if self.focused_input.is_some() {
                            self.edit_focused_input(&text, false);
                        }
                    }
                    true
                }
                "x" | "X" => {
                    // Cut applies to editable fields (omnibox / focused input).
                    if self.omnibox_open {
                        self.set_clipboard(&self.omnibox_input.clone());
                        self.omnibox_input.clear();
                        self.rerender();
                    } else if self.focused_input.is_some() {
                        let (text, _) = self.selection_text_and_rects();
                        self.set_clipboard(&text);
                    }
                    true
                }
                "f" | "F" => {
                    self.find_open = true;
                    self.find_query.clear();
                    self.find_session = FindSession::default();
                    self.rerender(); // the bar must APPEAR (it is drawn in draw_overlays)
                    true
                }
                // Standard browser shortcuts (Tick 18 — ERGONOMICS: the bindings a user already
                // knows must work, or the browser feels broken even when the feature exists).
                "d" | "D" => {
                    self.toggle_bookmark();
                    true
                }
                "r" | "R" => {
                    self.reload();
                    true
                }
                "t" | "T" => {
                    self.new_tab();
                    true
                }
                "w" | "W" => {
                    self.close_tab_by_id(self.tab_id);
                    true
                }
                "l" | "L" => {
                    self.omnibox_open = true;
                    self.omnibox_input = if self.url == "about:blank" {
                        String::new()
                    } else {
                        self.url.clone()
                    };
                    self.rerender();
                    true
                }
                // Ctrl+'+' arrives as '=' on most layouts.
                "=" | "+" => {
                    self.apply_zoom(chrome::zoom_in(self.zoom));
                    tracing::info!(zoom = self.zoom, "zoom in");
                    true
                }
                "-" => {
                    self.apply_zoom(chrome::zoom_out(self.zoom));
                    tracing::info!(zoom = self.zoom, "zoom out");
                    true
                }
                "0" => {
                    self.apply_zoom(chrome::zoom_reset());
                    tracing::info!("zoom reset");
                    true
                }
                "l" => {
                    self.omnibox_open = true;
                    self.omnibox_input = if self.url == "about:blank" {
                        String::new()
                    } else {
                        self.url.clone()
                    };
                    self.rerender();
                    tracing::info!("omnibox: type a URL or a search, Enter to go, Esc to cancel");
                    true
                }
                // §3 — open the in-browser agent panel over the current page.
                "j" | "J" => {
                    self.agent_open = true;
                    self.agent_input.clear();
                    tracing::info!("agent: type a task (reads this page; can also open/close/search tabs), Enter to run, Esc to cancel");
                    true
                }
                // §5 — tab management.
                "t" | "T" => {
                    self.new_tab();
                    true
                }
                "w" | "W" => {
                    self.close_tab(event_loop);
                    true
                }
                "d" => {
                    let title = self
                        .page
                        .as_ref()
                        .map(|p| p.title.clone())
                        .unwrap_or_default();
                    let on = self.bookmarks.toggle(&self.url.clone(), &title);
                    self.persist_bookmarks();
                    tracing::info!(bookmarked = on, url = %self.url, "bookmark toggled");
                    true
                }
                // G-e: instant per-tab resource honesty (task manager).
                "m" | "M" => {
                    let report = self.browser.resource_report();
                    println!("\n{}", report.to_table());
                    true
                }
                "q" => {
                    self.save_session();
                    event_loop.exit();
                    true
                }
                _ => false,
            },
            // §5 — Ctrl+Tab / Ctrl+Shift+Tab cycle tabs (wake on focus).
            Key::Named(NamedKey::F5) => {
                self.reload();
                true
            }
            Key::Named(NamedKey::Tab) if ctrl => {
                self.cycle_tab(if shift { -1 } else { 1 });
                true
            }
            Key::Named(NamedKey::ArrowLeft) if alt => {
                if let Some(u) = self.history.back().map(str::to_string) {
                    self.goto_no_history(&u);
                    tracing::info!(url = %u, "back");
                }
                true
            }
            Key::Named(NamedKey::ArrowRight) if alt => {
                if let Some(u) = self.history.forward().map(str::to_string) {
                    self.goto_no_history(&u);
                    tracing::info!(url = %u, "forward");
                }
                true
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.scroll_y += 48.0;
                self.clamp_scroll();
                self.rerender();
                true
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.scroll_y -= 48.0;
                self.clamp_scroll();
                self.rerender();
                true
            }
            Key::Named(NamedKey::PageDown) => {
                self.scroll_y += self.viewport.height * 0.9;
                self.clamp_scroll();
                self.rerender();
                true
            }
            Key::Named(NamedKey::PageUp) => {
                self.scroll_y -= self.viewport.height * 0.9;
                self.clamp_scroll();
                self.rerender();
                true
            }
            _ => false,
        }
    }

    fn clamp_scroll(&mut self) {
        self.scroll_y = self.scroll_y.clamp(0.0, self.viewport.max_scroll());
    }
}

impl ApplicationHandler<NavEvent> for App {
    /// R1 — off-thread navigation work landed. Discard stale results (the user navigated
    /// again), else build + swap in the page on this (UI) thread.
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: NavEvent) {
        match event {
            NavEvent::Fetched { gen, result } => {
                if gen != self.nav_gen {
                    return; // superseded/cancelled navigation
                }
                self.loading = false;
                match result {
                    Ok(manuk_page::Loaded::Prefetched(pre)) => self.finish_load_prefetched(*pre),
                    Ok(manuk_page::Loaded::Document { html, final_url }) => {
                        self.finish_load(html, final_url)
                    }
                    Ok(manuk_page::Loaded::Download {
                        filename,
                        path,
                        bytes,
                    }) => self.finish_download(filename, path, bytes),
                    Err(e) => {
                        tracing::error!("load {}: {e}", self.url);
                        self.rerender();
                    }
                }
            }
            NavEvent::Prewarmed { url, result } => self.finish_prewarm(url, result),
            NavEvent::ImagesReady { gen, tab, images } => self.finish_images(gen, tab, images),
            NavEvent::IframeReady {
                gen,
                tab,
                node,
                html,
                url,
            } => self.finish_iframe(gen, tab, node, html, url),
            NavEvent::PageFetch {
                gen,
                id,
                status,
                body,
            } => {
                // A response for a page the user has navigated away from must not be applied.
                if gen != self.nav_gen {
                    return;
                }
                let w = self.viewport.width;
                if let Some(page) = self.page.as_mut() {
                    page.resolve_fetch(id, status, &body, &self.fonts, w);
                }
                // The reaction may have issued a follow-on fetch, mutated the DOM, routed, or
                // posted a message — pump those, then repaint.
                self.pump_fetches();
                self.handle_history_ops();
                self.pump_messages();
                manuk_net::save_cookies();
                manuk_net::webstorage::save();
                self.rerender();
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("manuk")
            .with_inner_size(LogicalSize::new(self.width as f64, 768.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                tracing::error!("create_window: {e}");
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        match pollster::block_on(Gpu::new(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
        )) {
            Ok(gpu) => {
                self.window = Some(window);
                self.gpu = Some(gpu);
            }
            Err(e) => {
                tracing::error!("wgpu init: {e:#}");
                event_loop.exit();
                return;
            }
        }
        self.start_fetch();
        self.history.push(self.url.clone());
        // On the home / new-tab page, focus the address bar so the user can type immediately.
        if self.url == "about:blank" {
            self.omnibox_open = true;
            self.omnibox_input.clear();
        }
        self.rerender();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.save_session();
                event_loop.exit();
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    self.handle_key(&event.logical_key, event_loop);
                }
            }
            WindowEvent::Resized(size) => {
                let (w, h) = (size.width.max(1), size.height.max(1));
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(w, h);
                }
                if let Some(page) = &mut self.page {
                    page.relayout_zoomed(&self.fonts, w as f32, self.zoom);
                    self.viewport.width = w as f32;
                    self.viewport.height = (h as f32 - CHROME_TOP).max(1.0);
                    self.viewport.content_height = page.content_height;
                }
                self.clamp_scroll();
                // Reflow moved every rect, so recompute the highlights.
                if self.find_open {
                    self.refresh_find();
                }
                self.rerender();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 48.0,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                self.scroll_y -= dy;
                self.clamp_scroll();
                // The page is told it scrolled ONCE PER FRAME, not once per wheel event. A trackpad
                // delivers dozens of events per frame, and notifying the page on each one means
                // re-entering JS dozens of times to report scroll positions nobody will ever paint.
                // Mark it and let the frame flush it.
                self.scroll_dirty = true;
                self.rerender();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
                // A scrollbar thumb drag takes over cursor movement.
                if self.scrollbar_drag.is_some() {
                    let win_h = self.viewport.height + CHROME_TOP;
                    self.scrollbar_drag_to(position.y as f32, win_h);
                    return;
                }
                // A drag-select in progress: extend the selection to the cursor.
                if self.selecting {
                    let doc_y = (position.y as f32 - CHROME_TOP + self.scroll_y).max(0.0);
                    self.sel_end = Some((position.x as f32, doc_y));
                    self.rerender();
                    return;
                }
                // Show a hand cursor over links / clickable controls, an arrow otherwise.
                let (cx, cy) = self.cursor;
                let action = if cy >= CHROME_TOP {
                    self.classify_page_click(cx, cy - CHROME_TOP + self.scroll_y)
                } else {
                    PageAction::Clear
                };
                let clickable = matches!(
                    action,
                    PageAction::Link(_) | PageAction::Submit(_) | PageAction::Toggle(_)
                );
                if clickable != self.over_link {
                    self.over_link = clickable;
                    // R4: speculatively warm the connection to a newly-hovered link's origin
                    // (same-origin policy + recency/budget enforced by the net Preconnector).
                    // L32: and, if the hovered link is a safe same-origin GET, prewarm the whole
                    // page into the bfcache so the click is instant.
                    if let PageAction::Link(target) = &action {
                        let cur = self.url.clone();
                        let target = target.clone();
                        {
                            let (cur, target) = (cur.clone(), target.clone());
                            self.rt.spawn(async move {
                                manuk_net::preconnect(&cur, &target).await;
                            });
                        }
                        if let Some(url) = prerender::predict_next(&cur, Some(&target), &[]) {
                            self.prewarm(url);
                        }
                    }
                    if let Some(w) = &self.window {
                        w.set_cursor(if clickable {
                            winit::window::CursorIcon::Pointer
                        } else {
                            winit::window::CursorIcon::Default
                        });
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state == ElementState::Released {
                    self.scrollbar_drag = None; // end any scrollbar drag
                    if button == winit::event::MouseButton::Left {
                        self.finish_page_interaction();
                    }
                }
                if state == ElementState::Pressed {
                    match button {
                        winit::event::MouseButton::Left => self.handle_click(),
                        // Middle-click a tab to duplicate it (a fresh tab at the same URL).
                        winit::event::MouseButton::Middle => {
                            let (cx, cy) = self.cursor;
                            if cy < TAB_STRIP_H {
                                if let Some((id, _, _)) = self
                                    .tab_layout(self.viewport.width)
                                    .into_iter()
                                    .find(|(_, x, tw)| cx >= *x && cx < *x + *tw)
                                {
                                    self.duplicate_tab(id);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                // Coalesced paint: input bursts only set `needs_paint`; do the one CPU paint
                // + texture upload here, then present.
                if self.needs_paint {
                    self.paint_and_upload();
                    self.needs_paint = false;
                }
                if let Some(gpu) = &mut self.gpu {
                    self.frame.begin();
                    if let Err(e) = gpu.draw() {
                        tracing::warn!("draw: {e:?}");
                    }
                    self.frame.end();
                    // Log frame stats once per window of frames.
                    if self.frame.len() == 120 {
                        if let (Some(avg), Some(fps)) = (self.frame.average(), self.frame.fps()) {
                            tracing::debug!(
                                frame_ms = avg.as_secs_f64() * 1000.0,
                                fps,
                                janky = self.frame.janky(manuk_compositor::FRAME_BUDGET_60FPS),
                                "gpu present frame stats (120-frame window)"
                            );
                        }
                    }
                    // §8 metric #4: `browse --frames N` renders N frames back-to-back,
                    // reports GPU-present stats, then exits — a headful measurement.
                    if let Some(n) = self.measure_frames {
                        self.frames_done += 1;
                        if self.frames_done >= n {
                            let avg = self.frame.average().unwrap_or_default();
                            let p95 = self.frame.p95().unwrap_or_default();
                            println!(
                                "gpu-present over {} frames: avg {:.2} ms ({:.0} fps), p95 {:.2} ms, jank {}/{}",
                                self.frame.len(),
                                avg.as_secs_f64() * 1000.0,
                                self.frame.fps().unwrap_or(0.0),
                                p95.as_secs_f64() * 1000.0,
                                self.frame.janky(manuk_compositor::FRAME_BUDGET_60FPS),
                                self.frame.len(),
                            );
                            event_loop.exit();
                        } else if let Some(w) = &self.window {
                            w.request_redraw(); // keep the render loop running
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Trim `s` to roughly fit `max_w` px (approximate: ~8px/char at the chrome font size),
/// appending an ellipsis when trimmed. Good enough for a toolbar; exact metrics aren't
/// needed to avoid overflow.
fn clip_text(s: &str, max_w: f32) -> String {
    let max_chars = (max_w / 8.0).max(1.0) as usize;
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let head: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{head}\u{2026}")
    }
}

/// The `(first_row, row_count)` band that differs between two equal-size RGBA buffers, or
/// `None` when they are identical. Used to upload only the damaged rows to the GPU (#2).
fn row_damage(prev: &[u8], cur: &[u8], width: u32) -> Option<(u32, u32)> {
    let stride = width as usize * 4;
    if stride == 0 || prev.len() != cur.len() {
        return None;
    }
    let rows = prev.len() / stride;
    let mut first = None;
    let mut last = 0;
    for y in 0..rows {
        let r = y * stride;
        if prev[r..r + stride] != cur[r..r + stride] {
            first.get_or_insert(y);
            last = y;
        }
    }
    first.map(|f| (f as u32, (last - f + 1) as u32))
}

/// Resolve a possibly-relative `href` against the current page URL. Returns `None` for
/// empty or pure-fragment (`#…`) links, and for `javascript:`/`mailto:` schemes the GUI
/// does not navigate to.
fn resolve_href(base: &str, href: &str) -> Option<String> {
    let h = href.trim();
    if h.is_empty() || h.starts_with('#') {
        return None;
    }
    let lower = h.to_ascii_lowercase();
    if lower.starts_with("javascript:") || lower.starts_with("mailto:") || lower.starts_with("tel:")
    {
        return None;
    }
    let resolved = match url::Url::parse(h) {
        Ok(u) => u,
        // Relative (incl. protocol-relative `//host/…`): resolve against the base document URL.
        Err(_) => url::Url::parse(base).ok().and_then(|b| b.join(h).ok())?,
    };
    Some(unwrap_redirect(resolved).to_string())
}

/// Unwrap known search-engine redirect wrappers to their real destination. DuckDuckGo
/// result links point at `//duckduckgo.com/l/?uddg=<target>`, whose endpoint serves a
/// JS/meta interstitial our engine can't follow — so we jump straight to the decoded
/// target. A no-op for every other URL.
fn unwrap_redirect(u: url::Url) -> url::Url {
    let is_ddg = u
        .host_str()
        .is_some_and(|h| h == "duckduckgo.com" || h.ends_with(".duckduckgo.com"));
    if is_ddg && u.path() == "/l/" {
        if let Some((_, target)) = u.query_pairs().find(|(k, _)| k == "uddg") {
            if let Ok(dest) = url::Url::parse(&target) {
                return dest;
            }
        }
    }
    u
}

#[cfg(test)]
mod tests {
    use super::resolve_href;

    #[test]
    fn resolve_href_handles_relative_absolute_and_skips() {
        let base = "https://example.com/dir/page.html";
        // Relative resolves against the base's directory.
        assert_eq!(
            resolve_href(base, "next.html").as_deref(),
            Some("https://example.com/dir/next.html")
        );
        assert_eq!(
            resolve_href(base, "/root.html").as_deref(),
            Some("https://example.com/root.html")
        );
        assert_eq!(
            resolve_href(base, "../up.html").as_deref(),
            Some("https://example.com/up.html")
        );
        // Absolute passes through.
        assert_eq!(
            resolve_href(base, "https://other.test/x").as_deref(),
            Some("https://other.test/x")
        );
        // Non-navigational / empty are skipped.
        assert_eq!(resolve_href(base, "#frag"), None);
        assert_eq!(resolve_href(base, ""), None);
        assert_eq!(resolve_href(base, "javascript:void(0)"), None);
        assert_eq!(resolve_href(base, "mailto:a@b.com"), None);
    }

    #[test]
    fn resolve_href_unwraps_ddg_redirect() {
        let base = "https://lite.duckduckgo.com/lite/?q=rust";
        // Protocol-relative DDG redirect resolves to the decoded `uddg` target.
        let got = resolve_href(
            base,
            "//duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org%2F&rut=abc",
        );
        assert_eq!(got.as_deref(), Some("https://rust-lang.org/"));
        // A plain duckduckgo.com link is untouched.
        assert_eq!(
            resolve_href(base, "https://duckduckgo.com/about").as_deref(),
            Some("https://duckduckgo.com/about")
        );
    }
}

/// Truncate a string to `max` chars for a window-title summary, adding an ellipsis.
fn truncate(s: &str, max: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= max {
        s
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
    }
}

/// GPU state: surface, device/queue, the present pipeline, and the current frame
/// texture + bind group uploaded from the CPU canvas.
struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    bind_group: Option<wgpu::BindGroup>,
    /// The page texture, kept across frames (#2). Re-created only when the canvas size
    /// changes; otherwise `upload` writes into it in place instead of allocating a fresh
    /// texture + bind group every frame.
    texture: Option<wgpu::Texture>,
}

impl Gpu {
    async fn new(window: Arc<Window>, width: u32, height: u32) -> Result<Gpu> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window).context("create_surface")?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("no suitable GPU adapter")?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("manuk device"),
                ..Default::default()
            })
            .await
            .context("request_device")?;

        let config = surface
            .get_default_config(&adapter, width, height)
            .context("surface unsupported by adapter")?;
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("present shader"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("present pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        Ok(Gpu {
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group_layout,
            sampler,
            bind_group: None,
            texture: None,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Upload a CPU canvas as the texture to present next frame.
    /// Upload the whole `canvas` to the (persistent) page texture. Allocates the texture +
    /// bind group only when the size changes; otherwise reuses them (#2).
    fn upload(&mut self, canvas: &manuk_paint::Canvas) {
        self.ensure_texture(canvas.width(), canvas.height());
        self.write_region(canvas, 0, 0, canvas.width(), canvas.height());
    }

    /// Upload only the rows `[y0, y0+h)` of `canvas` — the damaged band — into the persistent
    /// texture, skipping the untouched rows (#2, partial damage upload). Falls back to a full
    /// upload if the texture must be (re)allocated. Rows (not a sub-rect) so the copy stays a
    /// single contiguous `write_texture` with the canvas' natural row stride.
    fn upload_damage(&mut self, canvas: &manuk_paint::Canvas, y0: u32, h: u32) {
        let full = self.texture.is_none()
            || self
                .texture
                .as_ref()
                .is_some_and(|t| t.width() != canvas.width() || t.height() != canvas.height());
        if full {
            self.upload(canvas);
            return;
        }
        let y0 = y0.min(canvas.height());
        let h = h.min(canvas.height() - y0);
        if h == 0 {
            return;
        }
        self.write_region(canvas, 0, y0, canvas.width(), h);
    }

    /// Create the page texture + bind group at `(w, h)` if absent or a different size.
    fn ensure_texture(&mut self, w: u32, h: u32) {
        if self
            .texture
            .as_ref()
            .is_some_and(|t| t.width() == w && t.height() == h)
        {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("page texture"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("page bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        }));
        self.texture = Some(texture);
    }

    /// Copy the rows `[y .. y+h)` (full width) of `canvas` into the persistent texture at the
    /// same rows. `x`/`w` span the full width (the shared row stride keeps the copy contiguous).
    fn write_region(&mut self, canvas: &manuk_paint::Canvas, _x: u32, y: u32, w: u32, h: u32) {
        let Some(texture) = &self.texture else { return };
        let cw = canvas.width();
        let start = (y as usize) * (cw as usize) * 4;
        let end = start + (h as usize) * (cw as usize) * 4;
        let bytes = &canvas.rgba_bytes()[start..end];
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * cw),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }

    fn draw(&mut self) -> Result<(), wgpu::SurfaceError> {
        let Some(bind_group) = &self.bind_group else {
            return Ok(()); // nothing uploaded yet
        };
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.config);
                self.surface.get_current_texture()?
            }
            Err(e) => return Err(e),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("present encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}
