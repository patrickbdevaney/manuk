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
