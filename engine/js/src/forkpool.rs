//! **R3.4 — make the process `fork()`-able after a page has loaded.**
//!
//! SpiderMonkey owns eight "JS Helper" threads (GC sweep, off-thread parse, Ion). A `fork()` copies
//! only the calling thread, so a child inherits SpiderMonkey's idea that those threads exist and the
//! first task it waits on never completes: the child HANGS (measured: 4 of 4 children hang with the
//! stock pool, `fork-probe` mode `naive`).
//!
//! The public seam that fixes it is `JS::SetHelperThreadTaskCallback` (js/public/HelperThreadAPI.h):
//! SpiderMonkey hands every helper task to *our* callback instead of its own threads, and we run it
//! with `JS::RunHelperThreadTask`. That makes the pool ours, so we can [`quiesce`] it (drain, join)
//! before a fork and [`resume`] it afterwards in the parent and in each child.
//!
//! Opt-in: [`enable`] must run before the first SpiderMonkey runtime exists. Without it nothing here
//! changes behaviour, and [`quiesce`] reports that the pool is not ours.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex, OnceLock};
use std::thread::JoinHandle;

/// Opaque `JS::HelperThreadTask`.
#[repr(C)]
pub struct HelperThreadTask {
    _p: [u8; 0],
}

type Callback = unsafe extern "C" fn(*mut HelperThreadTask);

// The symbols are Itanium-mangled C++ names; MSVC mangles differently, so on non-unix targets the
// pool is never wired in (the fork zygote that needs it is unix-only too) and these are inert.
#[cfg(unix)]
extern "C" {
    #[link_name = "_ZN2JS27SetHelperThreadTaskCallbackEPFvPNS_16HelperThreadTaskEEmm"]
    fn set_helper_thread_task_callback(cb: Callback, thread_count: usize, stack_size: usize);
    #[link_name = "_ZN2JS19RunHelperThreadTaskEPNS_16HelperThreadTaskE"]
    fn run_helper_thread_task(task: *mut HelperThreadTask);
}
#[cfg(not(unix))]
unsafe fn set_helper_thread_task_callback(_cb: Callback, _thread_count: usize, _stack_size: usize) {
}
#[cfg(not(unix))]
unsafe fn run_helper_thread_task(_task: *mut HelperThreadTask) {}

const THREADS: usize = 4;
const STACK: usize = 2 * 1024 * 1024;

struct State {
    queue: VecDeque<usize>,
    running: usize,
    stop: bool,
    workers: Vec<JoinHandle<()>>,
}

struct Pool {
    st: Mutex<State>,
    work: Condvar,
    idle: Condvar,
}

static POOL: OnceLock<Pool> = OnceLock::new();

fn pool() -> Option<&'static Pool> {
    POOL.get()
}

unsafe extern "C" fn dispatch(task: *mut HelperThreadTask) {
    // Called by SpiderMonkey from any thread, possibly with its own locks held: queue and return.
    if let Some(p) = pool() {
        if let Ok(mut s) = p.st.lock() {
            s.queue.push_back(task as usize);
        }
        p.work.notify_one();
    }
}

fn worker(p: &'static Pool) {
    loop {
        let task = {
            let Ok(mut s) = p.st.lock() else { return };
            loop {
                if let Some(t) = s.queue.pop_front() {
                    s.running += 1;
                    break t;
                }
                if s.stop {
                    return;
                }
                s = match p.work.wait(s) {
                    Ok(g) => g,
                    Err(_) => return,
                };
            }
        };
        // SAFETY: `task` came from SpiderMonkey's dispatch callback and is run exactly once.
        unsafe { run_helper_thread_task(task as *mut HelperThreadTask) };
        if let Ok(mut s) = p.st.lock() {
            s.running -= 1;
        }
        p.idle.notify_all();
    }
}

fn spawn_workers(p: &'static Pool) {
    let Ok(mut s) = p.st.lock() else { return };
    s.stop = false;
    while s.workers.len() < THREADS {
        let h = std::thread::Builder::new()
            .name("JS Helper".to_string())
            .stack_size(STACK)
            .spawn(move || worker(p));
        match h {
            Ok(h) => s.workers.push(h),
            Err(_) => break,
        }
    }
}

/// Ask for the embedder-owned helper pool. Must run before the first SpiderMonkey runtime exists;
/// the pool is wired in by [`install`] right after `JS_Init`. Idempotent.
pub fn enable() {
    POOL.get_or_init(|| Pool {
        st: Mutex::new(State {
            queue: VecDeque::new(),
            running: 0,
            stop: false,
            workers: Vec::new(),
        }),
        work: Condvar::new(),
        idle: Condvar::new(),
    });
}

/// Wire the pool into SpiderMonkey if [`enable`] was called. It has to be AFTER `JS_Init` (the
/// helper-thread state does not exist before it — the call segfaults) and BEFORE the first
/// `JSContext` (which is what starts SpiderMonkey's own threads).
pub(crate) fn install() {
    let Some(p) = POOL.get() else { return };
    spawn_workers(p);
    // SAFETY: after JS_Init, before any context, once (engine_handle holds the init lock).
    unsafe { set_helper_thread_task_callback(dispatch, THREADS, STACK) };
}

/// What [`quiesce`] stopped, to be handed back to [`resume`].
#[must_use]
pub struct Quiesced {
    pool: bool,
    watchdog: bool,
}

/// Drain the helper pool and the script watchdog, then join their threads: afterwards this process
/// has no thread but the caller's from SpiderMonkey's side, and is safe to `fork()`. The caller must
/// be between drains (no script running, no deadline armed).
pub fn quiesce() -> Quiesced {
    let watchdog = crate::watchdog::quiesce();
    let mut pool = false;
    if let Some(p) = pool_ref() {
        pool = true;
        let workers = {
            let Ok(mut s) = p.st.lock() else {
                return Quiesced { pool, watchdog };
            };
            while !(s.queue.is_empty() && s.running == 0) {
                s = match p.idle.wait(s) {
                    Ok(g) => g,
                    Err(_) => return Quiesced { pool, watchdog },
                };
            }
            s.stop = true;
            std::mem::take(&mut s.workers)
        };
        p.work.notify_all();
        for w in workers {
            let _ = w.join();
        }
    }
    Quiesced { pool, watchdog }
}

fn pool_ref() -> Option<&'static Pool> {
    POOL.get()
}

/// Restart what [`quiesce`] stopped. Call in the parent after `fork()` and in every child.
pub fn resume(q: &Quiesced) {
    if q.pool {
        if let Some(p) = pool_ref() {
            // A forked child may inherit a poisoned/held lock only if a thread held it at fork;
            // quiesce joined every holder.
            spawn_workers(p);
        }
    }
    crate::watchdog::resume(q.watchdog);
}

/// Live threads of this process (kernel's count), for the "quiesced" assertion.
pub fn thread_count() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("Threads:").map(|v| v.trim().parse().ok()))
                .flatten()
        })
        .unwrap_or(0)
}

/// Set in a forked child: the next script entry reseeds `Math.random` for the page realm.
static RESEED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Call in a forked child (after [`resume`]). A realm's `Math.random` is an xorshift state that
/// lives in the heap SpiderMonkey just duplicated, so every child that already drew a number
/// replays the parent's stream. Measured (fork-probe, 4 children): `JS::ResetRealmMathRandomSeed`
/// (public, js/public/Realm.h) is NOT enough — sibling children still drew the identical value
/// after it — so the reseed installs a fresh OS-seeded xorshift128+ as `Math.random` (the same
/// shape `--deterministic` uses), applied at the next script entry into the page realm.
pub fn mark_child_for_reseed() {
    RESEED.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub(crate) fn reseed_if_pending(
    runtime: &mut mozjs::rust::Runtime,
    global: mozjs::rust::HandleObject,
) {
    use std::sync::atomic::Ordering;
    if !RESEED.swap(false, Ordering::SeqCst) {
        return;
    }
    let mut seed = [0u8; 16];
    if getrandom::fill(&mut seed).is_err() {
        RESEED.store(true, Ordering::SeqCst);
        return;
    }
    let w = |i: usize| u32::from_le_bytes([seed[i], seed[i + 1], seed[i + 2], seed[i + 3]]);
    // A page that cached `var r = Math.random` before the fork holds the ORIGINAL native, whose
    // per-realm xorshift state is in the duplicated heap. `JS::ResetRealmMathRandomSeed` cannot reach
    // it (js/src/vm/Realm.cpp: `auto rng = ...getOrCreateRandomNumberGenerator()` copies the
    // generator, so `setState` writes a temporary - a no-op, which is what was measured). The only
    // public lever is to advance the native stream by a child-specific, OS-random count first, so
    // siblings' cached calls land on different outputs (a shifted window of one stream, distinct but
    // NOT independent - documented, and the page-visible `Math.random` below is fully independent).
    let burn = w(12) & 0x3f_ffff;
    let src = format!(
        "(function(){{var f=Math.random;var s0={}|1,s1={},s2={},s3={};Math.random=function random(){{\
         var t=s1<<9,r=s0*5;r=((r<<7)|(r>>>25))*9;s2^=s0;s3^=s1;s1^=s2;s0^=s3;s2^=t;s3=(s3<<11)|(s3>>>21);\
         return ((r>>>0)+((s1>>>0)&0x1fffff)*4294967296)/9007199254740992;}};\
         for(var n={burn};n>0;n--)f();}})()",
        w(0),
        w(4),
        w(8),
        w(12)
    );
    use mozjs::rooted;
    rooted!(&in(runtime.cx()) let mut rval = mozjs::jsval::UndefinedValue());
    let opts = mozjs::rust::CompileOptionsWrapper::new(
        runtime.cx_no_gc(),
        c"fork-reseed.js".to_owned(),
        1,
    );
    let _ = mozjs::rust::evaluate_script(runtime.cx(), global, &src, rval.handle_mut(), opts);
}
