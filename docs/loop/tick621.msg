measure(map): two of the map's `works` claims were false, and one was an inert stub

TICK SHAPE: measurement. Bar 0 untouched; no ratchet floor moved; NO ENGINE SOURCE CHANGED.

Audit #34 moved 14 rows to `unknown` because nothing measured them. This is the follow-through, and the
board's CO-#1 item (2) — probe the genuinely-open unknowns. Five measured in one probe:

  constructable stylesheets   new CSSStyleSheet() ok · replaceSync fn · adoptedStyleSheets=[] ok  -> WORKS
  forced reflow               40 -> 140  (a mid-script write is visible to the NEXT read)         -> WORKS
  list markers                li.left 48 vs ul.left 8 — 40px of indent, Chrome's default          -> WORKS
  overflow-anchor             getComputedStyle(...).overflowAnchor is UNDEFINED                   -> MISSING
  ResizeObserver              constructs · observe() accepts · callback NEVER CALLED              -> INERT

THE TWO THAT WERE FALSE ARE WHY THE AUDIT WAS WORTH RUNNING. Both had `works` in the map with NO gate.
overflow-anchor is not partially implemented — the property is not computed at all. And ResizeObserver is
the G_MUTATION shape exactly: an object that constructs, accepts .observe(), and calls your callback
never. "THE GLOBAL EXISTS" IS NOT "THE OBSERVER FIRES" — and only a probe that mutates the observed
element and then waits for delivery can tell them apart. The first version of my own probe reported
fired:0 SYNCHRONOUSLY, which proves nothing, since ResizeObserver is asynchronous by design; it took
mutating the box and draining to make the zero mean something.

ResizeObserver is pinned at `partial`, not `missing`: the surface is real and only DELIVERY is absent,
and `missing` would be a new inaccuracy in the other direction.

AND THE RECONCILER CAUGHT ME REINTRODUCING THE EXACT DEFECT I HAD JUST SPENT A TICK REMOVING. `partial`
with gate='-' is a bare assertion — unfalsifiable, which is what audit #34 was about. So the negative is
now PINNED as an assertion: roCtor:function + roFired:0 live in the gate. ⚠ That assertion is a NEGATIVE
and it is the one that rots — the moment delivery lands it goes RED, and that is the signal to re-price
the map row, NOT to retune the line.

RED-PROVEN: making adoptedStyleSheets getter-only again (both registration sites) -> RED
`adoptSet:TypeError` — the throw, not a no-op, which is the t612 class. Map drift back to 0 and
G_CONSTELLATION_WELLFORMED green on the result.

Gate: G_UNKNOWNS_PINNED (engine/page/tests/g_unknowns_pinned.rs) — constructable stylesheets end to end,
forced reflow, marker indent, and the ResizeObserver negative.
WIKI: none [forced] — no engine mechanism changed; the findings' home is the map's receipts.
PATTERN: [no-pattern] — no browser capability changed.

NEXT: ResizeObserver DELIVERY is now a named, measured gap with a gate that flips when it lands — worth
more than its row suggests, since IntersectionObserver already fires (t59) and every responsive component
library reaches for RO. The remaining audit-#34 `unknown` rows (font-display/unicode-range, ic/ric units,
test262) are each a probe of this shape.
