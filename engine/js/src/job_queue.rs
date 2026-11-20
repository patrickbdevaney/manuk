//! N9 step 1 — a **custom job-queue host**, so native `Promise` reactions run through
//! *our* event loop.
//!
//! ## History (SpiderMonkey 140 → 153)
//!
//! On 140 the embedder owned the queue: SpiderMonkey called an `enqueuePromiseJob` trap and we
//! parked each job in a rooted JS array. From 153 the **engine owns the microtask queue**
//! (`js/public/friend/MicroTask.h`): promise reactions are enqueued inside SpiderMonkey, and the
//! embedder only provides `runJobs` — pull each task off the queue and run it. That is exactly the
//! job the old array/slot machinery did by hand, so it is gone; what remains is the checkpoint
//! (`run_jobs`) and the (null) host-defined-data traps.
//!
//! `JS::UseInternalJobQueues` is still unusable (`mozjs` calls `InitSelfHostedCode` before we can),
//! so we still install an embedder `JS::JobQueue` via `SetJobQueue`, through `mozjs_sys`'s
//! `RustJobQueue` shim and its `JobQueueTraps` vtable.
//!
//! **Boundary (CLAUDE.md, non-negotiable):** public embedding interface only; no SpiderMonkey
//! internals are touched.
//!
//! **Documented gaps (not faked):** `getHostDefinedData`/`getHostDefinedGlobal` yield `null` (no
//! host-defined data); the queue is process-global and intentionally leaked (one `Runtime` per
//! process, the C1 model).

use std::ptr;

use mozjs::glue::{CreateJobQueue, JobQueueTraps};
use mozjs::jsapi::{JSContext as RawJSContext, JSObject, JSTracer, MutableHandle, Value};
#[cfg(test)]
use mozjs::jsval::UndefinedValue;
use mozjs::rooted;

static INSTALLED_Q: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// Handle to the installed queue. The jobs live inside SpiderMonkey now, so this is only a witness
/// that the queue is installed; ask it for the pending count with [`PromiseJobQueue::pending`].
pub struct PromiseJobQueue;

impl PromiseJobQueue {
    /// Number of promise jobs SpiderMonkey currently holds for this context.
    ///
    /// # Safety
    /// `cx` must be a live context.
    pub unsafe fn pending(&self, cx: *mut RawJSContext) -> usize {
        mozjs::jsapi::JS::GetRegularMicroTaskCount(cx)
    }
}

unsafe fn wrap_cx(cx: *mut RawJSContext) -> mozjs::context::JSContext {
    mozjs::context::JSContext::from_ptr(ptr::NonNull::new(cx).expect("trap cx is non-null"))
}

// ---- traps ----

/// We attach no host-defined data to promise jobs.
unsafe extern "C" fn get_host_defined_data(
    _cx: *mut RawJSContext,
    incumbent_global: MutableHandle<*mut JSObject>,
    optional_host_defined_data: MutableHandle<*mut JSObject>,
) -> bool {
    *incumbent_global.ptr = ptr::null_mut();
    *optional_host_defined_data.ptr = ptr::null_mut();
    true
}

unsafe extern "C" fn get_host_defined_global(
    _cx: *mut RawJSContext,
    data: MutableHandle<*mut JSObject>,
) -> bool {
    *data.ptr = ptr::null_mut();
    true
}

/// Every task we queue is a JS microtask (a GC thing), so SpiderMonkey traces it itself.
unsafe extern "C" fn trace_non_gc_thing_micro_task(_trc: *mut JSTracer, _value: *mut Value) {}

/// Drain the queue. A job may enqueue further jobs (a `.then()` inside a `.then()`), so the
/// loop pulls until the engine's queue is empty — that is what makes this a *microtask*
/// checkpoint rather than a single pass.
unsafe extern "C" fn run_jobs(cx: *mut RawJSContext) {
    use mozjs::jsapi::JS;
    loop {
        let task = JS::DequeueNextRegularMicroTask(cx);
        if task.is_null_or_undefined() {
            break;
        }
        // Rooted straight away: no GC can run between the dequeue and here.
        rooted!(in(cx) let task = task);
        if !JS::IsJSMicroTask(&task.get()) {
            // We enqueue no embedder tasks; nothing to run for a foreign one.
            continue;
        }
        rooted!(in(cx) let job = JS::ToUnwrappedJSMicroTask(&task.get()));
        if job.get().is_null() {
            continue;
        }
        let global = JS::GetExecutionGlobalFromJSMicroTask(job.get());
        if global.is_null() {
            continue; // dead wrapper
        }
        let _ar = mozjs::jsapi::JSAutoRealm::new(cx, global);
        // A job that throws must not abort the drain (spec "run the jobs").
        if !JS::RunJSMicroTask(cx, job.handle().into()) {
            mozjs::rust::wrappers2::JS_ClearPendingException(&wrap_cx(cx));
        }
    }
}

/// Reset hook kept for the callers that cleared the 140 fast-path cache when a global was created.
/// There is no per-global cache any more, so this is a no-op.
pub fn reset_global_cache() {}

/// Install our job queue on `cx`, replacing SpiderMonkey's (absent) default.
///
/// # Safety
/// `cx` must be a live context with no other job queue installed. The queue is leaked on
/// purpose: it must outlive the `Runtime`, and there is one `Runtime` per process.
pub unsafe fn install(cx: *mut RawJSContext) -> Result<&'static PromiseJobQueue, String> {
    static Q: PromiseJobQueue = PromiseJobQueue;
    install_with(cx, &Q)
}

/// Install exactly once per process. A second call returns the queue already installed —
/// there is one `Runtime` per process (the C1 model), and `SetJobQueue` twice would leak
/// the first queue while SpiderMonkey still held pointers into it.
///
/// # Safety
/// `cx` must be a live context.
pub unsafe fn install_once(cx: *mut RawJSContext) -> Result<&'static PromiseJobQueue, String> {
    if let Some(p) = INSTALLED_Q.get() {
        return Ok(&*(*p as *const PromiseJobQueue));
    }
    let q = install(cx)?;
    let _ = INSTALLED_Q.set(q as *const PromiseJobQueue as usize);
    Ok(q)
}

unsafe fn install_with(
    cx: *mut RawJSContext,
    queue: &'static PromiseJobQueue,
) -> Result<&'static PromiseJobQueue, String> {
    let traps = JobQueueTraps {
        getHostDefinedData: Some(get_host_defined_data),
        getHostDefinedGlobal: Some(get_host_defined_global),
        runJobs: Some(run_jobs),
        traceNonGCThingMicroTask: Some(trace_non_gc_thing_micro_task),
    };
    // The shim copies the traps, so a stack value is fine.
    let jq = CreateJobQueue(&traps as *const JobQueueTraps);
    if jq.is_null() {
        return Err("CreateJobQueue returned null".to_string());
    }
    mozjs::rust::wrappers2::SetJobQueue(&wrap_cx(cx), jq as *mut mozjs::jsapi::JS::JobQueue);
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
        assert_eq!(
            unsafe { queue.pending(raw_cx) },
            1,
            "SpiderMonkey enqueued exactly one job"
        );

        // Drain: SpiderMonkey dispatches into our `run_jobs` trap.
        unsafe { drain(raw_cx) };

        let after = eval_val(&mut runtime, global.handle(), "globalThis.done");
        assert_eq!(
            after.to_int32(),
            7,
            "the native promise reaction must have run"
        );
        assert_eq!(unsafe { queue.pending(raw_cx) }, 0);
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
            match String::from_jsval(runtime.cx(), rv.handle(), ()) {
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
            match String::from_jsval(runtime.cx(), v.handle(), ()) {
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
