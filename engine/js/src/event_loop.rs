//! D3 **events tranche — the event loop** (first piece: the loop core).
//!
//! The HTML event loop is *run a task, then drain the microtask queue*, repeated.
//! This module provides that substrate over SpiderMonkey, host-side:
//!
//! - **Macrotasks** (`setTimeout` callbacks, and later DOM event dispatch): a FIFO of
//!   callback functions in a rooted global array `__tasks` (rooted for free because
//!   the global is). `setTimeout(fn)` pushes; [`run`] pops one per turn.
//! - **Microtasks** (`queueMicrotask` callbacks): a second FIFO `__micro`, drained
//!   **fully** at each checkpoint (a microtask may queue more microtasks), before the
//!   next macrotask and once at the end — the spec's microtask-before-next-task rule.
//!
//! Driving both queues through `evaluate_script` (rather than low-level `Call`) keeps
//! this piece small and rooting-safe: callbacks live in the rooted global arrays, not
//! in Rust.
//!
//! **Scope / honest limitation:** this is the host loop core (`setTimeout` +
//! `queueMicrotask` with correct ordering). It does **not** yet route native
//! `Promise.prototype.then` reactions through the loop: that needs SpiderMonkey's job
//! queue (`JS::UseInternalJobQueues` + `RunJobs`), whose mozjs-0.18 wrapper *segfaults
//! on call in this build* — a real environment blocker, tracked in CLAUDE.md § D3.
//! `queueMicrotask` (a first-class web API) gives the same ordering guarantees for
//! host-scheduled microtasks in the meantime. `addEventListener`/`dispatchEvent`
//! (DOM events routed through this loop) are the next piece.
//!
//! Modification boundary: sanctioned embedding APIs only (script evaluation); no
//! JIT/GC/sandbox internals.

use mozjs::rust::Runtime;

/// The host prelude: the macrotask + microtask FIFOs and their schedulers. Evaluated
/// once per global.
const PRELUDE: &str = r#"
    globalThis.__tasks = [];
    globalThis.__micro = [];
    globalThis.setTimeout = function(cb) {
        if (typeof cb === 'function') { __tasks.push(cb); }
        return __tasks.length;   // a fake, monotonic timer id
    };
    globalThis.queueMicrotask = function(cb) {
        if (typeof cb === 'function') { __micro.push(cb); }
    };
"#;

/// Run the next macrotask if any; report whether one ran.
const NEXT_TASK: &str =
    "(function(){ if (__tasks.length === 0) return false; var t = __tasks.shift(); t(); return true; })()";

/// Drain the microtask queue completely (microtasks may enqueue more microtasks).
const DRAIN_MICRO: &str =
    "(function(){ while (__micro.length) { var m = __micro.shift(); m(); } })()";

/// Install the event-loop host surface onto `global` (the queues + schedulers). Call
/// once, inside the global's realm.
pub fn install(rt: &mut Runtime, global: mozjs::rust::HandleObject) -> Result<(), String> {
    eval(rt, global, PRELUDE, "event_loop_prelude.js").map(|_| ())
}

/// Run the event loop to quiescence: drain microtasks, then run each macrotask (each
/// followed by a full microtask drain), until no macrotasks remain. Returns the
/// number of macrotasks run.
pub fn run(rt: &mut Runtime, global: mozjs::rust::HandleObject) -> Result<u32, String> {
    let mut count = 0u32;
    loop {
        eval(rt, global, DRAIN_MICRO, "event_loop_micro.js")?; // checkpoint
        let ran = eval(rt, global, NEXT_TASK, "event_loop_tick.js")?;
        if !ran.is_boolean() || !ran.to_boolean() {
            break;
        }
        count += 1;
    }
    eval(rt, global, DRAIN_MICRO, "event_loop_micro.js")?; // final checkpoint
    Ok(count)
}

/// Evaluate `src` and return its (unrooted, immediately-consumed) value. Kept local
/// so callers never touch rooting.
fn eval(
    rt: &mut Runtime,
    global: mozjs::rust::HandleObject,
    src: &str,
    file: &str,
) -> Result<mozjs::jsapi::Value, String> {
    use mozjs::jsval::UndefinedValue;
    use mozjs::rooted;
    use mozjs::rust::{evaluate_script, CompileOptionsWrapper};
    rooted!(&in(rt.cx()) let mut rval = UndefinedValue());
    let fname = std::ffi::CString::new(file).unwrap_or_default();
    let opts = CompileOptionsWrapper::new(rt.cx_no_gc(), fname, 1);
    evaluate_script(rt.cx(), global, src, rval.handle_mut(), opts)
        .map_err(|()| format!("event-loop eval failed: {file}"))?;
    Ok(rval.get())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mozjs::jsapi::OnNewGlobalHookOption;
    use mozjs::rooted;
    use mozjs::rust::wrappers2::JS_NewGlobalObject;
    use mozjs::rust::{RealmOptions, SIMPLE_GLOBAL_CLASS};
    use std::ptr;

    // Run isolated (SpiderMonkey multi-Runtime teardown):
    //   cargo test -p manuk-js --features spidermonkey event_loop -- --ignored
    #[test]
    #[ignore = "SpiderMonkey multi-Runtime-per-process teardown; run in isolation"]
    fn microtasks_run_before_macrotasks() {
        let handle = crate::spidermonkey::engine_handle().expect("engine");
        let mut runtime = Runtime::new(handle);
        let options = RealmOptions::default();
        rooted!(&in(runtime.cx()) let global = unsafe {
            JS_NewGlobalObject(runtime.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook, &*options)
        });
        let _ar = mozjs::jsapi::JSAutoRealm::new(unsafe { runtime.cx().raw_cx() }, global.get());

        install(&mut runtime, global.handle()).expect("install event loop");

        // A microtask (p), and a macrotask (T) that itself queues a nested microtask
        // (m). The loop must drain the microtask BEFORE the macrotask, then run the
        // macrotask, then drain its nested microtask → "pTm". A second macrotask (U)
        // confirms FIFO macrotask order → final "pTmU".
        let user = r#"
            globalThis.log = "";
            queueMicrotask(function(){ log += "p"; });
            setTimeout(function(){ log += "T"; queueMicrotask(function(){ log += "m"; }); });
            setTimeout(function(){ log += "U"; });
        "#;
        eval(&mut runtime, global.handle(), user, "user.js").expect("user script");

        let ran = run(&mut runtime, global.handle()).expect("run loop");
        assert_eq!(ran, 2, "two macrotasks ran");

        let ok =
            eval(&mut runtime, global.handle(), "log === 'pTmU'", "check.js").expect("read log");
        assert!(
            ok.is_boolean() && ok.to_boolean(),
            "expected ordering pTmU (microtask before macrotask, FIFO macrotasks)"
        );
    }
}
