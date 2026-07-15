feat(loop tick 112): lang reflection -- getter-only fallback gains a setter (+4560)

html/dom's getAttribute-value-mismatch cluster ("got test-valueOf") was reflection value
correctness. Isolated bug: el.lang had a getter (generic attribute fallback) but NO setter, so
el.lang = x was silently dropped. lang is neither a named native accessor nor in the per-tag table,
so reflect_js never installed a proper accessor. Fix: add lang to the "*" global row -> reflect_js
installs a real getter+setter. Gate G_GLOBAL_REFLECT extended (setter reflects).

Bar-0 caught + reverted mid-tick: the first attempt also added title/enterKeyHint/inputMode and the
sweep showed css-grid crashes=35. Reverted; bisected to title (a native accessor exists; defining a
reflected one over it crashed). Kept only lang (crash-free).

MEASURED (full sweep): html/dom 40935 -> 45495 (+4560), TOTAL 407882 -> 412442, crashes=0, every
other area held.

WIKI: docs/wiki/dom-semantics.md -- "A getter-only attribute fallback silently drops the setter"

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
