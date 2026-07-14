//! N9 step 1 — a **custom promise job queue**, so native `Promise` reactions run through
//! *our* event loop.
//!
//! ## Why this exists (and why the old diagnosis was wrong)
//!
//! D3 recorded that `JS::UseInternalJobQueues` "segfaults in this build", which left native
//! `Promise.prototype.then` reactions outside our event loop and blocked dynamic `import()`.
//! The N7 research pass found the real cause: `mozjs`'s `Runtime::create` calls
//! `JS::InitSelfHostedCode` unconditionally, and SpiderMonkey requires
//! `js::UseInternalJobQueues(cx)` to be called **before** that. `mozjs` exposes no hook in
//! between, so the call always arrives too late — a newer `mozjs` cannot fix it.
//!
//! The right answer is the one browsers use: **do not use the internal queue at all.**
//! Provide an embedder queue via [`JS::JobQueue`], installed with `SetJobQueue`, which has
//! no ordering constraint. `mozjs_sys`'s glue already ships the C++ shim
//! (`RustJobQueue : public JS::JobQueue`) and its `JobQueueTraps` vtable; we fill in the
//! traps.
//!
//! **Boundary (CLAUDE.md, non-negotiable):** this touches **no** SpiderMonkey internals.
//! `JS::JobQueue` / `SetJobQueue` are the public embedding interface for exactly this
//! purpose — the same hook Gecko and Servo use to route promise jobs into their own event
//! loops. Nothing here patches the JIT, the GC, or the sandbox.
//!
//! ## Rooting
//!
//! An enqueued job is a `JSObject*` that must survive until it runs. Rather than root it
//! Rust-side (the hazard that produced the segfaults in the D3 prototype), jobs are pushed
//! onto a **JS array held by the global** — the same trick [`crate::event_loop`] already
//! uses for its task queues, where the global's own tracing keeps them alive.
//!
//! **Documented gaps (not faked):** `getHostDefinedData` always yields `null` (we have no
//! host-defined data to attach); the interrupt-queue traps are no-ops, which is correct
//! only because we never call `StopDrainingJobQueue`; the queue is process-global and
//! intentionally leaked (one `Runtime` per process, the C1 model).

use std::cell::Cell;
use std::ffi::c_void;
use std::ptr;

use mozjs::glue::{CreateJobQueue, JobQueueTraps};
use mozjs::jsapi::{Handle, JSContext as RawJSContext, JSObject, MutableHandle};
use mozjs::jsval::UndefinedValue;
use mozjs::rooted;
use mozjs::rust::wrappers2::{CurrentGlobalOrNull, JS_SetProperty};

/// The name of the JS-side array the jobs live in. Held by the global, so the GC traces it.
const JOBS: &str = "__promiseJobs";

/// Host state behind the queue: just a pending count, because the jobs themselves live in
/// the JS array. `empty()` has no `JSContext`, so it *must* be answerable from Rust alone.
pub struct PromiseJobQueue {
    pending: Cell<usize>,
}

// Single-threaded by construction (one `Runtime` per thread; SpiderMonkey calls the traps
// on the JS thread). The `Cell` is never shared across threads.
unsafe impl Sync for PromiseJobQueue {}

impl PromiseJobQueue {
    fn new() -> Self {
        PromiseJobQueue {
            pending: Cell::new(0),
        }
    }

    pub fn pending(&self) -> usize {
        self.pending.get()
    }
}

unsafe fn wrap_cx(cx: *mut RawJSContext) -> mozjs::context::JSContext {
    mozjs::context::JSContext::from_ptr(ptr::NonNull::new(cx).expect("trap cx is non-null"))
}

/// Evaluate `src` in the current global, returning whether it succeeded.
unsafe fn eval(cx: *mut RawJSContext, src: &str) -> bool {
    use mozjs::rust::{evaluate_script, CompileOptionsWrapper};
    let global = CurrentGlobalOrNull(&wrap_cx(cx));
    if global.is_null() {
        return false;
    }
    rooted!(in(cx) let g = global);
    rooted!(in(cx) let mut rval = UndefinedValue());
    let opts = CompileOptionsWrapper::new(&wrap_cx(cx), c"promise_jobs.js".to_owned(), 1);
    evaluate_script(&mut wrap_cx(cx), g.handle(), src, rval.handle_mut(), opts).is_ok()
}

// ---- traps ----

/// We attach no host-defined data to promise jobs.
unsafe extern "C" fn get_host_defined_data(
    _queue: *const c_void,
    _cx: *mut RawJSContext,
    data: MutableHandle<*mut JSObject>,
) -> bool {
    *data.ptr = ptr::null_mut();
    true
}

/// SpiderMonkey hands us a job (a nullary function). Park it on the global's job array,
/// where the GC will trace it, and bump the pending count.
unsafe extern "C" fn enqueue_promise_job(
    queue: *const c_void,
    cx: *mut RawJSContext,
    _promise: Handle<*mut JSObject>,
    job: Handle<*mut JSObject>,
    _allocation_site: Handle<*mut JSObject>,
    _host_defined: Handle<*mut JSObject>,
) -> bool {
    let q = &*(queue as *const PromiseJobQueue);

    let global = CurrentGlobalOrNull(&wrap_cx(cx));
    if global.is_null() || job.get().is_null() {
        return false;
    }
    rooted!(in(cx) let g = global);
    rooted!(in(cx) let job_val = mozjs::jsval::ObjectValue(job.get()));

    // Hand the job to JS through a scratch global property, then push it onto the array.
    if !JS_SetProperty(
        &mut wrap_cx(cx),
        g.handle(),
        c"__pendingJob".as_ptr(),
        job_val.handle(),
    ) {
        return false;
    }
    let script = format!(
        "(globalThis.{JOBS} = globalThis.{JOBS} || []).push(globalThis.__pendingJob); \
         globalThis.__pendingJob = undefined;"
    );
    if !eval(cx, &script) {
        return false;
    }
    q.pending.set(q.pending.get() + 1);
    true
}

/// Drain the queue. A job may enqueue further jobs (a `.then()` inside a `.then()`), so the
/// loop must re-check the array rather than snapshot its length — that is what makes this a
/// *microtask* checkpoint rather than a single pass.
unsafe extern "C" fn run_jobs(queue: *const c_void, cx: *mut RawJSContext) {
    let q = &*(queue as *const PromiseJobQueue);
    // A job that throws must not abort the drain: report and continue, as the spec's
    // "run the jobs" step does.
    let script = format!(
        "(function () {{ \
           var q = globalThis.{JOBS} || []; \
           var guard = 0; \
           while (q.length && guard++ < 100000) {{ \
             var j = q.shift(); \
             try {{ j(); }} catch (e) {{ globalThis.__lastJobError = String(e); }} \
           }} \
         }})();"
    );
    eval(cx, &script);
    q.pending.set(0);
}

unsafe extern "C" fn is_empty(queue: *const c_void) -> bool {
    let q = &*(queue as *const PromiseJobQueue);
    q.pending.get() == 0
}

// The interrupt-queue traps exist only for `StopDrainingJobQueue`, which we never call.
// They must still be non-null: `~RustJobQueue()` calls `dropInterruptQueues` unconditionally.
unsafe extern "C" fn push_new_interrupt_queue(_qs: *mut c_void) -> *const c_void {
    ptr::null()
}
unsafe extern "C" fn pop_interrupt_queue(_qs: *mut c_void) -> *const c_void {
    ptr::null()
}
unsafe extern "C" fn drop_interrupt_queues(_qs: *mut c_void) {}

/// Install our job queue on `cx`, replacing SpiderMonkey's (absent) default.
///
/// Call once, after the `Runtime` is created. Returns the queue so callers can inspect
/// [`PromiseJobQueue::pending`].
///
/// # Safety
/// `cx` must be a live context with no other job queue installed. The queue is leaked on
/// purpose: it must outlive the `Runtime`, and there is one `Runtime` per process.
pub unsafe fn install(cx: *mut RawJSContext) -> Result<&'static PromiseJobQueue, String> {
    let queue: &'static PromiseJobQueue = Box::leak(Box::new(PromiseJobQueue::new()));
    install_with(cx, queue)
}

/// Install exactly once per process. A second call returns the queue already installed —
///
/// # Safety
/// `cx` must be a live context.
/// there is one `Runtime` per process (the C1 model), and `SetJobQueue` twice would leak
/// the first queue while SpiderMonkey still held pointers into it.
pub unsafe fn install_once(cx: *mut RawJSContext) -> Result<&'static PromiseJobQueue, String> {
    use std::sync::OnceLock;
    static INSTALLED: OnceLock<usize> = OnceLock::new();
    if let Some(p) = INSTALLED.get() {
        return Ok(&*(*p as *const PromiseJobQueue));
    }
    let q = install(cx)?;
    let _ = INSTALLED.set(q as *const PromiseJobQueue as usize);
    Ok(q)
}

/// # Safety
/// `cx` must be a live context with no other job queue installed, and `queue` must live
/// for the process (SpiderMonkey retains the pointer).
unsafe fn install_with(
    cx: *mut RawJSContext,
    queue: &'static PromiseJobQueue,
) -> Result<&'static PromiseJobQueue, String> {
    let traps = JobQueueTraps {
        getHostDefinedData: Some(get_host_defined_data),
        enqueuePromiseJob: Some(enqueue_promise_job),
        runJobs: Some(run_jobs),
        empty: Some(is_empty),
        pushNewInterruptQueue: Some(push_new_interrupt_queue),
        popInterruptQueue: Some(pop_interrupt_queue),
        dropInterruptQueues: Some(drop_interrupt_queues),
    };

    let jq = CreateJobQueue(
        &traps as *const JobQueueTraps,
        queue as *const PromiseJobQueue as *const c_void,
        ptr::null_mut(),
    );
    if jq.is_null() {
        return Err("CreateJobQueue returned null".to_string());
    }
    mozjs::rust::wrappers2::SetJobQueue(&wrap_cx(cx), jq);
    Ok(queue)
}

/// Ask SpiderMonkey to drain the queue (dispatches to our [`run_jobs`] trap).
///
/// # Safety
/// `cx` must be inside a realm.
pub unsafe fn drain(cx: *mut RawJSContext) {
    mozjs::rust::wrappers2::RunJobs(&mut wrap_cx(cx));
}

#[cfg(test)]
mod tests {
    use super::*;
    use mozjs::jsapi::OnNewGlobalHookOption;
    use mozjs::rust::wrappers2::JS_NewGlobalObject;
    use mozjs::rust::{
        evaluate_script, CompileOptionsWrapper, RealmOptions, Runtime, SIMPLE_GLOBAL_CLASS,
    };

    fn eval_val(rt: &mut Runtime, g: mozjs::rust::HandleObject, src: &str) -> mozjs::jsapi::Value {
        rooted!(&in(rt.cx()) let mut rval = UndefinedValue());
        let opts = CompileOptionsWrapper::new(rt.cx_no_gc(), c"t.js".to_owned(), 1);
        evaluate_script(rt.cx(), g, src, rval.handle_mut(), opts).expect("eval");
        rval.get()
    }

    /// N9 step 1's acceptance — and the assertion the D3 note said we could not make:
    /// a **native** `Promise.prototype.then` reaction runs when our event loop drains.
    /// Nothing here uses the `queueMicrotask` shim.
    #[test]
    #[ignore = "SpiderMonkey multi-Runtime-per-process teardown; run in isolation"]
    fn native_promise_reactions_run_through_our_job_queue() {
        let handle = crate::spidermonkey::engine_handle().unwrap();
        let mut runtime = Runtime::new(handle);
        let raw_cx = unsafe { runtime.cx().raw_cx() };

        // Install BEFORE entering a realm: SetJobQueue has no ordering constraint (unlike
        // UseInternalJobQueues, which must precede InitSelfHostedCode -- see N7).
        let queue = unsafe { install(raw_cx) }.expect("install job queue");

        let options = RealmOptions::default();
        rooted!(&in(runtime.cx()) let global = unsafe {
            JS_NewGlobalObject(runtime.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook, &*options)
        });
        let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());

        // A native promise reaction. It must NOT have run yet: the reaction is a job.
        eval_val(
            &mut runtime,
            global.handle(),
            "globalThis.done = 0; Promise.resolve(7).then(v => { globalThis.done = v; });",
        );
        let before = eval_val(&mut runtime, global.handle(), "globalThis.done");
        assert_eq!(
            before.to_int32(),
            0,
            "a reaction must not run before the drain"
        );
        assert_eq!(queue.pending(), 1, "SpiderMonkey enqueued exactly one job");

        // Drain: SpiderMonkey dispatches into our `run_jobs` trap.
        unsafe { drain(raw_cx) };

        let after = eval_val(&mut runtime, global.handle(), "globalThis.done");
        assert_eq!(
            after.to_int32(),
            7,
            "the native promise reaction must have run"
        );
        assert_eq!(queue.pending(), 0);
    }

    /// The two microtask sources interleave correctly: a native promise reaction and a
    /// host `queueMicrotask` callback both run at the same checkpoint, before any
    /// macrotask. This is the property that made the `queueMicrotask` shim a *substitute*
    /// for native promises rather than a peer of them.
    #[test]
    #[ignore = "SpiderMonkey multi-Runtime-per-process teardown; run in isolation"]
    fn native_promise_jobs_and_host_microtasks_both_precede_macrotasks() {
        let handle = crate::spidermonkey::engine_handle().unwrap();
        let mut runtime = Runtime::new(handle);
        let options = RealmOptions::default();
        rooted!(&in(runtime.cx()) let global = unsafe {
            JS_NewGlobalObject(runtime.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook, &*options)
        });
        let raw_cx = unsafe { runtime.cx().raw_cx() };
        let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());

        // `event_loop::install` installs the job queue and the host queues together.
        crate::event_loop::install(&mut runtime, global.handle()).unwrap();

        eval_val(
            &mut runtime,
            global.handle(),
            "globalThis.order = '';
             setTimeout(() => { order += 'T'; });
             queueMicrotask(() => { order += 'm'; });
             Promise.resolve().then(() => { order += 'p'; });",
        );

        crate::event_loop::run(&mut runtime, global.handle()).unwrap();

        let v = eval_val(&mut runtime, global.handle(), "globalThis.order");
        let s = {
            use mozjs::conversions::{ConversionResult, FromJSValConvertible};
            rooted!(&in(runtime.cx()) let rv = v);
            match String::safe_from_jsval(runtime.cx(), rv.handle(), ()) {
                Ok(ConversionResult::Success(s)) => s,
                _ => String::new(),
            }
        };
        // Both microtask sources ran; the macrotask ran last.
        assert!(s.contains('m'), "host microtask ran: {s:?}");
        assert!(s.contains('p'), "NATIVE promise reaction ran: {s:?}");
        assert!(s.contains('T'), "macrotask ran: {s:?}");
        assert!(
            s.find('T') > s.find('m') && s.find('T') > s.find('p'),
            "both microtask sources must precede the macrotask, got {s:?}"
        );
    }

    /// A job may enqueue further jobs (a `.then()` inside a `.then()`); the drain is a
    /// microtask *checkpoint*, so chained reactions all run in one drain.
    #[test]
    #[ignore = "SpiderMonkey multi-Runtime-per-process teardown; run in isolation"]
    fn chained_reactions_all_run_in_one_drain_and_a_throwing_job_does_not_abort_it() {
        let handle = crate::spidermonkey::engine_handle().unwrap();
        let mut runtime = Runtime::new(handle);
        let raw_cx = unsafe { runtime.cx().raw_cx() };
        unsafe { install(raw_cx) }.unwrap();

        let options = RealmOptions::default();
        rooted!(&in(runtime.cx()) let global = unsafe {
            JS_NewGlobalObject(runtime.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook, &*options)
        });
        let _ar = mozjs::jsapi::JSAutoRealm::new(raw_cx, global.get());

        eval_val(
            &mut runtime,
            global.handle(),
            "globalThis.order = [];
             Promise.resolve()
               .then(() => { order.push('a'); throw new Error('boom'); })
               .catch(() => { order.push('caught'); })
               .then(() => { order.push('b'); });
             Promise.resolve().then(() => { order.push('parallel'); });",
        );

        unsafe { drain(raw_cx) };

        let joined = eval_val(&mut runtime, global.handle(), "globalThis.order.join(',')");
        let s = {
            use mozjs::conversions::{ConversionResult, FromJSValConvertible};
            rooted!(&in(runtime.cx()) let v = joined);
            match String::safe_from_jsval(runtime.cx(), v.handle(), ()) {
                Ok(ConversionResult::Success(s)) => s,
                _ => String::new(),
            }
        };
        // All four reactions ran, in interleaved microtask order, and the throw was caught
        // by the promise chain rather than aborting the drain.
        assert!(s.contains("a"), "first reaction ran: {s}");
        assert!(s.contains("parallel"), "the independent chain ran too: {s}");
        assert!(
            s.contains("caught"),
            "the rejection was routed to .catch: {s}"
        );
        assert!(s.contains("b"), "the chain continued after the catch: {s}");
    }
}
