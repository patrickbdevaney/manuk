//! SpiderMonkey-backed [`JsRuntime`], compiled only under `--features spidermonkey`.
//!
//! This is the sanctioned integration surface: we create an engine + runtime, make
//! a global with a realm, and evaluate script via the high-level `mozjs::rust`
//! wrappers (`evaluate_script`, `CompileOptionsWrapper`). We do **not** touch
//! JIT/GC internals or the sandbox — see the crate docs and CLAUDE.md §
//! modification boundary. The shape here follows mozjs's own `examples/eval.rs`.

use std::ffi::CString;
use std::ptr;
use std::sync::OnceLock;

use mozjs::jsapi::OnNewGlobalHookOption;
use mozjs::jsval::UndefinedValue;
use mozjs::rooted;
use mozjs::rust::wrappers2::*;
use mozjs::rust::SIMPLE_GLOBAL_CLASS;
use mozjs::rust::{
    evaluate_script, CompileOptionsWrapper, JSEngine, JSEngineHandle, RealmOptions, Runtime,
};

use crate::{JsError, JsRuntime, JsValue};

/// The process-global SpiderMonkey engine. `JSEngine::init()` may run only once per process, so it
/// is initialized exactly once and its handle shared across every runtime — the standard embedding
/// pattern.
///
/// The engine is **owned** here rather than leaked, because SpiderMonkey requires `JS_ShutDown()`
/// before the process exits. Leaking it (the obvious "keep it alive forever" move) means shutdown
/// never runs, and SpiderMonkey's C++ static destructors then execute against a still-initialized
/// engine and **segfault inside `__run_exit_handlers`** — after `main` has returned, so every byte
/// of output looks correct and only the exit code (139) betrays it. Worse, a crash there aborts the
/// remaining exit handlers, which is precisely where a browser flushes its cookie jar and
/// `localStorage` to the profile: the failure mode is silent data loss (ADR-009).
/// `JSEngine` is not `Send` — SpiderMonkey is thread-affine — so the engine itself lives on the one
/// thread that creates it (the UI thread). Its **handle** is `Send`, so it is published in a
/// process-global slot: that is how any thread obtains a `Runtime` from the single engine
/// `JSEngine::init()` is allowed to create.
///
/// The engine is held in `ManuallyDrop` so that thread-local teardown — whose order across
/// thread-locals is *unspecified* — can never drop it out from under a still-live `Runtime`
/// (`JSEngine::drop` asserts "There are outstanding JS engine handles" if it does). The only way it
/// is destroyed is [`shutdown_engine`], which runs after the runtime, in that order, on purpose.
thread_local! {
    static ENGINE: std::cell::RefCell<Option<std::mem::ManuallyDrop<JSEngine>>> =
        const { std::cell::RefCell::new(None) };
}

/// The published handle. A `Mutex<Option<_>>` rather than a `OnceLock` because it must be
/// *clearable*: a cached handle is an outstanding handle, and the engine refuses to shut down while
/// one exists.
static ENGINE_HANDLE: OnceLock<std::sync::Mutex<Option<JSEngineHandle>>> = OnceLock::new();

pub(crate) fn engine_handle() -> Result<JSEngineHandle, JsError> {
    let cell = ENGINE_HANDLE.get_or_init(|| std::sync::Mutex::new(None));
    let mut slot = cell.lock().map_err(|_| JsError {
        message: "SpiderMonkey engine lock poisoned".to_string(),
    })?;
    if slot.is_none() {
        let engine = JSEngine::init().map_err(|_| JsError {
            message: "SpiderMonkey JSEngine::init() failed".to_string(),
        })?;
        *slot = Some(engine.handle());
        ENGINE.with(|c| *c.borrow_mut() = Some(std::mem::ManuallyDrop::new(engine)));
    }
    slot.as_ref().cloned().ok_or_else(|| JsError {
        message: "SpiderMonkey JSEngine::init() failed".to_string(),
    })
}

/// Drop the engine, calling `JS_ShutDown()`. Every runtime (and therefore every outstanding
/// handle) must already be gone — `JSEngine::drop` asserts on that, and the assert is the point:
/// shutting down under a live runtime is the other way to crash here.
pub(crate) fn shutdown_engine() {
    // Release the cached handle first — it counts as outstanding.
    if let Some(cell) = ENGINE_HANDLE.get() {
        if let Ok(mut slot) = cell.lock() {
            *slot = None;
        }
    }
    // Then the engine itself, which is what calls `JS_ShutDown()`. Only the thread that created it
    // holds it; called from anywhere else this is a no-op and the engine simply leaks (the old
    // behaviour), which is safe but leaves the atexit crash in place — so call it from the JS
    // thread.
    ENGINE.with(|cell| {
        if let Some(e) = cell.borrow_mut().take() {
            drop(std::mem::ManuallyDrop::into_inner(e));
        }
    });
}

/// A SpiderMonkey runtime bound to the current thread.
///
/// SpiderMonkey is thread-affine, so this type is intentionally not `Send`. It
/// borrows the process-global engine ([`engine_handle`]); many runtimes (e.g. one
/// isolate per tab) share that single engine.
pub struct SpiderMonkeyRuntime {
    runtime: Runtime,
}

impl SpiderMonkeyRuntime {
    pub fn new() -> Result<Self, JsError> {
        let runtime = Runtime::new(engine_handle()?);
        Ok(SpiderMonkeyRuntime { runtime })
    }
}

impl JsRuntime for SpiderMonkeyRuntime {
    fn eval(&mut self, source: &str, filename: &str) -> Result<JsValue, JsError> {
        let options = RealmOptions::default();
        // Fresh global/realm per eval keeps this simple; a persistent global is
        // where the DOM bindings (crate::bindings) would live.
        rooted!(&in(self.runtime.cx()) let global = unsafe {
            JS_NewGlobalObject(
                self.runtime.cx(),
                &SIMPLE_GLOBAL_CLASS,
                ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook,
                &*options,
            )
        });

        rooted!(&in(self.runtime.cx()) let mut rval = UndefinedValue());

        let cfilename = CString::new(filename).unwrap_or_default();
        let compile_options = CompileOptionsWrapper::new(self.runtime.cx_no_gc(), cfilename, 1);
        let res = evaluate_script(
            self.runtime.cx(),
            global.handle(),
            source,
            rval.handle_mut(),
            compile_options,
        );

        match res {
            Ok(()) => Ok(convert(rval.get())),
            Err(()) => Err(JsError {
                message: format!("uncaught exception while evaluating {filename}"),
            }),
        }
    }

    fn engine_name(&self) -> &'static str {
        "SpiderMonkey (mozjs)"
    }
}

/// Convert a SpiderMonkey `Value` into our simplified [`JsValue`]. Strings and
/// objects are reported as typed placeholders (decoding them needs a rooted
/// conversion, a follow-on).
fn convert(v: mozjs::jsapi::Value) -> JsValue {
    if v.is_undefined() {
        JsValue::Undefined
    } else if v.is_null() {
        JsValue::Null
    } else if v.is_boolean() {
        JsValue::Bool(v.to_boolean())
    } else if v.is_int32() {
        JsValue::Number(v.to_int32() as f64)
    } else if v.is_double() {
        JsValue::Number(v.to_double())
    } else if v.is_string() {
        JsValue::Str("[string]".to_string())
    } else {
        JsValue::Str("[object]".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_real_javascript() {
        let mut rt = SpiderMonkeyRuntime::new().expect("boot SpiderMonkey");
        assert_eq!(rt.engine_name(), "SpiderMonkey (mozjs)");
        assert_eq!(rt.eval("40 + 2", "t.js").unwrap(), JsValue::Number(42.0));
        assert_eq!(rt.eval("2 > 5", "t.js").unwrap(), JsValue::Bool(false));
        assert_eq!(
            rt.eval("let a = 3; a * a", "t.js").unwrap(),
            JsValue::Number(9.0)
        );
    }
}
