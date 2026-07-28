measurement(render): the honest render-failed remainder is the hydration wipe — and I nearly reported the printer's cap as a parser bug

t709 cleared the bot walls out of render-failed, leaving theverge 928KB / vox 963KB / mongodb 956KB /
kotlinlang 250KB / notion 417KB — all of which receive the REAL document and still render blank.

THE ANSWER: IT IS THE SPA HYDRATION WIPE, ALREADY ON THE BOARD UNDER ANOTHER NAME. kotlinlang.org is
Next.js SSR; the served bytes carry a fully server-rendered tree (div#__next, layout_wrapper, header,
220 <div>s). After load:

  html [0 0 1200x0]  body [0 0 1200x0] elem_kids=3
    div#__next [0 0 1200x0] elem_kids=0     <- 220 divs of served markup, gone

Parsed WITHOUT scripts the same bytes give the whole tree. The document arrives, parses, and is then
emptied by its own client render — exactly C3833 MISSING BOX: <div> (32 sites, 7544 hits, top cluster
by hits), diagnosed at t696 on wix.com: innerHTML="" inside the load event deletes 4917 elements and
the re-render performs ZERO DOM operations.

So render-failed and C3833 are ONE failure seen by TWO instruments — the pixel side ("we drew
nothing") and the DOM side ("the boxes are missing"). They have been ranked and worked as separate
items. The honest remainder of render-failed is not a new cluster; it is more evidence for the biggest
one already open.

AND THE INSTRUMENT CAPPED, AND I READ THE CAP AS DATA. En route I measured "the parser stops after 32
<div>s" — stable at 32 across 20KB, 60KB and 120KB prefixes while the file's own count rose 40 -> 130
-> 195. A hard stop at a constant, immune to input size, is a beautiful bug. It is not a bug:

  (showing 32 of 195 matches — TRUNCATED)

WHY_CAP = 32 in the --why printer, and I had counted "^── match" lines with grep -c, which discards
the very line the tool prints to stop this happening. The comment above that cap says "a probe that
silently truncates its own answer is how a partial list gets read as a complete one". It does not
truncate silently — I silenced it. grep -c over a tool's output discards that tool's own error and
truncation reporting.

No code change, so no gates. CLUSTER: render-failed CONSOLIDATED into C3833 rather than worked as its
own population — no new cluster, one fewer.

ALSO THIS TICK — WALL AUDIT #22, WHICH RETRACTS TICK 710's. The section table says P (parity) is 172s
= 71% of a 242s wall. Measured ALONE on a quiet box (load 0.27), same tree, same fixtures:

  $ manuk-wpt parity   ->  TOTAL: 72/72 probes across 30 pages   elapsed: 3s

Three seconds against a 172-second line item. There is no parity optimisation to make: the 71% is
CONTENTION — the gate sharing 32 cores with ~25 concurrent gate builds — and the audit instrument
reports a wall-clock slice of a parallel wall as a line item's cost.

This is the second time, and the first time it was already written down. Audit #21 (t689) read P at
175s and blamed 72 serial Chrome spawns; the t694 CORRECTION measured 14s serial / 4s parallel and
added an explicit step 0: before optimising a line item, measure that line item ALONE on a quiet box.
Tick 710's wall note repeated the retracted claim verbatim because it read the section table and
skipped step 0 — with the correction sitting three headers above the one I appended to. A ledger only
protects you if you read the entry above yours before adding the next one.

The wall is LEAN and there is nothing admissible to trim, which Audit #21's own rules call a fine
result.

WIKI: none [forced] — a diagnosis that reclassifies an existing cluster; the engine is unchanged.
[no-pattern]
