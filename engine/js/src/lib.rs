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
fn with_runtime<R>(f: impl FnOnce(&mut mozjs::rust::Runtime) -> Result<R, JsError>) -> Result<R, JsError> {
    use std::cell::RefCell;
    use std::mem::ManuallyDrop;
    use mozjs::rust::Runtime;

    thread_local! {
        static RUNTIME: RefCell<Option<ManuallyDrop<Runtime>>> = const { RefCell::new(None) };
    }
    RUNTIME.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            let handle = spidermonkey::engine_handle()?;
            *slot = Some(ManuallyDrop::new(Runtime::new(handle)));
        }
        let rt: &mut Runtime = &mut *slot.as_mut().expect("runtime just initialized");
        f(rt)
    })
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

/// URLs the current page requested via `window.open(...)` since the last call — the host opens
/// each as a new tab/window (the OAuth-popup pattern). Empty without the JS feature.
#[cfg(feature = "_sm")]
pub fn take_window_opens() -> Vec<String> {
    dom_bindings::take_pending_window_opens()
}

#[cfg(not(feature = "_sm"))]
pub fn take_window_opens() -> Vec<String> {
    Vec::new()
}

/// Requests `ctx`'s page issued via `fetch`/`XMLHttpRequest` since the last call, each
/// `(id, url, method, body)`. The host performs the round-trip and settles it via
/// [`resolve_fetch`]. Empty without the JS feature.
#[cfg(feature = "_sm")]
pub fn take_fetches(ctx: &PageContext) -> Vec<(u32, String, String, String)> {
    with_runtime(|rt| ctx.take_fetches(rt).map_err(|message| JsError { message })).unwrap_or_default()
}

#[cfg(not(feature = "_sm"))]
pub fn take_fetches(_ctx: &PageContext) -> Vec<(u32, String, String, String)> {
    Vec::new()
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
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(), JsError> {
    with_runtime(|rt| {
        ctx.resolve_fetch(rt, dom, id, status, body, layout, styles)
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
    layout: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
) -> Result<(PageContext, usize), JsError> {
    with_runtime(|rt| {
        dom_bindings::PageContext::load(rt, dom, layout, styles).map_err(|message| JsError { message })
    })
}

/// Dispatch a trusted `ty` event to `node` in `ctx`'s document. Returns `true` if the engine
/// should still perform the element's default action (no listener called `preventDefault`).
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
        ctx.dispatch(rt, dom, node, ty, layout, styles).map_err(|message| JsError { message })
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

#[cfg(feature = "_sm")]
pub mod dom_bindings;

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
