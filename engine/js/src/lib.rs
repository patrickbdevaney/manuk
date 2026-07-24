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

/// **The mirror of [`canvas_bitmaps`]** — hand a decoded image *in*, so `ctx.drawImage(img, …)` has
/// something to draw. Keyed by the `<img>` element's `NodeId`, non-premultiplied RGBA8.
///
/// Idempotent, so the host can call it for every loaded image on every script round without thinking
/// about which ones it has already sent; re-publishing the same node at the same size is dropped inside.
/// A no-op without the JS feature, which is correct: no scripts, nothing to draw with.
#[allow(unused_variables)]
pub fn publish_image_source(node: u64, width: u32, height: u32, rgba: &[u8]) {
    #[cfg(feature = "_sm")]
    {
        canvas::publish_source(node, width, height, rgba);
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

/// Publish per-container scroll-snap candidates for the coming script round: `(xs, ys)` content-space
/// snap offsets, already clamped to the scrollable range, one list per axis (empty when the container
/// does not snap that axis).
///
/// The host owns the layout tree, so the host computes these; the JS `scrollLeft`/`scrollTop` setters
/// consume them to snap the mirror value at assignment time — measured Chrome lands the snap
/// SYNCHRONOUSLY (`el.scrollLeft = 130; el.scrollLeft` reads `100` on the same line). Recomputing snap
/// points inside the bindings (which hold no layout tree) would be the two-sources-of-truth trap.
#[cfg(feature = "_sm")]
pub fn set_snap_candidates(c: std::collections::HashMap<manuk_dom::NodeId, (Vec<f32>, Vec<f32>)>) {
    dom_bindings::set_snap_candidates(c);
}

#[cfg(not(feature = "_sm"))]
pub fn set_snap_candidates(_c: std::collections::HashMap<manuk_dom::NodeId, (Vec<f32>, Vec<f32>)>) {
}

/// A host callback that lays the document out synchronously — see [`set_reflow_hook`].
#[cfg(feature = "_sm")]
pub type ReflowFn = dom_bindings::ReflowFn;
#[cfg(not(feature = "_sm"))]
pub type ReflowFn = unsafe fn(ctx: *mut std::ffi::c_void, dom: *mut manuk_dom::Dom);

/// Install the **forced synchronous reflow** callback for the coming script round.
///
/// Geometry reads (`getBoundingClientRect`, `getClientRects`, `offsetWidth`/`offsetHeight`,
/// `offsetLeft`/`offsetTop`, `scrollIntoView`, used-value `getComputedStyle`) answer from a layout
/// snapshot taken before the script ran. When the script has since mutated the DOM that snapshot is
/// a lie, and this callback is how the read gets the truth: the binding calls up into the host, the
/// host re-cascades and re-lays-out, re-publishes the maps, and the read proceeds against fresh
/// geometry.
///
/// The host decides whether a reflow is actually needed — it knows which DOM state its current
/// layout was computed against (see `manuk_dom::Dom::mutation_seq`) — so this may be called on
/// every read.
///
/// # Safety
/// `ctx` must outlive the script round, must not alias any live reference held across it, and must
/// be torn down with [`clear_reflow_hook`].
#[cfg(feature = "_sm")]
pub unsafe fn set_reflow_hook(f: ReflowFn, ctx: *mut std::ffi::c_void) {
    unsafe { dom_bindings::set_reflow_hook(f, ctx) }
}

#[cfg(not(feature = "_sm"))]
pub unsafe fn set_reflow_hook(_f: ReflowFn, _ctx: *mut std::ffi::c_void) {}

/// A host callback answering "does this engine actually honour this CSS condition?".
pub type SupportsFn = fn(condition: &str) -> bool;

/// Install the CSS feature-query evaluator used by `CSS.supports()`.
///
/// The host passes its real one — Stylo, the same evaluator the cascade consults for `@supports` —
/// so the JS and CSS halves of one question cannot answer it differently.
#[cfg(feature = "_sm")]
pub fn set_supports_hook(f: SupportsFn) {
    dom_bindings::set_supports_hook(f)
}

#[cfg(not(feature = "_sm"))]
pub fn set_supports_hook(_f: SupportsFn) {}

/// A host callback answering "may an inline `<script>` with this nonce run under the document's
/// Content-Security-Policy?".
pub type CspInlineFn = Box<dyn Fn(manuk_dom::NodeId, Option<&str>) -> bool>;

/// Install (or clear, with `None`) the document's inline-script CSP check.
///
/// The evaluator lives in `manuk-net` alongside the rest of CSP; this crate holds no copy of the
/// matching rules, for the same reason it holds no CSS parser. Callers **must** call this on every
/// page construction — passing `None` when the document sent no policy — because the hook is a
/// thread-local and a policy left over from the previous navigation would be enforced against a
/// document that never sent one.
#[cfg(feature = "_sm")]
pub fn set_csp_inline_hook(f: Option<CspInlineFn>) {
    dom_bindings::set_csp_inline_hook(f)
}

#[cfg(not(feature = "_sm"))]
pub fn set_csp_inline_hook(_f: Option<CspInlineFn>) {}

/// Remove the forced-reflow callback. Paired with [`set_reflow_hook`] on every path out, including
/// the early returns — a stale `ctx` pointer outliving its owner is a use-after-free.
#[cfg(feature = "_sm")]
pub fn clear_reflow_hook() {
    dom_bindings::clear_reflow_hook();
}

#[cfg(not(feature = "_sm"))]
pub fn clear_reflow_hook() {}

/// The view-map pointers currently published to JS, so a caller that is about to replace them can
/// put back exactly what it found. See [`restore_view_maps`].
#[cfg(feature = "_sm")]
pub type ViewMaps = dom_bindings::ViewMaps;
#[cfg(not(feature = "_sm"))]
#[derive(Clone, Copy)]
pub struct ViewMaps;

#[cfg(feature = "_sm")]
pub fn view_maps() -> ViewMaps {
    dom_bindings::view_maps()
}

#[cfg(not(feature = "_sm"))]
pub fn view_maps() -> ViewMaps {
    ViewMaps
}

/// Restore view-map pointers taken by [`view_maps`].
///
/// A forced reflow publishes maps it owns; when those die, the bindings must be pointed back at
/// live ones. Without this the pointers dangle past the script round — a use-after-free that reads
/// as *the next page measuring garbage*, not as a crash at the site of the bug.
///
/// # Safety
/// The maps `v` refers to must still be alive (or `v` must carry the nulls from before anything
/// was published).
#[cfg(feature = "_sm")]
pub unsafe fn restore_view_maps(v: ViewMaps) {
    unsafe { dom_bindings::restore_view_maps(v) }
}

#[cfg(not(feature = "_sm"))]
pub unsafe fn restore_view_maps(_v: ViewMaps) {}

/// Re-point the layout/style maps at freshly laid-out ones, from inside a [`set_reflow_hook`]
/// callback.
///
/// # Safety
/// The maps must outlive the rest of the script round.
#[cfg(feature = "_sm")]
pub unsafe fn republish_view_maps(
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) {
    unsafe { dom_bindings::republish_view_maps(layout, styles) }
}

#[cfg(not(feature = "_sm"))]
pub unsafe fn republish_view_maps(
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) {
}

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

/// MSE byte-streams the page published since the last drain, `(node_id, full stream)`, oldest
/// first — one entry per settled `appendBuffer` that demuxed a video track. The host keeps the
/// last per node and decodes it exactly like a fetched progressive movie. Empty without the JS
/// feature (an MSE stream only ever exists inside a scripted page).
#[cfg(feature = "_sm")]
pub fn take_mse_streams() -> Vec<(u64, Vec<u8>)> {
    dom_bindings::take_pending_mse_streams()
}

#[cfg(not(feature = "_sm"))]
pub fn take_mse_streams() -> Vec<(u64, Vec<u8>)> {
    Vec::new()
}

/// Live media-IDL property writes since the last drain, `(node_id, prop, value)`, oldest first —
/// "muted" (0/1), "volume" (0..1), "playbackRate". The mute-button/volume-slider channel.
#[cfg(feature = "_sm")]
pub fn take_media_props() -> Vec<(u64, String, f64)> {
    dom_bindings::take_pending_media_props()
}

#[cfg(not(feature = "_sm"))]
pub fn take_media_props() -> Vec<(u64, String, f64)> {
    Vec::new()
}

/// Seed the OS-clipboard text the page may READ via `navigator.clipboard.readText()`/`read()`. The
/// host sets this to the real OS-clipboard contents (including text copied in another app) so paste
/// works. No-op without the JS feature.
#[cfg(feature = "_sm")]
pub fn set_host_clipboard(text: String) {
    dom_bindings::set_host_clipboard(text);
}

#[cfg(not(feature = "_sm"))]
pub fn set_host_clipboard(_text: String) {}

/// Seed the OS-clipboard IMAGE the page may READ via `navigator.clipboard.read()`, as a MIME type and
/// its raw bytes (e.g. `("image/png", png_bytes)`). The host sets this to whatever image is on the
/// real OS clipboard so a paste-a-screenshot handler receives it; an empty MIME/bytes clears it.
/// No-op without the JS feature.
#[cfg(feature = "_sm")]
pub fn set_host_clipboard_image(mime: String, bytes: Vec<u8>) {
    dom_bindings::set_host_clipboard_image(mime, bytes);
}

#[cfg(not(feature = "_sm"))]
pub fn set_host_clipboard_image(_mime: String, _bytes: Vec<u8>) {}

/// Drain the image parts a page passed to `navigator.clipboard.write()` since the last call, each
/// `(mime, bytes)`. The host puts them on the real OS clipboard — the WRITE half of the binary
/// bridge (copy-image-to-clipboard). Empty without the JS feature.
#[cfg(feature = "_sm")]
pub fn take_pending_clipboard_image_writes() -> Vec<(String, Vec<u8>)> {
    dom_bindings::take_pending_clipboard_image_writes()
}

#[cfg(not(feature = "_sm"))]
pub fn take_pending_clipboard_image_writes() -> Vec<(String, Vec<u8>)> {
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

/// As [`resolve_fetch`], but also carrying the response's raw bytes, so `arrayBuffer()` and an
/// `arraybuffer` XHR see the bytes the server sent rather than a UTF-8 re-encoding of the decoded
/// text. See `event_loop::deliver_bytes` for why both channels exist.
#[cfg(feature = "_sm")]
#[allow(clippy::too_many_arguments)]
pub fn resolve_fetch_bytes(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    id: u32,
    status: u16,
    body: &str,
    raw: &[u8],
    headers: &[(String, String)],
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    with_runtime(|rt| {
        ctx.resolve_fetch_bytes(
            rt,
            dom,
            id,
            status,
            body,
            Some(raw),
            headers,
            layout,
            styles,
        )
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

#[cfg(not(feature = "_sm"))]
#[allow(clippy::too_many_arguments)]
pub fn resolve_fetch_bytes(
    _ctx: &PageContext,
    _dom: &mut manuk_dom::Dom,
    _id: u32,
    _status: u16,
    _body: &str,
    _raw: &[u8],
    _headers: &[(String, String)],
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    Ok(())
}

/// One step of a **streaming** response, in the order the wire produces them: `Head` once (which is
/// where the page's `fetch()` promise resolves), then a `Chunk` per piece of body as it arrives, then
/// `End` once.
///
/// This is the shape the buffered [`resolve_fetch`] cannot express: it settles a request with its
/// whole body at once, so a page that streams sees a single lump at the end. The difference is an AI
/// answer that appears only when the server has finished versus one that types itself out.
#[derive(Debug, Clone)]
pub enum FetchStreamEvent {
    /// Response headers — the promise resolves here, with the body still arriving.
    Head {
        status: u16,
        headers: Vec<(String, String)>,
    },
    /// Raw body bytes. A chunk boundary may split a multi-byte UTF-8 sequence; the page's own
    /// `TextDecoder` reassembles, so these stay bytes and are never lossily decoded en route.
    Chunk(Vec<u8>),
    /// The body is complete — the page's pump loop sees `{done: true}`.
    End,
}

/// Deliver one [`FetchStreamEvent`] for request `id`, running the page's reactions (and any DOM
/// mutations they make) before returning — which is what lets the page re-render BETWEEN chunks.
/// No-op without the JS feature.
#[cfg(feature = "_sm")]
pub fn deliver_fetch_stream(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    id: u32,
    event: &FetchStreamEvent,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    with_runtime(|rt| {
        ctx.deliver_fetch_stream(rt, dom, id, event, layout, styles)
            .map_err(|message| JsError { message })
    })
}

#[cfg(not(feature = "_sm"))]
pub fn deliver_fetch_stream(
    _ctx: &PageContext,
    _dom: &mut manuk_dom::Dom,
    _id: u32,
    _event: &FetchStreamEvent,
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    Ok(())
}

/// One thing a page asked its WebSocket to do. The host owns the socket; the page queues.
///
/// `Connect` carries the offered subprotocols, `Send` the frame payload as raw bytes, `Close` the
/// code and reason the page passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsOp {
    Connect { url: String, protocols: Vec<String> },
    Send { data: Vec<u8>, binary: bool },
    Close { code: Option<u16>, reason: String },
}

/// Something that happened to a page's WebSocket, in the order the transport produces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsEvent {
    /// The handshake completed. `protocol` is what the SERVER chose (`""` if none).
    Open {
        protocol: String,
        extensions: String,
    },
    Message {
        data: Vec<u8>,
        binary: bool,
    },
    /// A queued frame reached the wire — decrements the page's `bufferedAmount`.
    Sent {
        bytes: usize,
    },
    /// The spec's `error` event carries no detail to the page (it would be a cross-origin info
    /// leak); `message` is for our logs.
    Error {
        message: String,
    },
    Close {
        code: u16,
        reason: String,
        clean: bool,
    },
}

/// What the page's WebSockets asked for since the last call, each `(socket_id, op)`. The host owns
/// the socket: the page queues a connect/send/close and the host performs it. Empty without JS.
#[cfg(feature = "_sm")]
pub fn take_ws_ops(ctx: &PageContext) -> Vec<(u32, WsOp)> {
    with_runtime(|rt| ctx.take_ws_ops(rt).map_err(|message| JsError { message }))
        .unwrap_or_default()
}

#[cfg(not(feature = "_sm"))]
pub fn take_ws_ops(_ctx: &PageContext) -> Vec<(u32, WsOp)> {
    Vec::new()
}

/// Deliver one [`WsEvent`] to socket `id`, running the page's handlers (and any DOM mutations they
/// make) before returning. No-op without the JS feature.
#[cfg(feature = "_sm")]
pub fn deliver_ws_event(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    id: u32,
    event: &WsEvent,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    with_runtime(|rt| {
        ctx.deliver_ws_event(rt, dom, id, event, layout, styles)
            .map_err(|message| JsError { message })
    })
}

#[cfg(not(feature = "_sm"))]
pub fn deliver_ws_event(
    _ctx: &PageContext,
    _dom: &mut manuk_dom::Dom,
    _id: u32,
    _event: &WsEvent,
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    Ok(())
}

/// One cue currently on screen for one media element, in the spec's own vocabulary.
///
/// The mirror of the JS-side active-cue set into Rust. Everything about captions — the cues, their
/// timing, the `showing` mode, the clock — lives in JavaScript (`el.__textTracks`,
/// `el.__currentTime`), because that is where the `TextTrack` API is. The painter is in Rust and
/// never sees the DOM at all. So for the UA to draw a caption, the one thing that knows a cue is
/// active has to say so, and this is the wire.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveCue {
    pub text: String,
    /// `None` is `auto` — and `auto` is NOT `0`; see `manuk_media::vtt::CueSettings`.
    pub line: Option<f64>,
    pub line_is_percent: bool,
    pub position: Option<f64>,
    pub size: f64,
    pub align: String,
    pub vertical: String,
}

/// The WebVTT cues currently on screen, per media element node id — the input to the UA's own
/// caption overlay. A **snapshot, not a drain**: a caption stays up until it comes down, so the
/// host reads this on every paint without consuming it. Empty without the JS feature (a build with
/// no JS has no `TextTrack` to be showing anything).
#[cfg(feature = "_sm")]
pub fn active_cues() -> std::collections::HashMap<u64, Vec<ActiveCue>> {
    dom_bindings::active_cues()
}

#[cfg(not(feature = "_sm"))]
pub fn active_cues() -> std::collections::HashMap<u64, Vec<ActiveCue>> {
    std::collections::HashMap::new()
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

/// Seed the window identity the NEXT document loaded on this thread is born with.
///
/// Must be used instead of [`set_identity`] whenever a page's own load-time scripts read
/// `window.opener` — which is the entire popup-login family (Google Identity Services, Stripe
/// Checkout, Auth0 `loginWithPopup`). `set_identity` can only run after `load_document` has already
/// executed those scripts, so they see `null` and never post their result back.
#[cfg(feature = "_sm")]
pub fn set_pending_identity(win_id: u64, opener_win: u64) {
    dom_bindings::PageContext::set_pending_identity(win_id, opener_win);
}

#[cfg(not(feature = "_sm"))]
pub fn set_pending_identity(_win_id: u64, _opener_win: u64) {}

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

/// The four `KeyboardEvent` modifier flags (`ctrlKey`/`shiftKey`/`altKey`/`metaKey`) carried by a
/// dispatched key. Threaded through [`dispatch_key`] so page handlers can read them — a command
/// palette (`if (e.metaKey && e.key === 'k')`, the Cmd/Ctrl+K in Slack/Notion/Linear/GitHub), a
/// composer that inserts a newline only on `Shift+Enter` — and so the default editing action does
/// NOT insert a stray character when a shortcut chord (Ctrl+B, Cmd+K) is held. `Default` = no
/// modifiers, which is what every existing caller of the plain [`dispatch_key`] gets.
#[derive(Clone, Copy, Default, Debug)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
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
    mods: KeyModifiers,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    with_runtime(|rt| {
        ctx.dispatch_key(rt, dom, node, ty, key, key_code, mods, layout, styles)
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
    _mods: KeyModifiers,
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    Ok(true)
}

/// Dispatch `dragenter`/`dragover`/`drop` to `node` with a real `DataTransfer` carrying
/// `files_json`. Returns `false` iff a handler called `preventDefault()` on the `drop`.
/// See [`PageContext::dispatch_drop`].
#[cfg(feature = "_sm")]
pub fn dispatch_drop(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    node: manuk_dom::NodeId,
    files_json: &str,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    with_runtime(|rt| {
        ctx.dispatch_drop(rt, dom, node, files_json, layout, styles)
            .map_err(|message| JsError { message })
    })
}

#[cfg(not(feature = "_sm"))]
pub fn dispatch_drop(
    _ctx: &PageContext,
    _dom: &mut manuk_dom::Dom,
    _node: manuk_dom::NodeId,
    _files_json: &str,
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    Ok(true)
}

/// Dispatch a within-page drag from `source` to `target` — `dragstart`/`drag`-sequence/`drop`/
/// `dragend` sharing one `DataTransfer`, the source→target reorder handoff. Returns `false` iff a
/// handler `preventDefault()`-ed the `drop`. See [`PageContext::dispatch_drag`].
#[cfg(feature = "_sm")]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_drag(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    source: manuk_dom::NodeId,
    target: manuk_dom::NodeId,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    with_runtime(|rt| {
        ctx.dispatch_drag(rt, dom, source, target, layout, styles)
            .map_err(|message| JsError { message })
    })
}

#[cfg(not(feature = "_sm"))]
pub fn dispatch_drag(
    _ctx: &PageContext,
    _dom: &mut manuk_dom::Dom,
    _source: manuk_dom::NodeId,
    _target: manuk_dom::NodeId,
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    Ok(true)
}

/// Dispatch the IME composition-commit sequence (`compositionstart`/`compositionupdate`/
/// `beforeinput`/`input`/`compositionend`) that commits `data` into `node`. Returns `false` iff a
/// handler `preventDefault()`-ed the `beforeinput`. See [`PageContext::dispatch_composition`].
#[cfg(feature = "_sm")]
pub fn dispatch_composition(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    node: manuk_dom::NodeId,
    data: &str,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    with_runtime(|rt| {
        ctx.dispatch_composition(rt, dom, node, data, layout, styles)
            .map_err(|message| JsError { message })
    })
}

#[cfg(not(feature = "_sm"))]
pub fn dispatch_composition(
    _ctx: &PageContext,
    _dom: &mut manuk_dom::Dom,
    _node: manuk_dom::NodeId,
    _data: &str,
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    Ok(true)
}

/// Dispatch one mouse event carrying `detail` (the click count) and `button`. Returns `false` iff a
/// handler called `preventDefault()`. See [`PageContext::dispatch_mouse`].
#[cfg(feature = "_sm")]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_mouse(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    node: manuk_dom::NodeId,
    ty: &str,
    detail: u32,
    button: u32,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    with_runtime(|rt| {
        ctx.dispatch_mouse(rt, dom, node, ty, detail, button, layout, styles)
            .map_err(|message| JsError { message })
    })
}

#[cfg(not(feature = "_sm"))]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_mouse(
    _ctx: &PageContext,
    _dom: &mut manuk_dom::Dom,
    _node: manuk_dom::NodeId,
    _ty: &str,
    _detail: u32,
    _button: u32,
    _layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    _styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    Ok(true)
}

/// [`dispatch_mouse`] with an explicit `buttons` mask — `1` on `mousedown`, `0` on `mouseup`.
#[cfg(feature = "_sm")]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_mouse_buttons(
    ctx: &PageContext,
    dom: &mut manuk_dom::Dom,
    node: manuk_dom::NodeId,
    ty: &str,
    detail: u32,
    button: u32,
    buttons: u32,
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<bool, JsError> {
    with_runtime(|rt| {
        ctx.dispatch_mouse_buttons(rt, dom, node, ty, detail, button, buttons, layout, styles)
            .map_err(|message| JsError { message })
    })
}

#[cfg(not(feature = "_sm"))]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_mouse_buttons(
    _ctx: &PageContext,
    _dom: &mut manuk_dom::Dom,
    _node: manuk_dom::NodeId,
    _ty: &str,
    _detail: u32,
    _button: u32,
    _buttons: u32,
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
pub mod mse_js;
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
