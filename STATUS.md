# manuk — STATUS

> **Read this first, every session, before anything else.** State the tier and any blocking items
> out loud before touching code. Do not proceed on assumed context from a previous session.
>
> **This file is GENERATED (`scripts/status-update.sh`), not hand-written.** A status file someone
> writes prose into starts describing what we *meant* to do. Every field below is read from the
> filesystem, git, the crawl output or the verify receipt.

```
TICK:              20
LAST_AUDIT_TICK:   19          (self-audit due every 10 ticks — the hook BLOCKS commits past that)
CURRENT_TIER:      0                     (Part 21 — one Tier-0 item left: the SPA miner)
LAST_WALL_TIME:    85s
ORACLE_CORPUS:     265 sites
ORACLE_CRAWLED:    265 sites, 379 clusters  → docs/loop/CLUSTERS.md
ORACLE_HANGS:      84                    ← Bar 0. Outranks every visual cluster (Part 24.3).
PENDING_GATES:     G_SILENT_FAIL G_DEDUP G_SPAWN G_POOL_ISOLATION
SINGLE_SITE_TICKS: 0                    (this audit window — a rising count is the drift signal)
UPDATED:           2026-07-12
```


## Settled Decisions — closed questions. Do not relitigate. (Part 29.2)

Re-deriving a decision that was already correctly made is the most expensive kind of drift: it
consumes real reasoning effort and *feels like progress* while producing no new ground truth.

- **Bar 2 (pixel precision) is deferred.** Breadth beats depth until Bar 1 is real. Pixel-exact on one
  site and broken on a thousand others is not what "usable" means.
- **Bar 0 (no crash, no hang, no unrecoverable panic) is the FLOOR**, checked before Bar 1 is even
  asked, for any pattern class. (Part 23.)
- **Stylo and SpiderMonkey are never patched internally.** Sanctioned FFI dependencies only.
- **No Blink/Gecko code is copied, ever, under any framing.** Algorithm extraction only, cited by
  reference. This one stays discipline, not a hook: a script cannot tell "extraction" from "close
  paraphrase" (Part 28.4), and pretending it could would be worse than naming it.
- **The oracle's cluster ranking IS the priority ledger** (`docs/loop/CLUSTERS.md`) — not a suggestion
  judgment may override outside of tie-breaking.
- **Crashes and hangs outrank every visual divergence** in that ledger (Part 24.3).

## Lessons — promoted out of the journal because they recurred (Part 29.1)

Short by construction. If this grows without bound, lessons are being added that should have become
**gates** instead — which is always the better outcome, and is where most of these ended up.

1. **A gate that does not measure what the user feels reports green while the user suffers.** Every
   gate here was born this way: G_ALLOC (the wheel-event clone freeze), G_LOAD (the frozen tab),
   G_INTERACT, G_CONTAIN (the apple.com core dump). Before adding a feature, name the gate that would
   have gone red if it were already broken. If you cannot, add it first.
2. **The symptom names the wrong organ.** rust-lang.org's columns *looked* stacked — they were in a
   perfect row, overflowing off-screen. The oracle's font-width divergence *looked* like a metrics
   bug — `font-family` was never mapped at all. Measure the boxes before theorising from a screenshot.
3. **When every instrument says a bug is impossible, they are all sampling the same layer.** The reddit
   grey was in no display item, no decoded image, no rect — because it was a *letter*, rasterised from
   an unscaled outline at `font-size: 0`. Bisect the layer below; do not reason harder about the one
   you are in.
4. **An oracle must never be able to charge its own slowness to your account.** One wall-clock budget
   per site cannot tell "we hung" from "Chromium hung", and will confidently tell you the wrong one.
   Time each engine separately, always.
5. **A fix for one instance is not containment of a class.** Fixing apple.com's panic was necessary and
   was not Bar 0. You will not prevent every crash-class bug before Bar 1 — the tail of uncovered
   patterns is where the panics live, and it is infinite.

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

## Bar 0 — the stability floor (Part 23). Checked BEFORE Bar 1 is even asked.

| Requirement | Status |
|---|---|
| No unrecoverable Rust panic at process level | ✅ **contained** — `panic = "unwind"` + a supervised per-navigation boundary. A panic in parse/cascade/layout/paint kills the PAGE and shows an error; the browser and every other tab carry on. Proven by `G_CONTAIN`, which deliberately panics a build. |
| No SpiderMonkey crash cascade | ⚠️ **partial** — caught at the six binding sites; a fault raised inside SpiderMonkey's own C++ frames still cannot be caught in-process (unwinding across that FFI edge is UB). Full containment needs a per-tab process, correctly deferred. Stated honestly rather than claimed. |
| No unrecoverable hang | ⚠️ **73/265 sites hang.** G_HANG counts them; production interruptibility is not yet built. |

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
| **G_CONTAIN** | ✅ **live** | Bar 0 — a panic kills the page, not the process (Part 23.2) |
| **G_RUNTIME_COUNT** | ✅ **live** | one async runtime for the process, not one per action (Part 25.2). The shell was building **two**. |
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
