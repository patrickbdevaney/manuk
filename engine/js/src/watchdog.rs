//! **The thread that makes the drain budget able to stop JavaScript.**
//!
//! The event loop has been bounded by a clock since `G_DRAIN_BUDGET`, and that bound is checked on
//! the **task boundary** — which bounds a runaway *chain* and is deliberately unable to touch a
//! single long-running task. Tick 1196 measured what that costs on the real corpus: the sweep's 150s
//! per-site timeout is not one pathological page, it is **four consecutive drain-budget overruns**,
//! and the budget could not preempt any of them because each was one task that never returned.
//!
//! ## The half that was missing, and it is the whole design of the API
//!
//! Tick 1197 built the obvious fix — a thread-local deadline, an interrupt callback returning
//! `false` past it, registered with [`JS_AddInterruptCallback`] on both drain paths — and it
//! **compiled, registered, and did nothing**: a 60s spin ran to completion, twice.
//!
//! > `JS_AddInterruptCallback` only **registers** the callback. SpiderMonkey polls it when an
//! > interrupt has been **REQUESTED** — a separate [`JS_RequestInterruptCallback`], which in Firefox
//! > is issued by a watchdog **thread**. Registration alone is inert by construction.
//!
//! So this module is that thread, and the reason it is a module rather than five lines inside
//! `event_loop` is the pointer it has to hold.
//!
//! ## Why a raw `*mut JSContext` crosses a thread here, and why that is sound
//!
//! SpiderMonkey is thread-affine and this file's neighbour ([`crate::spidermonkey`]) documents two
//! separate exit-crash classes that came from getting its lifetimes wrong (ADR-009). Three
//! properties keep this one contained, and they are the reason the design is *scoped to a drain*
//! rather than published for the process lifetime:
//!
//! 1. **`JS_RequestInterruptCallback` is the one entry point SpiderMonkey documents as callable from
//!    a thread other than the context's owner.** It sets a flag; it runs no JS, allocates nothing,
//!    and takes no GC lock. Everything else in this module stays on the JS thread.
//! 2. **The pointer is published only for the duration of a drain**, by a guard whose lifetime is a
//!    stack frame that holds `&mut Runtime`. It is cleared in that guard's `Drop`, so it cannot
//!    survive the runtime — the failure the neighbouring file exists to prevent.
//! 3. **Publication and clearing happen under the same mutex the watchdog must hold to use the
//!    pointer.** A clear that races a poll blocks until the poll's `JS_RequestInterruptCallback` has
//!    returned; the watchdog can never be inside SpiderMonkey with a context the JS thread has since
//!    dropped.
//!
//! The deadline itself is a plain atomic rather than mutex state, deliberately: the interrupt
//! callback runs on the **JS thread**, in the hot interrupt path, and must never be able to block on
//! the watchdog.
//!
//! ## What preemption is allowed to cost
//!
//! Terminating a script is the same policy the task ceiling and the clock budget already carry —
//! *"the page is not converging; paint what we have; the alternative is a frozen tab"* — applied to
//! the one shape they could not reach. It is **not** the North Star's *"fast because we never ran
//! the script"* trap for the same reason `G_DRAIN_BUDGET` is not: the runaway is not doing the
//! page's work, it is starving it. The gate proves that negative directly.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use mozjs::jsapi::{JSContext, JS_AddInterruptCallback, JS_RequestInterruptCallback};

/// The context the watchdog is permitted to interrupt.
///
/// `*mut JSContext` is not `Send`, and correctly so — almost nothing about a context may be touched
/// off-thread. This wrapper narrows that to the one call that may, and the narrowing is enforced by
/// the fact that this type is private and the only place it is dereferenced is [`watchdog_loop`].
struct CxSlot(*mut JSContext);

// SAFETY: the ONLY thing done with this pointer off the JS thread is `JS_RequestInterruptCallback`,
// which SpiderMonkey documents as safe from any thread. The slot is filled and cleared under
// [`cx_slot`]'s mutex by a guard that lives inside a drain (which holds `&mut Runtime`), so the
// pointer cannot outlive the context, and a clear cannot complete while a poll is mid-call.
unsafe impl Send for CxSlot {}

static CX: OnceLock<Mutex<Option<CxSlot>>> = OnceLock::new();

fn cx_slot() -> &'static Mutex<Option<CxSlot>> {
    CX.get_or_init(|| Mutex::new(None))
}

/// A process-wide monotonic origin, so the deadline is one `u64` compare in the interrupt path
/// rather than an `Instant` behind a lock.
static BASE: OnceLock<Instant> = OnceLock::new();

fn now_ms() -> u64 {
    BASE.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Milliseconds since [`BASE`] at which the running script must stop. `0` means **disarmed**, which
/// is the state outside a drain and the state the moment a termination is decided.
static DEADLINE: AtomicU64 = AtomicU64::new(0);

/// Set by [`interrupt_cb`] when it actually terminated a script. Read by the drain loops to tell
/// *"the page threw"* from *"we cut the page off"* — two identical-looking `Err`s with opposite
/// meanings.
static FIRED: AtomicBool = AtomicBool::new(false);

/// How often the watchdog asks. Small enough that the cut lands close to the budget, large enough
/// that an idle browser's watchdog thread is invisible: 50 wakeups a second, each an atomic load
/// that almost always returns `0` and goes straight back to sleep.
const POLL_MS: u64 = 20;

/// **Did the watchdog terminate a script since the current deadline was armed?**
///
/// The drain loops ask this before propagating an `Err`. A terminated script and a page that threw
/// arrive at exactly the same place with exactly the same shape, and only this flag distinguishes
/// *"the page is broken"* from *"we stopped it on purpose"*.
pub fn fired() -> bool {
    FIRED.load(Ordering::SeqCst)
}

/// Runs on the JS thread, inside SpiderMonkey's interrupt poll. Returning `false` terminates the
/// running script with an uncatchable error — the same mechanism as Firefox's slow-script stop.
///
/// It re-checks the deadline rather than trusting the request: the watchdog may have asked at the
/// last instant of a drain whose guard has since disarmed, and a spurious termination of the *next*
/// page's script would be a capability regression wearing a performance fix's clothes.
unsafe extern "C" fn interrupt_cb(_cx: *mut JSContext) -> bool {
    let deadline = DEADLINE.load(Ordering::SeqCst);
    if deadline != 0 && now_ms() >= deadline {
        FIRED.store(true, Ordering::SeqCst);
        // Disarm at the moment of decision, on the JS thread. The drain still has to run its final
        // microtask checkpoint and the page still has to service events; leaving the deadline armed
        // would terminate all of that too, and "we cut the runaway" would silently become "we cut
        // the page".
        DEADLINE.store(0, Ordering::SeqCst);
        return false;
    }
    true
}

thread_local! {
    /// Contexts on this thread that already carry [`interrupt_cb`]. `JS_AddInterruptCallback`
    /// *appends* — calling it once per drain would grow SpiderMonkey's callback vector for the life
    /// of the process.
    static REGISTERED: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn register_once(cx: *mut JSContext) {
    REGISTERED.with(|reg| {
        let mut reg = reg.borrow_mut();
        if reg.contains(&(cx as usize)) {
            return;
        }
        // SAFETY: called from the context's own thread (the caller holds `&mut Runtime`).
        unsafe { JS_AddInterruptCallback(cx, Some(interrupt_cb)) };
        reg.push(cx as usize);
    });
}

fn spawn_once() {
    static SPAWNED: OnceLock<()> = OnceLock::new();
    SPAWNED.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("manuk-js-watchdog".to_string())
            .spawn(watchdog_loop);
    });
}

fn watchdog_loop() {
    loop {
        std::thread::sleep(Duration::from_millis(POLL_MS));
        let deadline = DEADLINE.load(Ordering::SeqCst);
        if deadline == 0 || now_ms() < deadline {
            continue;
        }
        let Ok(slot) = cx_slot().lock() else { continue };
        if let Some(CxSlot(cx)) = slot.as_ref() {
            // SAFETY: see the module docs and `unsafe impl Send for CxSlot`. The lock is held
            // across this call precisely so a concurrent `ScriptDeadline::drop` cannot retire the
            // context underneath it.
            unsafe { JS_RequestInterruptCallback(*cx) };
        }
    }
}

/// **Arm the running drain's clock budget against the SCRIPT**, not only against the task boundary.
///
/// Held for the duration of one drain. Nested arms are no-ops that leave the outer deadline standing
/// — a drain driven from inside another drain must not be able to hand itself a fresh budget, which
/// is the loophole a re-entrant page would otherwise use to reset the clock forever.
pub struct ScriptDeadline {
    /// `true` only for the guard that actually armed. The nested/disabled guards clear nothing.
    owns: bool,
}

impl ScriptDeadline {
    /// `budget_ms == 0` is the documented "no clock bound" mode (`MANUK_MAX_DRAIN_MS=0`), and it must
    /// stay genuinely unbounded — it is the counterfactual arm of `G_DRAIN_BUDGET` and of this
    /// module's own gate.
    pub fn arm(cx: *mut JSContext, budget_ms: u128) -> Self {
        if budget_ms == 0 || cx.is_null() || DEADLINE.load(Ordering::SeqCst) != 0 {
            return ScriptDeadline { owns: false };
        }
        register_once(cx);
        FIRED.store(false, Ordering::SeqCst);
        if let Ok(mut slot) = cx_slot().lock() {
            *slot = Some(CxSlot(cx));
        }
        DEADLINE.store(now_ms().saturating_add(budget_ms as u64), Ordering::SeqCst);
        spawn_once();
        ScriptDeadline { owns: true }
    }
}

impl Drop for ScriptDeadline {
    fn drop(&mut self) {
        if !self.owns {
            return;
        }
        DEADLINE.store(0, Ordering::SeqCst);
        // Clear the verdict with the deadline that produced it. A stale `true` would make the NEXT
        // drain read a genuine page error as "we cut it off" and swallow it — the silent-failure
        // shape `G_SILENT_FAIL` exists to forbid.
        FIRED.store(false, Ordering::SeqCst);
        if let Ok(mut slot) = cx_slot().lock() {
            *slot = None;
        }
    }
}
