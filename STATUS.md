# manuk — STATUS

> **Read this first, every session, before anything else.** State the tier and any blocking items
> out loud before touching code. Do not proceed on assumed context from a previous session.
>
> This file holds *checkable facts*, not narrative. Every line is something that was measured, with
> the command that measured it. If a line here is stale, that is a compliance failure in itself.

```
TICK: 18
TIER: 0 (Part 21) — ONE of three Tier-0 items still open (the SPA miner)
UPDATED: 2026-07-12
```

## Tier 0 — nothing in the backlog starts until these are done or genuinely blocked

| # | Item | Status | The fact |
|---|------|--------|----------|
| 1 | Verify wall under 5 minutes | ✅ **MET** | **181s (3m 01s)** worst realistic tick (touch `engine/css`, the shared-type edit that cascades furthest); **57s** warm. Measured by timing `./scripts/verify.sh`. mold/nextest/workspace-hack **not needed** — the target is already met, and doing that work anyway would be infrastructure theatre. Re-measure if it ever crosses 300s. |
| 2 | Oracle crawl frame at 200–500 sites | ✅ **DONE** | **265 sites, 15 design-pattern classes** (`docs/bench/oracle-corpus.txt`). `scripts/oracle-crawl.sh` — process-isolated, watchdogged, resumable, snapshot-cached. First run: 129 diffed, 63 discarded (degraded oracle), **73 HUNG**. The cluster ranking is now the ledger. |
| 3 | Ten SPA starter apps through the Framework Exception Miner | ❌ **OPEN** | **0 apps run.** This is the single largest *unmeasured* unknown in the whole schedule and it is cheap to measure. Not started. |

**Definition of a productive session while TIER 0 is open: Tier 0 advanced.** Not bugs closed, not
features shipped. If I find myself reaching for a bug fix because it feels like more visible progress
than widening the oracle, that is the exact failure mode Part 21 exists to prevent, and I say so out
loud rather than quietly following the pull.

## Gates — standing up vs. pending

| Gate | Status | What it catches |
|------|--------|-----------------|
| build · parity 72/72 · G1 · G2 · G3 · G6 | ✅ in the wall | rendering + JS + affordances + clickability |
| G_ALLOC | ✅ in the wall | per-input-event allocation rate |
| G_TEARDOWN | ✅ in the wall | an exit path that skips the profile flush |
| G_LOAD | ✅ in the wall | a dead subresource holding the document hostage |
| G_INTERACT | ✅ in the wall | UI-thread stall on tab open/switch/close |
| F1 / F2 perf floors | ✅ in the wall | cascade ≤40ms, pipeline ≤125ms (asserted, not eyeballed) |
| **G_SILENT_FAIL** | ❌ **pending** | any caught error on the load/render/script path that is not surfaced |
| **G_HANG** | ✅ **live** | every oracle site runs in its own process under a watchdog. A timeout is a HARD, COUNTED, ATTRIBUTED failure — never a skipped test. **73/265 sites currently hang.** |
| **G_SPAWN / G_DEDUP / G_POOL_ISOLATION** | ❌ **pending** | tokio/rayon isolation, duplicate passes, pool contention |

## Enforcement — compliance is mechanical, not remembered

| Mechanism | Status | How it works |
|-----------|--------|--------------|
| Gate receipt | ✅ live | `verify.sh` writes `.git/manuk-verify-receipt` naming the **exact tree** it verified. `scripts/hooks/pre-commit` recomputes that name from what is being staged and **refuses the commit if they differ**. Verifying one version of the diff and committing another is now impossible, not merely discouraged. |
| Journal enforcement | ✅ live | The same hook refuses any commit unless `docs/loop/JOURNAL.md` has a `## Tick <N>` entry for the `TICK:` in this file. The entry is written at the *start* of a tick, so it states a hypothesis rather than narrating a success. |
| Session-start read | ✅ live | `CLAUDE.md` makes reading this file the literal first action of every session. |
| Self-audit every 10 ticks | ✅ live | `scripts/self-audit.sh` diffs what the methodology prescribes against what actually exists, and fails loudly on anything prescribed-but-not-executed. Due at tick 20. |

## Last 5 journal entries

- **Tick 15** — the invisible-content class: `font-size:0` painted glyph-shaped continents (swash rasterizes the *unscaled* outline at 0px); anonymous boxes stranded in stacking layer 0, burying text under its own ancestor's background; every insetless `position:absolute` element silently deleted (github coverage 91.4→97.8%); backgrounds stretched to their element. New gate G_INTERACT.
- **Tick 14** — the oracle pays for itself: `font-family` was never mapped from the cascade *at all*; the network layer had no timeout of any kind (w3schools 37.8s→15.0s); flex items could never shrink; a percentage width on a flex item resolved **twice** (used width came out squared); every responsive image rendered stretched.
- **Tick 13** — headless screenshot discipline; flex items with block children.
- **Tick 12** — in-process automation surface (selectors/wait/assert).
- **Tick 11** — file uploads.

## THE NUMBER THAT MATTERS RIGHT NOW

```
73 of 265 sites HANG  (27.5%)     ← a browser that hangs on one site in four is not a browser
```

Attributed, not guessed: same snapshot, each engine timed separately. bbc.co.uk **26,128ms** vs
Chromium's 7,695ms. apple.com **5,560ms** vs 287ms (19×). It is not the network and it is not the
oracle — **it is us, and it is CPU and duplicate work.** Per navigation, measured:

```
bbc.co.uk:  9 full-document LAYOUTS · 4 full CASCADES · 487 fetches (302 DUPLICATE)
```

Part 22.3 asked whether we do duplicate work in the call graph. We do, enormously. This is the top of
the ledger and the next thing after the SPA miner.

## Corpus (18 sites — the OLD frame, kept for per-site fidelity scores)

```
MEAN COVERAGE  99.0%   (Bar 1 — of what Chrome renders, what do we render at all)
MEAN VISUAL    81.1%   (Bar 2 — DEFERRED, per Part 21.2 item 5; do not micro-tune this)
```

Bar 2 stays deferred. A browser that is pixel-exact on one site and broken on a thousand others is
not what "usable" means here.
