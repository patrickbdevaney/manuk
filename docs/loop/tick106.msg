docs(loop tick 106): stack-quota fix implemented + REVERTED -- the html/semantics crasher is NATIVE recursion

Acted on the teed-up stack-quota lever with the deterministic repro. Built the effective quota fix
(libc + pthread_getattr_np -> real thread bottom; set JS_SetNativeStackQuota at lib.rs with_runtime
where the page-eval RUNTIME is actually created, not SpiderMonkeyRuntime::new). It compiles and sets
a correct real-bottom quota (avail=8MB, JS limit ~360KB above bottom).

Negative result, correcting tick 105: the crasher STILL SIGSEGVs on the main thread (7MB headroom).
A JS stack quota only guards at JS call checkpoints; this overshoots them -> it is NATIVE recursion
in our own Rust (the <script>.textContent setter re-preparing the script / CSP re-eval, re-entering
itself). Not the stack-quota class. Reverted (doesn't hit its gate; unverified quota not landed on
hope).

Redirects: (1) this crasher needs a native-recursion fix -- a "script already started" guard, found
with a debug build. (2) the quota fix is real but its value is small-worker-thread JS recursion,
un-gateable on the main thread -- needs a worker-thread repro + full-sweep before landing; do NOT
re-attempt gated on this file.

WIKI: docs/wiki/js-engine.md -- corrected the "second crasher" section (NATIVE recursion, not the quota class)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
