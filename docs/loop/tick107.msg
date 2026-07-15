feat(loop tick 107): element.getClientRects() (correct missing API, ratchet-neutral)

Implemented element.getClientRects() -- a missing CSSOM-View API used broadly to measure geometry.
Returns a DOMRectList from the layout snapshot (like getBoundingClientRect): a laid-out element ->
one rect; display:none -> empty list (never a zero rect). Includes .item(i) + indexed access. Gate
G_CLIENT_RECTS, proven falsifiable.

Ratchet-neutral (full sweep TOTAL 389478 unchanged, crashes=0): the lone getClientRects reference
is in a multi-assertion css-position test that fails on other geometry too. Landed tick-97/102-style:
correct, additive, zero-regression capability real sites call constantly.

Strategic confirmation: 8 areas probed (ticks 99-107); the clean single-mechanism FLIP wins are
harvested. Remaining is diffuse (layout/computed-value precision) or deep subsystems (Typed OM, Web
Animations, CSSOM .sheet) or the Bar-0 unblocks -- none a single bounded flip. Next progress needs
a deep subsystem (fresh context) or the Bar-0 fixes (debug build / worker-thread / ASAN).

WIKI: docs/wiki/interaction-surface.md -- "element.getClientRects() reuses the layout snapshot"

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
