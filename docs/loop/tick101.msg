feat(loop tick 101): isolation-retry unmasks a cross-file UAF as an artifact; land Range.createContextualFragment (+33)

Three things: (1) self-audit (due at 101, clean); (2) capability Range.createContextualFragment
(HTML string -> DocumentFragment, +33 domparsing, verified in isolation); (3) the instrument fix
that unblocked it.

The sweep uncovered a real Bar-0 SIGSEGV (exit 139) in css-flexbox/stretched-child-shrink-on-
relayout.html: cross-file heap corruption (a dangling reflector / unrooted *mut JSObject) that
trips only when many pages share one reused SpiderMonkey runtime, but is CLEAN in a fresh runtime.
It is a Heisenbug (vanishes under gdb; needs ASAN to fix) and blocks the whole sweep.

Isolation-retry: when a batch child dies by signal, re-run the culprit ALONE; if it passes in a
fresh runtime, record its true per-page result as a distinct ACCUM metric, NOT a per-page Bar-0
crash. A file that crashes ALONE too stays CRASH -- a real per-page crash is never reclassified.
This measures the engine as it is actually used (one document per fresh context) while keeping the
UAF loud and tracked as an OPEN Bar-0 to fix with ASAN.

MEASURED (full sweep): crashes=0 (flexbox recovered to true 220/3594, grid 150, ACCUM surfaced);
domparsing 149->182; TOTAL 389035->389069. Gate G_RANGE extended (createContextualFragment),
proven falsifiable. Underlying UAF documented for a fresh-context ASAN fix.

WIKI: docs/wiki/conformance-and-oracles.md (isolation-retry) + js-engine.md (the reused-runtime UAF)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
