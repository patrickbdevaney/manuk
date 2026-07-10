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
use manuk_paint::CpuPainter;
use manuk_text::FontContext;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::chrome::{self, Bookmarks, History, Settings};
use crate::find::{self, FindSession};
use crate::panel::{self, AgentPanel, HandoffConsent, PanelScope};
use crate::session::{self, SessionStore};
use crate::tab::Browser;
use manuk_agent::Handoff;
use manuk_compositor::TabId;
use manuk_page::{fetch_html, Page};

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
pub fn run(url: String, width: u32, measure_frames: Option<usize>) -> Result<()> {
    let event_loop = EventLoop::new().context("creating winit event loop")?;
    let mut app = App::new(url, width, measure_frames);
    event_loop.run_app(&mut app).context("running event loop")?;
    Ok(())
}

struct App {
    url: String,
    width: u32,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    fonts: FontContext,
    page: Option<Page>,
    viewport: Viewport,
    scroll_y: f32,
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

    // ---- §5 session persistence ----
    /// The on-disk tab store (session + collections), outside the repo. `None` if the state
    /// directory could not be resolved — persistence is then silently disabled.
    store: Option<SessionStore>,

    // ---- §3 in-browser agent panel ----
    /// Ctrl+J opens a task prompt; typed text goes to the agent, not the page.
    agent_open: bool,
    agent_input: String,
}

impl App {
    fn new(url: String, width: u32, measure_frames: Option<usize>) -> Self {
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
                    tracing::info!(tabs = n, "restored prior session (hibernated; only the focused tab loads)");
                }
                Ok(None) => {}
                Err(e) => tracing::warn!("session restore skipped: {e:#}"),
            }
        }

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
            page: None,
            viewport: Viewport::new(width as f32, 768.0),
            scroll_y: 0.0,
            browser,
            tab_id,
            frame: manuk_compositor::FrameTimer::new(240),
            measure_frames,
            frames_done: 0,
            modifiers: ModifiersState::empty(),
            history: History::new(),
            bookmarks: Bookmarks::new(),
            settings: Settings::default(),
            zoom: 1.0,
            find_open: false,
            find_query: String::new(),
            find_session: FindSession::default(),
            omnibox_open: false,
            omnibox_input: String::new(),
            store,
            agent_open: false,
            agent_input: String::new(),
        }
    }

    fn load_page(&mut self, width: u32, height: u32) {
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("tokio runtime: {e}");
                return;
            }
        };
        match rt.block_on(fetch_html(&self.url)) {
            Ok((html, final_url)) => {
                let page = Page::load(&html, &final_url, &self.fonts, width as f32);
                if let Some(w) = &self.window {
                    w.set_title(&format!("{} — manuk", page.title));
                }
                self.viewport = Viewport::new(width as f32, height as f32);
                self.viewport.content_height = page.content_height;
                self.browser.set_loaded(
                    self.tab_id,
                    page.final_url.clone(),
                    page.title.clone(),
                    page.content_height,
                );
                self.page = Some(page);
            }
            Err(e) => tracing::error!("load {}: {e:#}", self.url),
        }
    }

    fn rerender(&mut self) {
        let (Some(gpu), Some(page)) = (&mut self.gpu, &self.page) else {
            return;
        };
        let mut canvas = CpuPainter::new(&self.fonts).render_scrolled(
            &page.root_box,
            gpu.config.width,
            gpu.config.height,
            Rgba::WHITE,
            self.scroll_y,
        );

        // E1 find-in-page: highlights are an **overlay** composited after paint, so
        // finding text never mutates the DOM or triggers a relayout.
        if self.find_open && !self.find_session.is_empty() {
            const HIGHLIGHT: Rgba = Rgba { r: 255, g: 235, b: 59, a: 110 };
            const ACTIVE: Rgba = Rgba { r: 255, g: 145, b: 0, a: 255 };
            for r in self.find_session.all_rects() {
                canvas.fill_rect_blended(r.x, r.y - self.scroll_y, r.width, r.height, HIGHLIGHT);
            }
            if let Some(m) = self.find_session.active_match() {
                let b = m.bounds();
                canvas.stroke_rect(b.x, b.y - self.scroll_y, b.width, b.height, ACTIVE, 2.0);
            }
        }

        gpu.upload(&canvas);
        if let Some(w) = &self.window {
            w.request_redraw();
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
                tracing::info!("omnibox {}: [{:?}] {} — {}", i + 1, sug.source, sug.url, sug.title);
            }
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

    /// Navigate to `url`, recording it in the history stack.
    fn goto(&mut self, url: &str) {
        self.goto_no_history(url);
        self.history.push(url.to_string());
    }

    /// Load `url` without touching the history stack (used by back/forward).
    fn goto_no_history(&mut self, url: &str) {
        let (w, h) = match &self.gpu {
            Some(g) => (g.config.width, g.config.height),
            None => (self.width, 768),
        };
        self.url = url.to_string();
        self.load_page(w, h);
        if let Some(page) = &mut self.page {
            page.relayout_zoomed(&self.fonts, w as f32, self.zoom);
            self.viewport.content_height = page.content_height;
        }
        self.scroll_y = 0.0;
        self.rerender();
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

    /// Run one read-only agent task over the current page, then hand the session back.
    ///
    /// The Ctrl+J keypress that opened the prompt **is** the consent gesture (E6/G-a): the
    /// live page moves into an [`AgentPanel`] under a **read-only** scope (it may look and
    /// scroll and answer, never mutate the page or navigate away), the task runs, and the
    /// page is handed straight back. Page content stays untrusted throughout — the panel
    /// reuses `run_task`, whose observation fence is unconditional.
    fn run_agent(&mut self, task: &str) {
        let Some(page) = self.page.take() else {
            tracing::warn!("agent: no page loaded");
            return;
        };
        // Resolve a backend from the environment; if none, tell the user how to get one.
        let llama_port = std::env::var("MANUK_LLAMA_PORT").ok().and_then(|s| s.parse().ok());
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
        let mut panel = AgentPanel::new(PanelScope::read_only(), w, h);
        panel.take_over(handoff, HandoffConsent::user_approved());

        let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("agent runtime: {e}");
                // Recover the page from the panel so the user is not left blank.
                if let Some(h) = panel.hand_back(HandoffConsent::user_approved()) {
                    self.page = Some(h.page);
                }
                return;
            }
        };
        tracing::info!(backend = ?kind, task, "agent: running (read-only)");
        match rt.block_on(panel.run(&backend, task)) {
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
                    return true;
                }
                Key::Character(c) if !ctrl && !alt => {
                    self.omnibox_input.push_str(c);
                    self.log_suggestions();
                    return true;
                }
                Key::Named(NamedKey::Space) if !ctrl && !alt => {
                    self.omnibox_input.push(' ');
                    self.log_suggestions();
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

        match key {
            Key::Character(c) if ctrl => match c.as_str() {
                "f" => {
                    self.find_open = true;
                    self.find_query.clear();
                    self.find_session = FindSession::default();
                    tracing::info!("find-in-page: type to search, Enter/Shift+Enter to cycle, Esc to close");
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
                    self.omnibox_input.clear();
                    tracing::info!("omnibox: type a URL or a search, Enter to go, Esc to cancel");
                    true
                }
                // §3 — open the in-browser agent panel over the current page.
                "j" | "J" => {
                    self.agent_open = true;
                    self.agent_input.clear();
                    tracing::info!("agent (read-only): type a task about this page, Enter to run, Esc to cancel");
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

impl ApplicationHandler for App {
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
        self.load_page(size.width.max(1), size.height.max(1));
        self.history.push(self.url.clone());
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
                    self.viewport.height = h as f32;
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
                self.rerender();
            }
            WindowEvent::RedrawRequested => {
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
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Upload a CPU canvas as the texture to present next frame.
    fn upload(&mut self, canvas: &manuk_paint::Canvas) {
        let size = wgpu::Extent3d {
            width: canvas.width(),
            height: canvas.height(),
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("page texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            canvas.rgba_bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * canvas.width()),
                rows_per_image: Some(canvas.height()),
            },
            size,
        );
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
