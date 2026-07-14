//! SpiderMonkey-backed [`JsRuntime`], compiled only under `--features spidermonkey`.
//!
//! This is the sanctioned integration surface: we create an engine + runtime, make
//! a global with a realm, and evaluate script via the high-level `mozjs::rust`
//! wrappers (`evaluate_script`, `CompileOptionsWrapper`). We do **not** touch
//! JIT/GC internals or the sandbox — see the crate docs and CLAUDE.md §
//! modification boundary. The shape here follows mozjs's own `examples/eval.rs`.

use std::ffi::CString;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
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
/// # Why the runtime and the engine share ONE thread-local
///
/// SpiderMonkey must be torn down in exactly one order — every `JSContext` first, then
/// `JS_ShutDown()` — and it must happen **before the process exits**, or its C++ static destructors
/// run against a live engine and segfault inside `__run_exit_handlers` (ADR-009; the visible symptom
/// is `pthread_mutex_destroy failed: Device or resource busy`, then SIGSEGV, *after* `main` returns,
/// with every byte of output already correct).
///
/// The previous design held the engine and the runtime in **two** thread-locals, both `ManuallyDrop`
/// so that neither could drop the other out from under it — because the drop order *across* separate
/// thread-locals is unspecified. That made teardown safe but never automatic: the only thing that
/// actually ran it was a caller remembering `manuk_js::shutdown()`. Half of them did. `g_globals`,
/// `js_conformance` and `g_dedup` did not, and every one of them crashed on the way out.
///
/// **A convention that half the callers forget is not a fix, it is a list of the places you have not
/// been bitten yet.** So both now live in one struct, in one thread-local, and teardown is a `Drop`
/// impl that runs them in the only correct order. The thread that owns SpiderMonkey ends SpiderMonkey
/// — whether or not anybody asks it to.
///
/// `JSEngine` is not `Send` (SpiderMonkey is thread-affine), so the engine lives on the thread that
/// created it. Its **handle** is `Send`, and is published in a process-global slot so that any thread
/// can build a `Runtime` from the single engine `JSEngine::init()` is permitted to create.
/// # The ordering trap, and why the fix is a separate guard
///
/// The obvious version of this — one struct holding `Runtime` and `JSEngine`, in one thread-local,
/// with a `Drop` that tears them down in order — **does not work**, and failing at it is what taught
/// us the actual rule:
///
/// > **Thread-local destructors run in REVERSE order of registration** (glibc pushes them onto a LIFO
/// > list as each is first initialized). And mozjs keeps thread-locals of its own — `Runtime::drop`
/// > reaches for its internal `CONTEXT`.
///
/// Our state has to be initialized *before* the runtime exists (the engine must be parked somewhere
/// the moment `JSEngine::init()` returns), so it registers **first**, so it is destroyed **last** —
/// by which time mozjs's own thread-local is already gone. Teardown then dies with
/// `cannot access a Thread Local Storage value during or after destruction`, inside a `nounwind`
/// frame, which is an instant abort. We swapped one exit crash for another.
///
/// So the state is split from the *trigger*:
///
/// * [`ENGINE`] and [`RUNTIME`] hold `ManuallyDrop`, which has **no drop glue** — so they register no
///   destructor at all, are never torn down by TLS, and stay readable at any point during shutdown.
/// * [`TEARDOWN`] is an empty guard whose only job is its `Drop`. It is first touched **after**
///   `Runtime::new()` — therefore registered *after* mozjs's thread-locals, therefore destroyed
///   *before* them. It runs while every thing it needs is still alive.
///
/// The lesson generalises past SpiderMonkey: **to run first at teardown, register last.**
struct TeardownGuard;

impl Drop for TeardownGuard {
    fn drop(&mut self) {
        teardown();
    }
}

/// The one true teardown, in the one correct order. Idempotent: calling it twice is a no-op, so an
/// explicit [`shutdown_engine`] and the automatic guard cannot fight each other.
fn teardown() {
    // 1. The context. A rooted JS object outliving its runtime is the *other* way to crash here, so
    //    nothing may touch JS after this point.
    RUNTIME.with(|cell| {
        if let Some(rt) = cell.borrow_mut().take() {
            drop(std::mem::ManuallyDrop::into_inner(rt));
        }
    });

    // 2. The published handle. A cached handle is an OUTSTANDING handle, and `JSEngine::drop` asserts
    //    "There are outstanding JS engine handles" if one survives it.
    if let Some(cell) = ENGINE_HANDLE.get() {
        if let Ok(mut slot) = cell.lock() {
            *slot = None;
        }
    }

    // 3. `JS_ShutDown()`. After this the engine cannot be re-initialized in this process — a
    //    SpiderMonkey rule, not ours — so record it and refuse later requests rather than crash.
    ENGINE.with(|cell| {
        if let Some(e) = cell.borrow_mut().take() {
            drop(std::mem::ManuallyDrop::into_inner(e));
            SHUT_DOWN.store(true, Ordering::SeqCst);
        }
    });
}

thread_local! {
    /// The engine. `ManuallyDrop` ⇒ no drop glue ⇒ **no TLS destructor is registered**, so this is
    /// still readable from inside [`TeardownGuard::drop`]. That is the entire point.
    static ENGINE: std::cell::RefCell<Option<std::mem::ManuallyDrop<JSEngine>>> =
        const { std::cell::RefCell::new(None) };

    /// The thread's `JSContext`. Same reasoning as [`ENGINE`].
    pub(crate) static RUNTIME: std::cell::RefCell<Option<std::mem::ManuallyDrop<Runtime>>> =
        const { std::cell::RefCell::new(None) };

    /// The trigger. Touched only *after* the runtime exists, so it is registered last and therefore
    /// destroyed first — before mozjs's own thread-locals go.
    static TEARDOWN: TeardownGuard = const { TeardownGuard };
}

/// Arm the automatic teardown. **Must be called after `Runtime::new()`**, never before: the whole
/// mechanism is the registration order, and calling this early silently reintroduces the crash.
pub(crate) fn arm_teardown() {
    TEARDOWN.with(|_| {});
}

/// Set once `JS_ShutDown()` has run. `JSEngine::init()` may not be called again afterwards, so a late
/// request for JS is answered with an honest error instead of a crash.
static SHUT_DOWN: AtomicBool = AtomicBool::new(false);

/// The published handle. A `Mutex<Option<_>>` rather than a `OnceLock` because it must be
/// *clearable*: a cached handle is an outstanding handle, and the engine refuses to shut down while
/// one exists.
static ENGINE_HANDLE: OnceLock<std::sync::Mutex<Option<JSEngineHandle>>> = OnceLock::new();

pub(crate) fn engine_handle() -> Result<JSEngineHandle, JsError> {
    if SHUT_DOWN.load(Ordering::SeqCst) {
        return Err(JsError {
            message: "SpiderMonkey has already been shut down in this process".to_string(),
        });
    }
    let cell = ENGINE_HANDLE.get_or_init(|| std::sync::Mutex::new(None));
    let mut slot = cell.lock().map_err(|_| JsError {
        message: "SpiderMonkey engine lock poisoned".to_string(),
    })?;
    if slot.is_none() {
        let engine = JSEngine::init().map_err(|_| JsError {
            message: "SpiderMonkey JSEngine::init() failed".to_string(),
        })?;
        *slot = Some(engine.handle());
        // Park the engine in a thread-local with NO drop glue, so TLS never tears it down behind our
        // back. `TeardownGuard` is the only thing that ever drops it, and it is armed later — after
        // `Runtime::new`, which is the whole trick.
        ENGINE.with(|c| *c.borrow_mut() = Some(std::mem::ManuallyDrop::new(engine)));
    }
    slot.as_ref().cloned().ok_or_else(|| JsError {
        message: "SpiderMonkey JSEngine::init() failed".to_string(),
    })
}

/// Tear SpiderMonkey down now, rather than waiting for the thread to end.
///
/// This is no longer *required* — [`JsThread`]'s `Drop` does it automatically — but it stays, because
/// a browser wants to choose the moment it stops running JavaScript (e.g. before it flushes the
/// profile), rather than inherit whatever moment the runtime picks. Calling it twice is harmless.
pub(crate) fn shutdown_engine() {
    teardown();
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
