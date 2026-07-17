//! manuk-js — the JavaScript runtime boundary and Web API bindings.
//!
//! # Modification boundary (read CLAUDE.md § "before touching the JS engine")
//!
//! This crate **configures and binds to** SpiderMonkey (via `mozjs`, the same path
//! Servo uses — *not* V8). It must never patch SpiderMonkey's JIT (Warp/Ion) or GC
//! internals, nor alter how untrusted JS is sandboxed. Those are "come back to the
//! human" boundaries: JIT miscompilation is historically the largest source of
//! exploitable browser RCE, and the reason SpiderMonkey is trustworthy is years of
//! adversarial fuzzing this project has no equivalent of. Everything here stays in
//! the well-specified, low-blast-radius FFI/binding layer.
//!
//! # Phase note
//!
//! The current phase is a GUI browser with **no** LLM/agent code and no script
//! execution requirement. The default [`new_runtime`] therefore returns
//! [`NoScriptRuntime`], a no-op. Real script execution is opt-in via the
//! `spidermonkey` cargo feature, which is compiled and proven to build but not on
//! the default path.

/// A minimal JS value surfaced across the FFI boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum JsValue {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
}

/// A script evaluation error (parse/throw/engine).
#[derive(Clone, Debug)]
pub struct JsError {
    pub message: String,
}

impl std::fmt::Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JS error: {}", self.message)
    }
}
impl std::error::Error for JsError {}

/// The pluggable JS engine boundary. Implementations own a runtime + global.
///
/// The DOM/Web API surface (the large-volume follow-on called out in CLAUDE.md) is
/// installed onto the runtime's global by the binding layer; see [`bindings`].
pub trait JsRuntime {
    /// Evaluate `source` (named `filename` for diagnostics) and return its value.
    fn eval(&mut self, source: &str, filename: &str) -> Result<JsValue, JsError>;

    /// Human-readable name of the backing engine, for diagnostics/about pages.
    fn engine_name(&self) -> &'static str;
}

/// The default runtime: executes nothing. Correct for the GUI-only, no-JS phase —
/// `<script>` content is parsed into the DOM but not run.
#[derive(Debug, Default)]
pub struct NoScriptRuntime {
    ignored: usize,
}

impl NoScriptRuntime {
    pub fn new() -> Self {
        Self::default()
    }
    /// How many scripts have been handed to (and ignored by) this runtime.
    pub fn ignored_count(&self) -> usize {
        self.ignored
    }
}

impl JsRuntime for NoScriptRuntime {
    fn eval(&mut self, _source: &str, filename: &str) -> Result<JsValue, JsError> {
        self.ignored += 1;
        tracing::debug!(%filename, "script ignored (NoScriptRuntime; JS disabled this phase)");
        Ok(JsValue::Undefined)
    }
    fn engine_name(&self) -> &'static str {
        "none (script execution disabled for the GUI-only phase)"
    }
}

/// Construct the active JS runtime. Returns SpiderMonkey when the `spidermonkey`
/// feature is enabled, otherwise the no-op [`NoScriptRuntime`].
pub fn new_runtime() -> Box<dyn JsRuntime> {
    #[cfg(feature = "_sm")]
    {
        match spidermonkey::SpiderMonkeyRuntime::new() {
            Ok(rt) => return Box::new(rt),
            Err(e) => {
                tracing::error!("SpiderMonkey init failed ({e}); falling back to NoScriptRuntime");
            }
        }
    }
    Box::new(NoScriptRuntime::new())
}

/// Web API / DOM bindings surface.
///
/// This is where `manuk_dom::Dom` is projected into the JS runtime as `document`,
/// `Element`, `Node`, etc., generated from WebIDL and verified per-API against WPT
/// (CLAUDE.md § DOM). It is intentionally a stub in this phase — the trait boundary
/// and crate wiring exist so the binding work has a home that does not entangle the
/// parse/layout path with the JS engine.
pub mod bindings {
    use manuk_dom::Dom;

    /// Marker for a not-yet-installed Web API surface. Real bindings will install
    /// `document` and the DOM interface objects onto a runtime global.
    pub struct WebApiSurface<'a> {
        pub document: &'a Dom,
    }

    impl<'a> WebApiSurface<'a> {
        pub fn new(document: &'a Dom) -> Self {
            WebApiSurface { document }
        }
    }
}

/// Run a document's inline `<script>`s against its DOM, mutating the arena in place, using a
/// **process/thread-global** SpiderMonkey runtime created once on first use.
///
/// The runtime is intentionally leaked (never dropped): dropping and re-creating a
/// SpiderMonkey `Runtime` in one process is unsupported and crashes on teardown, so a
/// single long-lived runtime is both the correct model and the way to avoid that. Each
/// document gets a fresh global (the navigation model). Returns how many scripts ran.
///
/// A no-op returning `Ok(0)` when built without the `spidermonkey` feature (the default),
/// so the parse/layout path is unchanged for JS-less builds.
///
/// `layout` maps each element's `NodeId` to its border box `[x, y, width, height]` from a
/// pre-script layout snapshot, so `element.getBoundingClientRect()` returns real geometry.
/// The one SpiderMonkey [`Runtime`] per thread (the UI thread, in the shell). Shared by every
/// JS entry point so load and later event dispatch reach the same runtime (SpiderMonkey is
/// thread-affine; one live runtime per thread). Held in `ManuallyDrop` — leaked deliberately
/// for the process lifetime, since tearing a runtime down mid-process is the fragile path.
#[cfg(feature = "_sm")]
#[cfg(feature = "_sm")]
fn with_runtime<R>(
    f: impl FnOnce(&mut mozjs::rust::Runtime) -> Result<R, JsError>,
) -> Result<R, JsError> {
    use mozjs::rust::Runtime;

    // The runtime lives in the SAME thread-local as the engine (`spidermonkey::JS_THREAD`), and that
    // is the entire fix for the exit segfault: two separate thread-locals have an *unspecified* drop
    // order relative to one another, so the only way to guarantee "context first, then JS_ShutDown()"
    // is to put them in one struct and let its `Drop` say so. See `spidermonkey::JsThread`.
    let handle = spidermonkey::engine_handle()?;
    spidermonkey::RUNTIME.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(std::mem::ManuallyDrop::new(Runtime::new(handle)));
        }
        let rt: &mut Runtime = slot.as_mut().expect("runtime just initialized");
        let out = f(rt);

        // Arm the automatic teardown HERE — after script has actually run, not merely after
        // `Runtime::new`. Thread-local destructors run in REVERSE registration order, and mozjs
        // registers thread-locals of its own *lazily*: `trace_traceables` (which `Runtime::drop`
        // walks, via `finishRoots`) does not exist until the first `rooted!`, which happens inside the
        // first eval. Arm before that and mozjs registers after us, is destroyed before us, and our
        // teardown dies reaching for a thread-local that is already gone.
        //
        // **To run first at teardown, register last.** So we register after the JS has run.
        spidermonkey::arm_teardown();
        out
    })
}

/// Tear the thread's JS runtime down **before the process exits**.
///
/// SpiderMonkey installs an `atexit` handler. If the process exits with a live `JSContext`, that
/// handler runs against a context nobody destroyed and **segfaults** — after `main` has returned, so
/// the exit code is 139 and the output looks perfectly fine. That is not a cosmetic crash: it aborts
/// the rest of the exit handlers, which is where a browser flushes its cookie jar and `localStorage`
/// to the profile. A crash on the way out is how a browser silently loses a user's data (ADR-009).
///
/// Call this once, last, after every `Page` (and therefore every `PageContext`) has been dropped —
/// dropping a rooted JS object *after* its runtime is gone would crash in turn.
/// Every `<canvas>` a script has drawn into since the last call: `(node id, width, height, RGBA8)`.
///
/// Non-premultiplied RGBA8 — the exact shape `manuk_paint::DecodedImage` wants — so the host can drop
/// these straight into the image map an `<img>` lands in, and the painter never has to know that a
/// canvas exists. Empty without the JS feature, which is correct: no scripts, nothing drawn.
pub fn canvas_bitmaps() -> Vec<(u64, u32, u32, Vec<u8>)> {
    #[cfg(feature = "_sm")]
    {
        canvas::take_dirty()
    }
    #[cfg(not(feature = "_sm"))]
    {
        Vec::new()
    }
}

pub fn shutdown() {
    #[cfg(feature = "_sm")]
    {
        // Order matters — context, then handle, then `JS_ShutDown()` — and it is now stated once, in
        // `JsThread::drop`, instead of being re-derived by every caller. This just runs it early.
        spidermonkey::shutdown_engine();
    }
}

#[cfg(feature = "_sm")]
pub fn run_document_scripts(
    dom: &mut manuk_dom::Dom,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<usize, JsError> {
    with_runtime(|rt| {
        dom_bindings::run_scripts(rt, dom, layout, styles).map_err(|message| JsError { message })
    })
}

/// `window.open(...)` requests since the last call, each `(win_id, url)` — the host opens each as
/// a new tab/window (recording `win_id → tab` for `postMessage` routing). Empty without the JS
/// feature.
/// Does this page have a `scroll` listener or any observer? If not, the whole view-notification
/// path can be skipped — which for most pages it can.
#[cfg(feature = "_sm")]
pub fn wants_view_events(ctx: &PageContext) -> bool {
    with_runtime(|rt| Ok(ctx.wants_view_events(rt))).unwrap_or(false)
}

#[cfg(not(feature = "_sm"))]
pub fn wants_view_events(_ctx: &PageContext) -> bool {
    false
}

/// Tell the page its view changed (scroll / relayout): run the observers, fire `scroll`.
#[cfg(feature = "_sm")]
#[allow(clippy::too_many_arguments)]
pub fn view_changed(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    scroll_y: f32,
    vw: f32,
    vh: f32,
    scrolled: bool,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    with_runtime(|rt| {
        ctx.view_changed(rt, dom, scroll_y, vw, vh, scrolled, layout, styles)
            .map_err(|message| JsError { message })
    })
}

/// Publish the viewport's scroll offset + the focused element into the JS world before a re-entry.
#[cfg(feature = "_sm")]
pub fn set_view_state(scroll_x: f32, scroll_y: f32, active: Option<manuk_dom::NodeId>) {
    dom_bindings::set_view_state(scroll_x, scroll_y, active);
}

#[cfg(not(feature = "_sm"))]
pub fn set_view_state(_scroll_x: f32, _scroll_y: f32, _active: Option<manuk_dom::NodeId>) {}

/// Publish per-element scroll geometry for the coming script round:
/// `[scrollTop, scrollLeft, scrollHeight, scrollWidth, clientHeight, clientWidth]`.
///
/// The host owns the layout tree, so the host owns these numbers. They are *copied* rather than
/// borrowed because a script mutates them mid-round — `el.scrollTop = x` must be readable on the very
/// next line, and a virtualised list does exactly that.
#[cfg(feature = "_sm")]
pub fn set_scroll_geometry(g: std::collections::HashMap<manuk_dom::NodeId, [f32; 6]>) {
    dom_bindings::set_scroll_geometry(g);
}

#[cfg(not(feature = "_sm"))]
pub fn set_scroll_geometry(_g: std::collections::HashMap<manuk_dom::NodeId, [f32; 6]>) {}

/// Publish the live child documents — the arena behind each `<iframe>`'s `contentDocument`.
///
/// The map is `iframe element → (arena address, that document's root node)`. The arenas belong to child
/// `Page`s the parent owns and keeps boxed; the addresses are only meaningful while those `Page`s live,
/// which is what [`unregister_dom`] enforces on the way out.
#[cfg(feature = "_sm")]
pub fn set_iframe_docs(
    m: std::collections::HashMap<manuk_dom::NodeId, (usize, manuk_dom::NodeId)>,
) {
    dom_bindings::set_iframe_docs(m);
}

#[cfg(not(feature = "_sm"))]
pub fn set_iframe_docs(
    _m: std::collections::HashMap<manuk_dom::NodeId, (usize, manuk_dom::NodeId)>,
) {
}

/// An arena is legal to resolve reflectors against.
#[cfg(feature = "_sm")]
pub fn register_dom(dom: *mut manuk_dom::Dom) {
    dom_bindings::register_dom(dom);
}

#[cfg(not(feature = "_sm"))]
pub fn register_dom(_dom: *mut manuk_dom::Dom) {}

/// An arena is going away. **This must run before the `Page` drops**, or a reflector a script still
/// holds becomes a use-after-free rather than a `null`. `is_alive()` cannot save you here: it validates a
/// node id *within* an arena, and the arena itself is the thing that stopped existing.
#[cfg(feature = "_sm")]
pub fn unregister_dom(dom: *mut manuk_dom::Dom) {
    dom_bindings::unregister_dom(dom);
}

#[cfg(not(feature = "_sm"))]
pub fn unregister_dom(_dom: *mut manuk_dom::Dom) {}

/// `element.scrollTop = n` assignments a script made, already clamped. The host applies them, because
/// the host owns the layout tree.
#[cfg(feature = "_sm")]
pub fn take_element_scrolls() -> Vec<(manuk_dom::NodeId, f32, f32)> {
    dom_bindings::take_element_scrolls()
}

#[cfg(not(feature = "_sm"))]
pub fn take_element_scrolls() -> Vec<(manuk_dom::NodeId, f32, f32)> {
    Vec::new()
}

/// Scroll requests the page made (`scrollTo`, `scrollBy`, `scrollIntoView`) — the host performs
/// them, because the host owns the viewport.
#[cfg(feature = "_sm")]
pub fn take_scrolls() -> Vec<(f32, f32)> {
    dom_bindings::take_scrolls()
}

#[cfg(not(feature = "_sm"))]
pub fn take_scrolls() -> Vec<(f32, f32)> {
    Vec::new()
}

/// Focus requests the page made (`el.focus()`, `el.blur()`).
#[cfg(feature = "_sm")]
pub fn take_focus_requests() -> Vec<Option<manuk_dom::NodeId>> {
    dom_bindings::take_focus_requests()
}

#[cfg(not(feature = "_sm"))]
pub fn take_focus_requests() -> Vec<Option<manuk_dom::NodeId>> {
    Vec::new()
}

#[cfg(feature = "_sm")]
pub fn take_window_opens() -> Vec<(u64, String)> {
    dom_bindings::take_pending_window_opens()
}

#[cfg(not(feature = "_sm"))]
pub fn take_window_opens() -> Vec<(u64, String)> {
    Vec::new()
}

/// `navigator.clipboard.writeText(...)` calls the page made since the last drain (oldest first). The
/// host writes each to the OS clipboard; the last wins. Empty without the JS feature.
#[cfg(feature = "_sm")]
pub fn take_clipboard_writes() -> Vec<String> {
    dom_bindings::take_pending_clipboard_writes()
}

#[cfg(not(feature = "_sm"))]
pub fn take_clipboard_writes() -> Vec<String> {
    Vec::new()
}

/// Allocate the next process-unique window id (for ordinary, non-`window.open` tabs), shared
/// with the id space `window.open` draws from. `0` without the JS feature (unused there).
#[cfg(feature = "_sm")]
pub fn next_window_id() -> u64 {
    dom_bindings::next_window_id()
}

#[cfg(not(feature = "_sm"))]
pub fn next_window_id() -> u64 {
    0
}

/// Requests `ctx`'s page issued via `fetch`/`XMLHttpRequest` since the last call, each
/// `(id, url, method, headers, body)`. The host performs the round-trip (replaying `headers` onto
/// the wire) and settles it via [`resolve_fetch`]. Empty without the JS feature.
#[cfg(feature = "_sm")]
pub fn take_fetches(
    ctx: &PageContext,
) -> Vec<(u32, String, String, Vec<(String, String)>, String)> {
    with_runtime(|rt| ctx.take_fetches(rt).map_err(|message| JsError { message }))
        .unwrap_or_default()
}

#[cfg(not(feature = "_sm"))]
pub fn take_fetches(
    _ctx: &PageContext,
) -> Vec<(u32, String, String, Vec<(String, String)>, String)> {
    Vec::new()
}

/// Node ids of forms a script asked to submit. `direct` = `form.submit()` (no event, the script has
/// decided); `requested` = `form.requestSubmit()` (fire `submit` first, the page may cancel).
#[cfg(feature = "_sm")]
pub fn take_form_submits(ctx: &PageContext) -> (Vec<usize>, Vec<usize>) {
    with_runtime(|rt| {
        Ok::<_, JsError>((
            ctx.take_form_queue(rt, "__formSubmits"),
            ctx.take_form_queue(rt, "__formRequests"),
        ))
    })
    .unwrap_or_default()
}

#[cfg(not(feature = "_sm"))]
pub fn take_form_submits(_ctx: &PageContext) -> (Vec<usize>, Vec<usize>) {
    (Vec::new(), Vec::new())
}

/// Settle the `fetch`/`XHR` request `id` in `ctx`'s document with an HTTP `status` and response
/// `body` (`status == 0` = network failure). Runs the page's `.then`/`onload` reactions and any
/// DOM mutations they make. No-op without the JS feature.
#[cfg(feature = "_sm")]
pub fn resolve_fetch(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    id: u32,
    status: u16,
    body: &str,
    headers: &[(String, String)],
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    with_runtime(|rt| {
        ctx.resolve_fetch(rt, dom, id, status, body, headers, layout, styles)
            .map_err(|message| JsError { message })
    })
}

#[cfg(not(feature = "_sm"))]
pub fn resolve_fetch(
    _ctx: &PageContext,
    _dom: &mut manuk_dom::Dom,
    _id: u32,
    _status: u16,
    _body: &str,
    _headers: &[(String, String)],
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    Ok(())
}

/// `history` ops (`pushState`/`replaceState`/`back`/`forward`/`go`) the page performed since the
/// last call, each `(kind, state_json, url)`. The host reflects them in the omnibox +
/// back/forward stack without a network navigation. Empty without the JS feature.
#[cfg(feature = "_sm")]
pub fn take_history_ops(ctx: &PageContext) -> Vec<(u8, String, String)> {
    ctx.take_history_ops()
}

#[cfg(not(feature = "_sm"))]
pub fn take_history_ops(_ctx: &PageContext) -> Vec<(u8, String, String)> {
    Vec::new()
}

/// Fire a `popstate` event into `ctx`'s document (a real back/forward to a same-document
/// history entry), updating `history.state` + `location` and running the page's reactions.
/// No-op without the JS feature.
#[cfg(feature = "_sm")]
pub fn fire_popstate(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    state_json: &str,
    url: &str,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    with_runtime(|rt| {
        ctx.fire_popstate(rt, dom, state_json, url, layout, styles)
            .map_err(|message| JsError { message })
    })
}

#[cfg(not(feature = "_sm"))]
pub fn fire_popstate(
    _ctx: &PageContext,
    _dom: &mut manuk_dom::Dom,
    _state_json: &str,
    _url: &str,
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    Ok(())
}

/// Cross-window `postMessage` sends `ctx`'s page made since the last call, each `(target_win,
/// json, origin, source_win)`. The host routes each to the target window's [`deliver_message`].
/// Empty without the JS feature.
#[cfg(feature = "_sm")]
pub fn take_messages(ctx: &PageContext) -> Vec<(u64, String, String, u64)> {
    ctx.take_messages()
}

#[cfg(not(feature = "_sm"))]
pub fn take_messages(_ctx: &PageContext) -> Vec<(u64, String, String, u64)> {
    Vec::new()
}

/// Seed `ctx`'s window identity (own id + opener id) after load, so posted messages carry the
/// right `source` and `window.opener` resolves. No-op without the JS feature.
#[cfg(feature = "_sm")]
pub fn set_identity(ctx: &PageContext, win_id: u64, opener_win: u64) -> Result<(), JsError> {
    with_runtime(|rt| {
        ctx.set_identity(rt, win_id, opener_win)
            .map_err(|message| JsError { message })
    })
}

#[cfg(not(feature = "_sm"))]
pub fn set_identity(_ctx: &PageContext, _win_id: u64, _opener_win: u64) -> Result<(), JsError> {
    Ok(())
}

/// Deliver a cross-window message into `ctx`'s document: fire a `message` MessageEvent
/// (`{data, origin, source}`) and run the handler. No-op without the JS feature.
#[cfg(feature = "_sm")]
pub fn deliver_message(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    json: &str,
    origin: &str,
    source_win: u64,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    with_runtime(|rt| {
        ctx.deliver_message(rt, dom, json, origin, source_win, layout, styles)
            .map_err(|message| JsError { message })
    })
}

#[cfg(not(feature = "_sm"))]
pub fn deliver_message(
    _ctx: &PageContext,
    _dom: &mut manuk_dom::Dom,
    _json: &str,
    _origin: &str,
    _source_win: u64,
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    Ok(())
}

/// The interactive JS surface. `load_document` runs the page's scripts on a **persistent**
/// global and returns a [`PageContext`] to keep alive; `dispatch_event` later fires a trusted
/// event (a real click/input) into that same global so the page's registered listeners run.
/// This is what turns a one-shot script run into an interactive page.
#[cfg(feature = "_sm")]
pub use dom_bindings::PageContext;

/// Load `dom`'s scripts on a persistent global and return the context to retain for the
/// document's lifetime, plus the number of scripts that ran.
#[cfg(feature = "_sm")]
pub fn load_document(
    dom: &mut manuk_dom::Dom,
    url: &str,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(PageContext, usize), JsError> {
    with_runtime(|rt| {
        dom_bindings::PageContext::load(rt, dom, url, layout, styles)
            .map_err(|message| JsError { message })
    })
}

/// Run the scripts that do NOT block first paint — `defer`, `async`, and `type="module"` (deferred by
/// default in every real browser, and what every Vite bundle ships as).
///
/// The shell calls this *after* the document is on screen. `Page::load` calls it immediately, so every
/// gate sees the behaviour it has always seen.
#[cfg(feature = "_sm")]
pub fn run_deferred_scripts(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<usize, JsError> {
    with_runtime(|rt| {
        ctx.run_deferred_scripts(rt, dom, layout, styles)
            .map_err(|message| JsError { message })
    })
}

/// Dispatch a trusted `ty` event to `node` in `ctx`'s document. Returns `true` if the engine
/// should still perform the element's default action (no listener called `preventDefault`).
/// Evaluate a dynamically fetched script in an existing page context.
#[cfg(feature = "_sm")]
pub fn eval_in_page(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    src: &str,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    with_runtime(|rt| {
        ctx.eval(rt, dom, src, layout, styles)
            .map_err(|message| JsError { message })
    })
}

#[cfg(feature = "_sm")]
pub fn dispatch_event(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    node: manuk_dom::NodeId,
    ty: &str,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    with_runtime(|rt| {
        ctx.dispatch(rt, dom, node, ty, layout, styles)
            .map_err(|message| JsError { message })
    })
}

/// JS-less build: an opaque zero-sized context so callers compile unchanged. `dispatch_event`
/// always returns `true` (perform the default action), and `load_document` is a no-op.
#[cfg(not(feature = "_sm"))]
#[derive(Default)]
pub struct PageContext;

#[cfg(not(feature = "_sm"))]
pub fn load_document(
    _dom: &mut manuk_dom::Dom,
    _url: &str,
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(PageContext, usize), JsError> {
    Ok((PageContext, 0))
}

#[cfg(not(feature = "_sm"))]
pub fn dispatch_event(
    _ctx: &PageContext,
    _dom: &mut manuk_dom::Dom,
    _node: manuk_dom::NodeId,
    _ty: &str,
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    Ok(true)
}

/// Dispatch a `keydown`/`keyup` keyboard event carrying `key` + `key_code` to `node`. Returns
/// `false` iff a handler called `preventDefault()`. See [`PageContext::dispatch_key`].
#[cfg(feature = "_sm")]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_key(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    node: manuk_dom::NodeId,
    ty: &str,
    key: &str,
    key_code: u32,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    with_runtime(|rt| {
        ctx.dispatch_key(rt, dom, node, ty, key, key_code, layout, styles)
            .map_err(|message| JsError { message })
    })
}

#[cfg(not(feature = "_sm"))]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_key(
    _ctx: &PageContext,
    _dom: &mut manuk_dom::Dom,
    _node: manuk_dom::NodeId,
    _ty: &str,
    _key: &str,
    _key_code: u32,
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    Ok(true)
}

/// The JS-less build: `<script>`s are parsed into the DOM but not executed.
#[cfg(not(feature = "_sm"))]
pub fn run_document_scripts(
    _dom: &mut manuk_dom::Dom,
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<usize, JsError> {
    Ok(0)
}

#[cfg(feature = "_sm")]
pub mod spidermonkey;

/// D3 Step-0 prototype: an `Element.textContent`-style binding over the arena DOM.
#[cfg(feature = "_sm")]
pub mod bindings_prototype;

/// D3 hand-written DOM binding subset (jQuery-core methods) over the arena DOM.
/// N9 — a custom promise job queue, so native `Promise` reactions run through our
/// event loop (the embedder `JS::JobQueue` hook; no SpiderMonkey internals).
#[cfg(feature = "_sm")]
pub mod job_queue;

// NOTE the cfg on EACH. Inserting `pub mod canvas;` on the line above `pub mod dom_bindings;` silently
// STOLE its `#[cfg(feature = "_sm")]` — an attribute applies to the item that follows it, and the item
// that followed it was now canvas. `dom_bindings` then tried to compile with no JS engine underneath:
// 283 errors, and only in the no-feature build, which the wall runs and I do not.
#[cfg(feature = "_sm")]
pub mod attrs_js;
#[cfg(feature = "_sm")]
pub mod canvas;
#[cfg(feature = "_sm")]
pub mod collections_js;
#[cfg(feature = "_sm")]
pub mod dom_bindings;
#[cfg(feature = "_sm")]
pub mod iframe_js;
#[cfg(feature = "_sm")]
pub mod inline_handlers_js;
#[cfg(feature = "_sm")]
pub mod mutation_js;
#[cfg(feature = "_sm")]
pub mod range_js;
#[cfg(feature = "_sm")]
pub mod reflect_js;
#[cfg(feature = "_sm")]
pub mod reflect_table;
#[cfg(feature = "_sm")]
pub mod traversal_js;

/// N2 (host half) — History API state model; no JS engine dependency, always built.
pub mod history_host;

/// N2 — the History API JS bindings (pushState/replaceState/popstate/hashchange).
#[cfg(feature = "_sm")]
pub mod history_bindings;

/// D3 events tranche: the HTML event loop (microtasks + macrotasks/`setTimeout`).
#[cfg(feature = "_sm")]
pub mod event_loop;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noscript_is_a_noop() {
        let mut rt = NoScriptRuntime::new();
        assert_eq!(rt.eval("1 + 1", "inline").unwrap(), JsValue::Undefined);
        assert_eq!(rt.ignored_count(), 1);
    }

    #[test]
    fn factory_returns_something_named() {
        let rt = new_runtime();
        assert!(!rt.engine_name().is_empty());
    }
}
