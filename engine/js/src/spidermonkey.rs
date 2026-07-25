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
        let mut runtime = Runtime::new(engine_handle()?);
        install_host_hooks(&mut runtime);
        Ok(SpiderMonkeyRuntime { runtime })
    }
}

/// The host hooks SpiderMonkey **requires an embedder to install**, and aborts the process without.
///
/// ## `FinalizationRegistry` — a SEGFAULT on this seam, found by the first test262 run (tick 546)
///
/// `new FinalizationRegistry(() => {})` **core-dumped the process.** Not an exception, not a
/// missing-feature `TypeError` — a null dereference: the constructor asks the host for the
/// *incumbent global*, that question is routed through `JS::JobQueue`, and this runtime installed
/// no queue. `typeof FinalizationRegistry` was `"function"`, so every feature detector on the web
/// said yes.
///
/// **Scope, stated precisely rather than dramatically.** The PAGE path was already safe:
/// `event_loop::install` calls `job_queue::install_once` when it builds a document's global, so a
/// real tab has a queue. What did not was the bare [`SpiderMonkeyRuntime`] — the [`JsRuntime`] seam
/// that `manuk eval` and any other embedder of this crate uses, and the seam the conformance runner
/// runs on. So: a crash on a shipped surface, not a crash in the browser. Installing the queue here
/// makes the two paths agree, which is the real defect — *one of two constructors of the same engine
/// set up the host and the other did not*, and nothing said so.
///
/// It sat here for 500+ ticks because nothing had ever asked the engine this question. That is the
/// meta-instrument argument as a receipt rather than a principle: the corpus crawl cannot see it (no
/// corpus site constructs one), and the *first thing* a conformance suite did was walk into it.
///
/// ## The cleanup callback, and why a no-op is the honest answer
///
/// The `SetHostCleanupFinalizationRegistryCallback` half is belt-and-braces: with a queue installed
/// the constructor no longer crashes, and this names where cleanup jobs would go if we ran them.
/// **We do not run them, and that is spec-legal** — ECMAScript explicitly does not require an
/// implementation to ever call a cleanup callback (a host that never collects is conforming). So the
/// registry is real, registration and `unregister` work, and the callback never fires. Draining
/// `doCleanup` through the real job queue after a GC is a named follow-on: an improvement on a legal
/// baseline, not a fix for a lie.
fn install_host_hooks(runtime: &mut Runtime) {
    unsafe extern "C" fn drop_cleanup_job(
        _do_cleanup: *mut mozjs::jsapi::JSFunction,
        _incumbent_global: *mut mozjs::jsapi::JSObject,
        _data: *mut std::ffi::c_void,
    ) {
    }
    unsafe {
        // The JOB QUEUE is the load-bearing half. `new FinalizationRegistry(fn)` asks the host for
        // the *incumbent* global — through `JS::JobQueue` — and a context with no queue installed
        // dereferences a null one. It is the queue, not the cleanup callback, that turns the abort
        // into an ordinary object construction.
        let raw = runtime.cx().raw_cx();
        let _ = crate::job_queue::install_once(raw);
        SetHostCleanupFinalizationRegistryCallback(
            runtime.cx(),
            Some(drop_cleanup_job),
            ptr::null_mut(),
        );
    }
}

/// Install the host hooks on the shared per-thread runtime (`crate::with_runtime`), which builds its
/// own `Runtime` and therefore its own `JSContext`. **The hooks are per-context, so installing them
/// in one constructor does not cover the other** — and the page path is the one a real user's tab
/// runs on, so missing it would leave the crash live exactly where it matters most.
pub(crate) fn install_host_hooks_on(runtime: &mut Runtime) {
    install_host_hooks(runtime);
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
            // ── SAY WHAT THREW. `"uncaught exception while evaluating <file>"` is a shrug, not a
            // diagnostic: it is the same eleven words for a syntax error on line 1, a `TypeError`
            // from a missing IDL property, and an out-of-memory. This file's own sibling
            // (`dom_bindings::pending_exception`) has reported the real message for ticks — *"every
            // swallowed exception is a discarded bug report"* — and the RUNTIME's own eval was the
            // one path still swallowing it. It is also load-bearing for conformance: a negative
            // test's verdict is *which* error type was thrown, so a runner cannot score one against
            // an engine that will not say. Clears the exception (leaving it pending poisons the
            // next eval on this context), and degrades to the old wording if there is nothing to
            // read — an unstringifiable exception must not become a panic.
            // ⚠ The read must happen INSIDE the script's realm. `evaluate_script` has already left
            // it by the time it returns, and `JS_GetPendingException` asserts a current realm —
            // outside one it does not return an error, it **aborts the process**. (Measured: the
            // test262 runner core-dumped on its first failing case until this `JSAutoRealm` existed.)
            Err(()) => Err(JsError {
                message: unsafe {
                    let raw = self.runtime.cx().raw_cx();
                    let _ar = mozjs::jsapi::JSAutoRealm::new(raw, global.get());
                    pending_exception_message(raw)
                }
                .unwrap_or_else(|| format!("uncaught exception while evaluating {filename}")),
            }),
        }
    }

    fn engine_name(&self) -> &'static str {
        "SpiderMonkey (mozjs)"
    }

    /// A full, non-incremental collection. `GCReason::API` is the reason code SpiderMonkey reserves
    /// for an embedder asking explicitly, which keeps it distinguishable in a GC log from the
    /// engine's own heuristics.
    fn gc(&mut self) {
        unsafe {
            mozjs::jsapi::JS_GC(self.runtime.cx().raw_cx(), mozjs::jsapi::JS::GCReason::API);
        }
    }
}

/// The pending exception on `cx`, stringified, **and cleared** — `None` when there is nothing
/// pending or it cannot be read.
///
/// Clearing is not optional bookkeeping. A pending exception left on the context makes the *next*
/// call on it fail too, so the first real error would smear across every subsequent evaluation and
/// the second failure would be reported as though it had its own cause. That is how one bug becomes
/// a wall of noise with the true first line scrolled off the top.
///
/// `String::safe_from_jsval` runs the value's own `toString`, which for an `Error` yields
/// `"SyntaxError: unexpected token"` — the type name FIRST, which is exactly the discriminator a
/// conformance runner scores a negative test on. A thrown non-`Error` (test suites throw plain
/// objects and primitives) stringifies by the same rule the language uses everywhere else, and an
/// object whose `toString` itself throws returns `None` rather than re-entering the failure.
unsafe fn pending_exception_message(cx: *mut mozjs::jsapi::JSContext) -> Option<String> {
    use mozjs::conversions::{ConversionResult, FromJSValConvertible};
    let ptr = std::ptr::NonNull::new(cx)?;
    rooted!(in(cx) let mut ex = UndefinedValue());
    if !mozjs::jsapi::JS_GetPendingException(cx, ex.handle_mut().into()) {
        return None;
    }
    mozjs::jsapi::JS_ClearPendingException(cx);
    let mut c = mozjs::context::JSContext::from_ptr(ptr);
    match String::safe_from_jsval(&mut c, ex.handle(), ()) {
        Ok(ConversionResult::Success(s)) => Some(s),
        _ => None,
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

        // ── Bar 0 on this seam: `new FinalizationRegistry(fn)` used to SEGFAULT the process.
        //
        // Not throw — segfault. The constructor asks the host for the incumbent global through
        // `JS::JobQueue`, and this runtime installed none, so it dereferenced a null queue. The page
        // path was safe (it installs one at global setup); this seam — the one `manuk eval` and any
        // embedder of `JsRuntime` uses — was not, and nothing had ever asked it the question until a
        // conformance suite did (tick 546).
        //
        // RED-PROVEN: delete the `job_queue::install_once` line in `install_host_hooks` and this test
        // does not fail, it CORE-DUMPS the test binary. That is the shape of the bug, and it is why
        // it lived here for 500+ ticks: a crash reports nothing at all.
        assert_eq!(
            rt.eval(
                "var r = new FinalizationRegistry(function(){}); typeof r",
                "t.js"
            )
            .unwrap(),
            JsValue::Str("[string]".to_string()),
            "constructing a FinalizationRegistry must be an ordinary object construction"
        );

        // ── And the runtime must SAY WHAT THREW. `"uncaught exception while evaluating t.js"` is the
        // same eleven words for every failure; a conformance runner scores a negative test on the
        // error TYPE, so a runtime that will not name it cannot be measured against the spec.
        let e = rt.eval("throw new TypeError('boom')", "t.js").unwrap_err();
        assert!(
            e.message.starts_with("TypeError"),
            "the exception must be reported with its own type FIRST, got: {}",
            e.message
        );
        assert!(
            e.message.contains("boom"),
            "…and its message, got: {}",
            e.message
        );
        // A syntax error is reported the same way — and it is the discriminator ~4,000 test262
        // `negative: SyntaxError` cases are scored on.
        let s = rt.eval("var = ;", "t.js").unwrap_err();
        assert!(
            s.message.starts_with("SyntaxError"),
            "a parse failure must name SyntaxError, got: {}",
            s.message
        );
        // The exception must be CLEARED, or it smears onto the next evaluation and the second
        // failure is reported as though it had its own cause.
        assert_eq!(rt.eval("1 + 1", "t.js").unwrap(), JsValue::Number(2.0));
    }
}
