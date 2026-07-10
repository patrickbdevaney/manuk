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
use manuk_layout::TextStyle;
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
use crate::session::{self, SessionStore};
use crate::tab::Browser;
use manuk_agent::Handoff;
use manuk_compositor::TabId;
use manuk_page::{fetch_html, Page};

/// Height of the browser chrome band (toolbar) drawn above the page, in physical px.
const CHROME_HEIGHT: f32 = 44.0;

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

    /// Last known cursor position in physical window pixels (for click hit-testing).
    cursor: (f32, f32),
    /// The text `<input>`/`<textarea>` node currently focused for typing, if any.
    focused_input: Option<manuk_dom::NodeId>,
    /// Whether the cursor is currently over a clickable link (drives the hand cursor).
    over_link: bool,
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
            cursor: (0.0, 0.0),
            focused_input: None,
            over_link: false,
        }
    }

    /// Handle a left click at the current cursor: chrome (toolbar) if it lands in the band,
    /// else follow a link under it on the page.
    fn handle_click(&mut self) {
        let (cx, cy) = self.cursor;
        if cy < CHROME_HEIGHT {
            self.handle_chrome_click(cx);
            return;
        }
        // Page document coordinates: undo the chrome offset, add the scroll.
        let (doc_x, doc_y) = (cx, cy - CHROME_HEIGHT + self.scroll_y);
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
                let name = dom.element(node).and_then(|e| e.attr("name")).map(str::to_string);
                // Clear the whole radio group, then check this one.
                let group: Vec<manuk_dom::NodeId> = dom
                    .descendants(dom.root())
                    .filter(|&n| {
                        dom.tag_name(n) == Some("input")
                            && dom.element(n).and_then(|e| e.attr("type")).is_some_and(|t| t.eq_ignore_ascii_case("radio"))
                            && dom.element(n).and_then(|e| e.attr("name")).map(str::to_string) == name
                    })
                    .collect();
                for n in group {
                    page.dom_mut().remove_attr(n, "checked");
                }
                page.dom_mut().set_attr(node, "checked", "");
            } else {
                let checked = dom.element(node).is_some_and(|e| e.attr("checked").is_some());
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
        let action = self.page.as_ref().and_then(|page| {
            let dom = page.dom();
            let form = manuk_agent::forms::owning_form(dom, node)?;
            manuk_agent::forms::submission_url(dom, form, &page.final_url).ok()
        });
        match action {
            Some(url) => {
                tracing::info!(url = %url, "submit: form GET");
                self.goto(&url);
            }
            None => tracing::info!("submit: no form / non-GET method (ignored)"),
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

    /// A click within the chrome band: back / forward / reload buttons, or the address field.
    fn handle_chrome_click(&mut self, x: f32) {
        if x < 30.0 {
            if let Some(u) = self.history.back().map(str::to_string) {
                self.goto_no_history(&u);
            }
        } else if x < 56.0 {
            if let Some(u) = self.history.forward().map(str::to_string) {
                self.goto_no_history(&u);
            }
        } else if x < 92.0 {
            let u = self.url.clone();
            if !u.is_empty() && u != "about:blank" {
                self.goto_no_history(&u); // reload
            }
        } else {
            // Focus the address field: open the omnibox pre-filled with the current URL.
            self.omnibox_open = true;
            self.omnibox_input = if self.url == "about:blank" { String::new() } else { self.url.clone() };
            self.rerender();
        }
    }

    fn load_page(&mut self, width: u32, height: u32) {
        // The home / new-tab page has no document — just chrome over a blank canvas.
        if self.url.is_empty() || self.url == "about:blank" {
            self.page = None;
            self.viewport = Viewport::new(width as f32, (height as f32 - CHROME_HEIGHT).max(1.0));
            if let Some(w) = &self.window {
                w.set_title("New Tab — manuk");
            }
            return;
        }
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
                self.viewport = Viewport::new(width as f32, (height as f32 - CHROME_HEIGHT).max(1.0));
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
        let Some(gpu) = &self.gpu else {
            return;
        };
        let (w, h) = (gpu.config.width, gpu.config.height);
        // The page is painted **below** the chrome band: shifting the scroll by
        // -CHROME_HEIGHT moves page content down so its top sits just under the toolbar.
        let mut canvas = match &self.page {
            Some(page) => CpuPainter::new(&self.fonts).render_scrolled(
                &page.root_box,
                w,
                h,
                Rgba::WHITE,
                self.scroll_y - CHROME_HEIGHT,
            ),
            None => Canvas::new(w, h, Rgba::WHITE),
        };

        // E1 find-in-page highlights (offset into the page region).
        if self.find_open && !self.find_session.is_empty() {
            const HIGHLIGHT: Rgba = Rgba { r: 255, g: 235, b: 59, a: 110 };
            const ACTIVE: Rgba = Rgba { r: 255, g: 145, b: 0, a: 255 };
            let dy = CHROME_HEIGHT - self.scroll_y;
            for r in self.find_session.all_rects() {
                canvas.fill_rect_blended(r.x, r.y + dy, r.width, r.height, HIGHLIGHT);
            }
            if let Some(m) = self.find_session.active_match() {
                let b = m.bounds();
                canvas.stroke_rect(b.x, b.y + dy, b.width, b.height, ACTIVE, 2.0);
            }
        }

        self.draw_focus_caret(&mut canvas);
        self.draw_chrome(&mut canvas, w);

        if let Some(gpu) = &mut self.gpu {
            gpu.upload(&canvas);
        }
        if let Some(win) = &self.window {
            win.request_redraw();
        }
    }

    /// Draw a text caret at the end of the focused field's value (a thin dark bar), in the
    /// page region (offset by the chrome band and scroll).
    fn draw_focus_caret(&self, canvas: &mut Canvas) {
        let Some(node) = self.focused_input else { return };
        let Some(page) = self.page.as_ref() else { return };
        let rects = page.root_box.node_rects(page.dom());
        let Some(r) = rects.get(&node) else { return };
        let value = page.dom().element(node).and_then(|e| e.attr("value")).unwrap_or("");
        let key = FontKey { family: FontFamily::SansSerif, bold: false, italic: false };
        // 20px is the form-control default font size; measure the value to place the caret.
        let font_size = 16.0;
        let tw = self.fonts.measure(value, key, font_size);
        let caret_x = (r.x + 7.0 + tw).min(r.x + r.width - 3.0);
        let top = r.y + CHROME_HEIGHT - self.scroll_y + 4.0;
        let h = (r.height - 8.0).max(10.0);
        const INK: Rgba = Rgba { r: 30, g: 30, b: 30, a: 255 };
        canvas.fill_rect(caret_x, top, 1.5, h, INK);
    }

    /// Draw the browser chrome (toolbar) over the top [`CHROME_HEIGHT`] px: nav buttons, the
    /// address/search field, and its current text (the URL, or the omnibox input while
    /// editing).
    fn draw_chrome(&self, canvas: &mut Canvas, w: u32) {
        const BAND: Rgba = Rgba { r: 240, g: 240, b: 242, a: 255 };
        const FIELD: Rgba = Rgba { r: 255, g: 255, b: 255, a: 255 };
        const BORDER: Rgba = Rgba { r: 205, g: 205, b: 210, a: 255 };
        const INK: Rgba = Rgba { r: 40, g: 40, b: 45, a: 255 };
        const HINT: Rgba = Rgba { r: 150, g: 150, b: 155, a: 255 };

        canvas.fill_rect(0.0, 0.0, w as f32, CHROME_HEIGHT, BAND);
        canvas.fill_rect(0.0, CHROME_HEIGHT - 1.0, w as f32, 1.0, BORDER);

        let font = |color: Rgba| TextStyle {
            font_key: FontKey { family: FontFamily::SansSerif, bold: false, italic: false },
            font_size: 15.0,
            color,
            line_height: 18.0,
        };
        let baseline = CHROME_HEIGHT / 2.0 + 5.0;
        // Nav "buttons" (drawn as glyphs; their hit zones are in `handle_click`).
        let back_ink = if self.page.is_some() { INK } else { HINT };
        canvas.draw_text(&self.fonts, 14.0, baseline, "\u{2039}", &font(back_ink)); // ‹
        canvas.draw_text(&self.fonts, 40.0, baseline, "\u{203A}", &font(back_ink)); // ›
        canvas.draw_text(&self.fonts, 68.0, baseline - 1.0, "\u{25CB}", &font(INK)); // ○ reload

        // Address/search field.
        let field_x = 100.0;
        let field_w = (w as f32 - field_x - 12.0).max(20.0);
        canvas.fill_rect(field_x, 7.0, field_w, CHROME_HEIGHT - 14.0, FIELD);
        canvas.stroke_rect(field_x, 7.0, field_w, CHROME_HEIGHT - 14.0, BORDER, 1.0);

        let (text, ink) = if self.omnibox_open {
            (format!("{}\u{2502}", self.omnibox_input), INK) // trailing caret
        } else if self.url.is_empty() || self.url == "about:blank" {
            ("Search or enter address".to_string(), HINT)
        } else {
            (self.url.clone(), INK)
        };
        // Clip the text to the field width by trimming from the left when overlong.
        let padded = field_x + 10.0;
        canvas.draw_text(&self.fonts, padded, baseline, &clip_text(&text, field_w - 20.0), &font(ink));
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
        let mut panel = AgentPanel::new(PanelScope::assistant(), w, h);
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
        let known = self.known_history();
        let settings = self.settings.clone();
        tracing::info!(backend = ?kind, task, "agent: running (assistant: read-only page + tab control)");
        let result = {
            let mut tabs = crate::session::BrowserTabs::new(&mut self.browser, known, settings);
            rt.block_on(panel.run_with_tabs(&backend, task, &mut tabs))
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
                    self.omnibox_input = if self.url == "about:blank" { String::new() } else { self.url.clone() };
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
                    self.viewport.height = (h as f32 - CHROME_HEIGHT).max(1.0);
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
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
                // Show a hand cursor over links / clickable controls, an arrow otherwise.
                let (cx, cy) = self.cursor;
                let clickable = cy >= CHROME_HEIGHT
                    && matches!(
                        self.classify_page_click(cx, cy - CHROME_HEIGHT + self.scroll_y),
                        PageAction::Link(_) | PageAction::Submit(_) | PageAction::Toggle(_)
                    );
                if clickable != self.over_link {
                    self.over_link = clickable;
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
                if state == ElementState::Pressed && button == winit::event::MouseButton::Left {
                    self.handle_click();
                }
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

/// Resolve a possibly-relative `href` against the current page URL. Returns `None` for
/// empty or pure-fragment (`#…`) links, and for `javascript:`/`mailto:` schemes the GUI
/// does not navigate to.
fn resolve_href(base: &str, href: &str) -> Option<String> {
    let h = href.trim();
    if h.is_empty() || h.starts_with('#') {
        return None;
    }
    let lower = h.to_ascii_lowercase();
    if lower.starts_with("javascript:") || lower.starts_with("mailto:") || lower.starts_with("tel:") {
        return None;
    }
    match url::Url::parse(h) {
        Ok(u) => Some(u.to_string()),
        // Relative: resolve against the base document URL.
        Err(_) => url::Url::parse(base)
            .ok()
            .and_then(|b| b.join(h).ok())
            .map(|u| u.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_href;

    #[test]
    fn resolve_href_handles_relative_absolute_and_skips() {
        let base = "https://example.com/dir/page.html";
        // Relative resolves against the base's directory.
        assert_eq!(resolve_href(base, "next.html").as_deref(), Some("https://example.com/dir/next.html"));
        assert_eq!(resolve_href(base, "/root.html").as_deref(), Some("https://example.com/root.html"));
        assert_eq!(resolve_href(base, "../up.html").as_deref(), Some("https://example.com/up.html"));
        // Absolute passes through.
        assert_eq!(resolve_href(base, "https://other.test/x").as_deref(), Some("https://other.test/x"));
        // Non-navigational / empty are skipped.
        assert_eq!(resolve_href(base, "#frag"), None);
        assert_eq!(resolve_href(base, ""), None);
        assert_eq!(resolve_href(base, "javascript:void(0)"), None);
        assert_eq!(resolve_href(base, "mailto:a@b.com"), None);
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
