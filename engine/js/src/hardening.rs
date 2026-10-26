//! Agent-mode engine hardening (R1.6): SpiderMonkey without any JIT tier.
//!
//! The 12 recent SpiderMonkey CVEs in the sprint's evidence base were all JIT or wasm-JIT bugs, and
//! wasm is on ~0.35% of pages. `JS::DisableJitBackend()` is not bound by mozjs and must run before
//! `JS_Init`, so this uses the bound, runtime-settable equivalent: the global JIT compiler options
//! (Baseline interpreter, Baseline, Ion, native regexp and both wasm JIT tiers off). What remains is
//! the C++ interpreter and the portable baseline *interpreter*, which generate no machine code.
//!
//! A process-wide switch, like [`crate::determinism`]: the binary calls [`enable_jitless`] once, and
//! every runtime built afterwards (page path and bare `SpiderMonkeyRuntime`) applies it.

use std::sync::atomic::{AtomicBool, Ordering};

use mozjs::jsapi::JSJitCompilerOption as O;

static JITLESS: AtomicBool = AtomicBool::new(false);

pub fn enable_jitless() {
    JITLESS.store(true, Ordering::Relaxed);
}

pub fn jitless() -> bool {
    JITLESS.load(Ordering::Relaxed)
}

const TIERS: [O; 6] = [
    O::JSJITCOMPILER_ION_ENABLE,
    O::JSJITCOMPILER_BASELINE_ENABLE,
    O::JSJITCOMPILER_BASELINE_INTERPRETER_ENABLE,
    O::JSJITCOMPILER_NATIVE_REGEXP_ENABLE,
    O::JSJITCOMPILER_WASM_JIT_BASELINE,
    O::JSJITCOMPILER_WASM_JIT_OPTIMIZING,
];

/// Apply the switch to a fresh runtime. No-op unless [`enable_jitless`] was called.
pub(crate) fn apply(cx: &mozjs::context::JSContext) {
    if !jitless() {
        return;
    }
    for t in TIERS {
        unsafe { mozjs::rust::wrappers2::JS_SetGlobalJitCompilerOption(cx, t, 0) };
    }
}

/// Read back whether any JIT tier is on (used by the gate test).
pub fn jit_tiers_on(cx: &mozjs::context::JSContext) -> Vec<u32> {
    TIERS
        .iter()
        .map(|t| {
            let mut v = 0u32;
            unsafe { mozjs::rust::wrappers2::JS_GetGlobalJitCompilerOption(cx, *t, &mut v) };
            v
        })
        .collect()
}
