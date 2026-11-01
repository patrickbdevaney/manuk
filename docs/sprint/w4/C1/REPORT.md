# W4.C1: engine-exact settle, measured (3 arms)

chrome_ver=145.0.7632.116; harness `bench/agent-tasks/c1_settle.py` (no LLM; raw MCP stdio; 3 runs per cell); data `settle.tsv`.
Each run: navigate, click `#go`, read the act's own returned observation. "content" = the fixture's token is in that
observation. Arms: manuk (`manuk-mcp`), cdp-layer (`manuk-mcp --backend chrome`, same layer code), playwright
(`@playwright/mcp@0.0.83`, `browser_click` with `target=<ref>`; see the addendum for the earlier `ref` error).

| fixture (content lands after click) | manuk content / act wall ms [band] | cdp-layer | playwright |
|---|---|---|---|
| late-fetch-1400 (fetch 1.4 s) | 3/3, 1406 [1406-1406] | 3/3, 1784 [1783-1785] | 0/3, 1991 [1984-2000] |
| fetch-then-timer (400 ms fetch, 1.5 s timer) | 3/3, 406 [406-406] | 3/3, 2305 [2301-2307] | 0/3, 1096 [1093-1100] |
| chained-timers (3 x 400 ms) | 3/3, 4 [4-5] | 3/3, 1577 [1577-1578] | 0/3, 585 [575-598] |
| date-skew (chained timers; content only if `Date.now()` advanced >= 1 s) | 3/3, 3 [3-3] | 3/3, 1601 [1596-1607] | 0/3, 588 [586-593] |
| font-css (@font-face + document.fonts.load, 900 ms) | 3/3, 4 | 0/3, 851 | 0/3, 1498 [1493-1505] |
| font-swap (new FontFace().load()) | 0/3, 3 | 0/3, 850 | 0/3, 1503 [1491-1510] |
| microtask-100k | 3/3, 897 [878-922] | 3/3, 856 [855-857] | 0/3, 592 [585-598] |
| microtask-storm (3M, first fixture version, kept) | 0/3, 8448 [8369-8594] | 3/3, 948 [945-955] | 0/3, 679 [669-692] |
| never-10s (control: must not wait) | absent, 3 | absent, 848 | absent, 584 [580-587] |

## Reading (only what the numbers support)
- Gate: the 1.4 s fixture is read correctly by the engine arm in 3/3 (also cdp-layer 3/3; playwright 0/3). Mutation
  `MANUK_C1_MUTATE=late` (content lands at 30 s) makes the gate print `FAIL` (0/3, exit 1).
- **Correct reads on late content are a LAYER win, not an engine win.** cdp-layer matches manuk on the three timing
  fixtures (3/3 each); both beat Playwright (its click result lacks the content in 0/3 on every content fixture).
- **Wall time per act is an engine difference on timer-bound pages**: manuk 4 ms vs cdp-layer 1577 ms (chained timers),
  406 vs 2305 ms (fetch+timer), outside the 3-run band (bands are 1-5 ms wide). Mechanism: the manuk engine advances its
  own timer clock instead of sleeping (visible as 4 ms for 1.2 s of timers); not yet checked for page-visible side
  effects on `Date.now()`/animations. Where the wait is real network (late-fetch) the gap is 1406 vs 1784 ms
  (cdp-layer has an ~850 ms floor even on the never-10s control).
- **Engine regressions found**: `new FontFace().load()` does not land content (0/3, a missing-API no-op: the act says "no change");
  a 3M-microtask chain fails in 8.4 s on manuk while Chrome does it in 0.95 s. (font-css reports 3/3 but its wait is
  likely not a real font wait: manuk resolves in 4 ms, so treat it as unproven.)
- Turns per task (not isolated to settle, from committed REAL data `docs/sprint/w4/A6-real.tsv`): manuk 31.8 (n=141),
  cdp-layer 43.1 (n=141), playwright 36.0 (n=47, one run). Pass: 32/141, 49/141, 25/47. Own suite turns/task
  (docs/sprint/w4/B4): manuk 5.38, cdp-layer 4.58, playwright 6.27. These do not attribute any turn difference to settle.

## Verdict
Claim the settle *correctness* win as LAYER (cdp-layer ties). Claim the *wall-time* win on timer-bound acts as ENGINE,
for fixtures with timers only, pending the Date.now check (follow-up W4.C1.a). No claim on turns per task.

## Addendum (W4.C2.d): playwright arm re-run with a valid click
- The first run called `browser_click` with `ref`. `@playwright/mcp@0.0.83` requires `target` (its README lists
  `target`, not `ref`), so every playwright click was a rejected call; its "0/3, 1 ms" cells measured the error, not
  Playwright. The playwright cells above come from a re-run with `target=<ref>`, and `c1_settle.py` now refuses to
  record a playwright error response (`tool_full` checks `isError`).
- Playwright's click takes 0.6-2.0 s (cause not measured) and still returns without the content
  on all 8 content fixtures (0/3 each). The control stays absent 3/3. So the old claim that browser_click "returns at
  once" was wrong and is removed; the 0/3 result stands.
- The `date-skew` fixture was added to the script after the first run, so its rows are new for all three arms. It
  reads the content in 3/3 on manuk (the page's `Date.now()` saw >= 1 s of chained timers) and in 3/3 on cdp-layer.
  This meets the W4.C1.a Date.now gate in this run (3/3, same chrome_ver); the verdict above is unchanged.
- Data: `settle.tsv`, 81 rows (9 fixtures x 3 arms x 3 runs), all `chrome_ver=145.0.7632.116`. Manuk and cdp-layer
  rows for the eight original fixtures are unchanged from the first run; only the playwright rows were re-measured.
- Harness changes: `target` for the click, the `isError` guard, and `MANUK_C1_ARMS` to run a subset of arms.
