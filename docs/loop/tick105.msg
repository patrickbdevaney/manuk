docs(loop tick 105): Bar-0 triage -- the html/semantics crasher is a deterministic C-stack overflow

Per Bar-0 primacy, investigated the crasher that keeps html/semantics out of the sweep.
script-text-modifications-csp.html SIGSEGVs (exit 139) IN ISOLATION -- deterministic, so gdb WILL
catch it on a debug build (unlike the flexbox Heisenbug). gdb backtrace = tight repeating cycle
over NaN-boxed JS values = deep JS recursion overflowing the C stack: a SIGSEGV where SpiderMonkey
should throw "too much recursion" -- the stack-quota mis-anchoring class (Runtime::new's quota
computed from the async-buried nativeStackBase). setTimeout defers correctly, so the recursion is
script-text-setter + CSP re-eval, or the harness drain; needs a symboled build to pinpoint.

This is the teed-up effective-stack-quota fix WITH the clean deterministic repro tick 95 asked for
-- the best first target, a fresh-context job. Fixing it unblocks html/semantics (~8879 failing,
the biggest single mass) into the sweep. Recorded in js-engine.md + memory.

WIKI: docs/wiki/js-engine.md -- "A second, DETERMINISTIC C-stack overflow (html/semantics)"

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
