# RESUME — where the loop is

**Last tick: 32.** All gates green (`scripts/verify.sh --fast`).

## The map

**[PARITY-LEDGER.md](../../PARITY-LEDGER.md) is now the work queue.** It is the complete,
honest inventory of the web platform with Manuk's true status against each part, prioritised by
*measured blast radius on real sites*. The loop selects from its "Selection order" until it is empty.

## Scoreboard (vs headless Chromium, shipping config)

| metric | now |
|---|---|
| COVERAGE — of the elements Chrome renders, the fraction Manuk renders at all | **99.7%** |
| VISUAL — coarse block-grid agreement | **~89.7%** |
| PLACEMENT — median dy, Wikipedia | **1,087px** (was 5,226) |
| box parity (synthetic corpus) | **72/72** |

## What this session established, that must not be re-learned

1. **THE SCORE GATES; THE EYEBALL DIAGNOSES — but only a MEASUREMENT names the box.**
   Hours went into staring at a stacked header. Ten minutes went into `boxes --tree`, which
   printed `label.cdx-button <InlineFlex> [44 17 236×32]` and ended the argument. Build the probe
   *first*.

2. **When a metric will not move, suspect the metric.** Wikipedia's median dy sat at exactly
   5,122px across four real fixes. Chrome's screenshot and Chrome's box probe were rendering
   *different pages*; `node_rects` was unioning overflow into every ancestor; and the site was
   serving us a degraded document. None of those were the engine.

3. **The web feature-detects and GRADES the browser.** `'localStorage' in window` is not a
   feature check, it is an admissions test — MediaWiki fails us out of it and ships the no-script
   page. Look for the gate before blaming the layout.

4. **A workaround that hides a crash is a data-loss bug wearing a disguise.** The shell's
   `libc::_exit()` skipped SpiderMonkey's exit segfault by skipping *every* exit handler — which
   is where a browser flushes the user's profile.

5. **ADR-011 keeps re-earning itself.** Every gate must run the shipping configuration. It has
   caught us three times now: MinimalCascade vs Stylo, then no-JS vs SpiderMonkey, and DEBT-4
   (below) is the same trap still open.

## Useful tools built this session

- `manuk-wpt boxes --html F [--url U] [--tree ID]` — Manuk's rect for every `[id]`, or the layout
  box subtree with each box's **computed display**. This is the tool that finds the wrong box.
- `MANUK_TRACE_INTRINSIC=<id>` — what a box told taffy it wanted to be. Flex wrapping is decided
  by that number and it is otherwise invisible.
- `manuk-wpt fidelity` now reports PLACEMENT (median dx/dy/dw/dh) and FIRST DIVERGENCE (the first
  element down the page that breaks agreement).

## Open debts

- **DEBT-2** — no rule index on the **Stylo** (shipping) cascade. EPOCH-1 indexed the one users
  don't run. **P0.**
- **DEBT-3** — shell chrome cannot be painted headlessly, so AESTHETICS/ERGONOMICS are unprobeable.
- **DEBT-4** — dynamic scripts run on `load_async` but not on the shell's prefetch nav path.
  **The ADR-011 trap, still open. P0.**

## Next

Ledger "Selection order" items 5 onward: scroll **events** + Intersection/ResizeObserver (real-time
feeds), `fetch`/XHR + URL/FormData (the SPA data path), transitions & animations, then canvas/svg/
iframe/video, then the security half (CORS, CSP).

Next epoch: earliest Tick 30 has passed — **an EPOCH audit is due**; drift is high (a great deal of
new capability, little new perf/stability work beyond the two crash fixes).
