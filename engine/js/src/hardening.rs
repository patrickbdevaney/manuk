//! Agent-mode engine hardening (R1.6, JIT.BASE): a tier set per mode. Default for agents is
//! `Baseline` (baseline interpreter + baseline compiler on; Ion/Warp, native regexp and both wasm JIT
//! tiers off); `--jitless` selects `None` (no JIT tier at all).
//!
//! The 12 recent SpiderMonkey CVEs in the sprint's evidence base were all JIT or wasm-JIT bugs, and
//! wasm is on ~0.35% of pages. `JS::DisableJitBackend()` is not bound by mozjs and must run before
//! `JS_Init`, so this uses the bound, runtime-settable equivalent: the global JIT compiler options
//! (Baseline interpreter, Baseline, Ion, native regexp and both wasm JIT tiers off). What remains is
//! the C++ interpreter and the portable baseline *interpreter*, which generate no machine code.
//!
//! A process-wide switch, like [`crate::determinism`]: the binary calls [`enable_jitless`] once, and
//! every runtime built afterwards (page path and bare `SpiderMonkeyRuntime`) applies it.

use std::sync::atomic::{AtomicU8, Ordering};

use mozjs::jsapi::JSJitCompilerOption as O;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JitMode {
    /// Engine defaults: every tier on (shell, tests).
    Full = 0,
    /// Baseline interpreter + baseline compiler only; the optimizing tiers stay off.
    Baseline = 1,
    /// No JIT tier at all.
    None = 2,
}

impl JitMode {
    pub fn name(self) -> &'static str {
        match self {
            JitMode::Full => "full",
            JitMode::Baseline => "baseline",
            JitMode::None => "none",
        }
    }
}

static MODE: AtomicU8 = AtomicU8::new(0);

pub fn set_mode(m: JitMode) {
    MODE.store(m as u8, Ordering::Relaxed);
}

pub fn mode() -> JitMode {
    match MODE.load(Ordering::Relaxed) {
        1 => JitMode::Baseline,
        2 => JitMode::None,
        _ => JitMode::Full,
    }
}

pub fn enable_jitless() {
    set_mode(JitMode::None);
}

pub fn enable_baseline_only() {
    set_mode(JitMode::Baseline);
}

/// SEC.DID.9: whether `target` (a URL, `file:` URL or path as typed) is a local page.
pub fn is_local_target(target: &str) -> bool {
    let t = target.trim();
    if t.starts_with("about:") || t.starts_with("file:") || t.starts_with("data:") {
        return true;
    }
    let Some((scheme, rest)) = t.split_once("://") else {
        return !t.contains('.') || t.starts_with('/') || t.starts_with('.');
    };
    if scheme != "http" && scheme != "https" {
        return true;
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or("");
    let host = if host.starts_with('[') {
        host.split(']').next().unwrap_or("").trim_start_matches('[')
    } else {
        host.split(':').next().unwrap_or("")
    };
    host == "localhost" || host == "::1" || host.starts_with("127.")
}

/// SEC.DID.9: the shell binary's tier set. Untrusted (non-local) pages get the same set as agent
/// mode (baseline tiers on, Ion/wasm-JIT off); only a local target runs every tier. `browse` can
/// navigate anywhere from its address bar, so it is baseline unless the user passes
/// `--jit-mode=full`; `override_mode` is that explicit choice.
pub fn shell_mode(browse: bool, target: Option<&str>, override_mode: Option<JitMode>) -> JitMode {
    if let Some(m) = override_mode {
        return m;
    }
    match target {
        Some(t) if !browse && is_local_target(t) => JitMode::Full,
        _ => JitMode::Baseline,
    }
}

pub fn jitless() -> bool {
    mode() == JitMode::None
}

const TIERS: [O; 6] = [
    O::JSJITCOMPILER_ION_ENABLE,
    O::JSJITCOMPILER_BASELINE_ENABLE,
    O::JSJITCOMPILER_BASELINE_INTERPRETER_ENABLE,
    O::JSJITCOMPILER_NATIVE_REGEXP_ENABLE,
    O::JSJITCOMPILER_WASM_JIT_BASELINE,
    O::JSJITCOMPILER_WASM_JIT_OPTIMIZING,
];

/// Whether `t` is on in `m`. Only the two baseline tiers survive `Baseline`.
fn wanted(m: JitMode, t: O) -> u32 {
    match m {
        JitMode::Baseline => matches!(
            t,
            O::JSJITCOMPILER_BASELINE_ENABLE | O::JSJITCOMPILER_BASELINE_INTERPRETER_ENABLE
        ) as u32,
        _ => 0,
    }
}

/// Apply the mode to a fresh runtime. No-op in `Full`.
pub(crate) fn apply(cx: &mozjs::context::JSContext) {
    let m = mode();
    if m == JitMode::Full {
        return;
    }
    for t in TIERS {
        unsafe { mozjs::rust::wrappers2::JS_SetGlobalJitCompilerOption(cx, t, wanted(m, t)) };
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
