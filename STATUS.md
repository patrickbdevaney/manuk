# manuk — STATUS

> **Read this first, every session, before anything else.** State the tier and any blocking items
> out loud before touching code. Do not proceed on assumed context from a previous session.
>
> **This file is GENERATED (`scripts/status-update.sh`), not hand-written.** A status file someone
> writes prose into starts describing what we *meant* to do. Every field below is read from the
> filesystem, git, the crawl output or the verify receipt.

```
TICK:              37
LAST_AUDIT_TICK:   29          (self-audit due every 10 ticks — the hook BLOCKS commits past that)
CURRENT_TIER:      0                     (Part 21 — one Tier-0 item left: the SPA miner)
LAST_WALL_TIME:    253s
ORACLE_CORPUS:     265 sites
ORACLE_CRAWLED:    265 sites, 640 clusters  → docs/loop/CLUSTERS.md
ORACLE_HANGS:      4   ← Bar 0, on OUR clock (manuk_ms > 30s). Outranks every visual cluster.
ORACLE_UNATTRIB:   13   ← oracle process hit its watchdog. Whose time? UNKNOWN — never ours by default.
PENDING_GATES:     G_SPAWN G_POOL_ISOLATION
SINGLE_SITE_TICKS: 0                    (this audit window — a rising count is the drift signal)
UPDATED:           2026-07-13
```


## THE NORTH STAR — one sentence, and it decides what "done" means

> **Chromium is the CEILING on capability, and the FLOOR on everything else.**

- **Capability — MATCH it.** Whatever a page can do in Chrome, it must be able to do here: the scripts
  run, the layout resolves, the forms submit, the embeds render. This is the only axis where Chromium is
  the *target*, and on this axis we are behind and must catch up.
- **Performance, stability, resource use, honesty of failure — EXCEED it.** On these, Chromium is the
  *baseline to beat*, not the number to converge on. Being faster is not a divergence. There is nothing
  to regress toward.

This resolves the question the oracle's diff can never answer on its own, and it resolves it *in advance*
rather than case by case: **a structural divergence is a bug; a timing divergence in our favour is the
point.** The oracle diffs structure. It has never scored timing, and it must not start.

**The trap, and it is the one this project keeps catching itself in.** A speed advantage is only real if
it comes from doing the same work *better* — dedup, caching, single-flight, deferring what the author
said to defer — and **not from not doing the work at all.** *"Fast because we never loaded the images"*
and *"fast because we never ran the script"* are two lies already told and caught here; `G_FIRST_PAINT`
and `G_DEFER` exist for exactly that reason, because a speed number achieved by **skipping a capability**
is indistinguishable, on the clock, from one achieved by an optimisation.

So the mechanical form: **a speed claim is only admissible next to a coverage number**, and
`scripts/crawl-report.sh` prints coverage FIRST and has no flag to print speed alone. If coverage holds
and we are faster, we are faster. If coverage moved, the speed is a measurement of the thing we stopped
doing.

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
- **SpiderMonkey is settled** (Part 30). The "V8 is more capable" intuition does not survive contact
  with the evidence — sites broken on Firefox are overwhelmingly browser-sniffing, not conformance.
  And the capability bar (Chromium parity) rules out the lean/embedded tier — QuickJS, Hermes,
  JerryScript — **entirely**, not just against V8. Leanness that costs capability is not a trade this
  project can make. Do not reopen without evidence the BAR changed.
- **Chromium is the CEILING on capability, the FLOOR on everything else** — see THE NORTH STAR above.
  Match its capability; beat it on speed, stability and resource use. A timing divergence in our favour is
  not a bug to close. Do not reopen this.

- **The app web is ADDITIVE substrate, not a scheduling subsystem** (measured, tick 20: 0/8 → 3/8
  frameworks rendering from ~6 IDL fixes). This was the open question the whole schedule hung on.

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

   **This one has now fired three times, and the third time it had produced the project's top-line
   metric.** "73/265 sites hang" was the oracle *process* hitting a watchdog that wraps Chromium — the
   slower engine on most news sites. The general form, which is the version to keep:

   > **Every number has a harness, and the harness is part of the number.** Before believing a metric
   > moved, ask what else moved. I widened the crawl from 4 jobs to 12 to make it finish sooner and
   > watched "the hang rate" go from 12.5% to 49% on the same binary in the same hour.

   It is a lesson I could recite while breaking it, which means it was decoration. It is four mechanisms
   now: `TIMEOUT` is attributed to nobody · Bar 0 counts `manuk_ms` · the crawl warns at a non-baseline
   job count · `status-update.sh` refuses to print a partial crawl as a number.
5. **A fix for one instance is not containment of a class.** Fixing apple.com's panic was necessary and
   was not Bar 0. You will not prevent every crash-class bug before Bar 1 — the tail of uncovered
   patterns is where the panics live, and it is infinite.

## Tier 0 — nothing in the backlog starts until these are done or genuinely blocked

| # | Item | Status | The fact |
|---|------|--------|----------|
| 1 | Verify wall under 5 minutes | ✅ **MET** | **181s (3m 01s)** worst realistic tick (touch `engine/css`, the shared-type edit that cascades furthest); **57s** warm. Measured by timing `./scripts/verify.sh`. mold/nextest/workspace-hack **not needed** — the target is already met, and doing that work anyway would be infrastructure theatre. Re-measure if it ever crosses 300s. |
| 2 | Oracle crawl frame at 200–500 sites | ✅ **DONE** | **265 sites, 15 design-pattern classes** (`docs/bench/oracle-corpus.txt`). Process-isolated, watchdogged, resumable, snapshot-cached, **run-stamped**. Latest clean run: 206 diffed, 44 discarded, 15 unattributed timeouts. |
| 3 | Ten SPA starter apps through the Framework Exception Miner | ✅ **DONE (tick 26)** | **8 of 8 frameworks mount and render** — React (TS+JS), Vue, Svelte, Solid, Preact, Lit, Vanilla. Every blocker was one of *our* primitives, not the framework: a use-after-GC in `ownerDocument`, `file://` unsupported by the net layer, `CharacterData.data` missing, a shadow root typed 8 instead of 11, and no accessors on `Node.prototype`. All six are now asserted in **G2 scenario 14**. |

**TIER 0 IS COMPLETE.** The definition of a productive session is no longer "Tier 0 advanced". The
schedule is now set by the ledger and by Bar 0's residue, which is **two sites** (wix.com, flickr.com).

**Definition of a productive session while TIER 0 is open: Tier 0 advanced.** Not bugs closed, not
features shipped. If I find myself reaching for a bug fix because it feels like more visible progress
than widening the oracle, that is the exact failure mode Part 21 exists to prevent, and I say so out
loud rather than quietly following the pull.

## Bar 0 — the stability floor (Part 23). Checked BEFORE Bar 1 is even asked.

| Requirement | Status |
|---|---|
| No unrecoverable Rust panic at process level | ✅ **contained** — `panic = "unwind"` + a supervised per-navigation boundary. A panic in parse/cascade/layout/paint kills the PAGE and shows an error; the browser and every other tab carry on. Proven by `G_CONTAIN`, which deliberately panics a build. |
| No SpiderMonkey crash cascade | ⚠️ **partial** — caught at the six binding sites; a fault raised inside SpiderMonkey's own C++ frames still cannot be caught in-process (unwinding across that FFI edge is UB). Full containment needs a per-tab process, correctly deferred. Stated honestly rather than claimed. |
| No unrecoverable hang | ✅ **9 of 206 sites exceed 30s on our clock (4.4%)** — and Chromium is slower still on 7 of those 9. We are **faster than Chromium on 175/206 (84%)**; median render 21.7s vs its 35.7s. The old "73/265 hang (27.5%)" was the oracle *process* hitting a watchdog that wraps Chromium too. **Remaining: two sites** (wix.com 39.1s vs 22.4s; flickr.com 31.1s vs 14.8s). Production interruptibility (a cancellable long task) is still not built. |

## Gates — standing up vs. pending

| Gate | Status | What it catches |
|------|--------|-----------------|
| build · parity 72/72 · G1 · G2 · G3 · G6 | ✅ in the wall | rendering + JS + affordances + clickability |
| G_ALLOC | ✅ in the wall | per-input-event allocation rate |
| G_TEARDOWN | ✅ in the wall | an exit path that skips the profile flush |
| G_LOAD | ✅ in the wall | a dead subresource holding the document hostage |
| G_INTERACT | ✅ in the wall | UI-thread stall on tab open/switch/close |
| F1 / F2 perf floors | ✅ in the wall | cascade ≤40ms, pipeline ≤125ms (asserted, not eyeballed) |
| **G_SILENT_FAIL** | ✅ **live** | an error on the load/render/script path that is swallowed. Named by the failure that cost several ticks: "React mounts, throws nothing, renders nothing" was React throwing *truthfully* inside an async render, with nothing listening. |
| **G_DEDUP** | ✅ **live** | the same URL on the **wire** twice for one navigation (nytimes was pulling one sprite down once per element that named it) |
| **G_HANG** | ✅ **live, and now honest** | Every oracle site runs in its own process under a watchdog. The watchdog is a **backstop against a true infinite loop** — it wraps our render *and Chromium's*, so when it fires it is recorded as `TIMEOUT` and **attributed to nobody**. The Bar 0 hang count comes from `manuk_ms`. A metric that cannot say whose time it measured must not name a culprit. |
| **G_CONTAIN** | ✅ **live** | Bar 0 — a panic kills the page, not the process (Part 23.2) |
| **G_RUNTIME_COUNT** | ✅ **live** | one async runtime for the process, not one per action (Part 25.2). The shell was building **two**. |
| **G_SPAWN / G_POOL_ISOLATION** | ⏹ **retired, with a reason** | G_SPAWN is subsumed by G_RUNTIME_COUNT; G_POOL_ISOLATION guards a rayon pool that **does not exist**. A gate on absent machinery passes forever and is counted as coverage — which is the definition of vacuous. Saying so beats building theatre to make an audit green. |
| **FALSIFY** | ✅ **live** | **a gate that cannot go red.** `scripts/falsify.sh` mutation-tests the wall against itself. On its first run it found `G_LOAD` — a *Bar 0* gate — had **never tested the thing it was named for**. |

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

**Clean 265-site crawl, tick-36 binary, one RUN_ID, our own clock.** (`scripts/crawl-report.sh`)

```
BAR 1 — is the node THERE?          92.2%   (162,570 of 176,311 probed)
        ...and `display` agrees     73.0%   ← the next real gap
BAR 2 — geometry (DEFERRED)        123,796  the node exists, SAME SIZE, moved. Not a failure.

BAR 0 — over 30s on our clock       4/211   (1.9%)   was 4.4%
FASTER than Chromium              195/211   (92%)    was 84%
median render        ours 16.1s  ·  Chromium 36.5s
p90                  ours 24.7s  ·  Chromium 99.5s
```

**We are slower than Chromium on exactly one site** (atlassian.com, 34.6s vs 32.2s). Median **2.3×
faster**. That is the north star, measured: *capability approached, performance exceeded.*

**The next Bar 1 target is the 27% `display` disagreement** — 33,825 nodes where we render the node but
Chrome and we disagree about whether it is *shown*. `display: none` vs shown is the likely bulk of it, and
unlike geometry it is a **real** rendering difference: a node we hide that Chrome shows is content the
user cannot see.

> **This report lied on its first run.** It lumped `geometry` into "coverage" and announced **2.8%** for a
> browser that renders fine. The instrument built to stop me trusting bad numbers produced one immediately
> — the fourth time an instrument has done that here (see `docs/loop/PROCESS.md`). None of them get to be
> trusted on sight.

## Corpus (18 sites — the OLD frame, kept for per-site fidelity scores)

```
MEAN COVERAGE  99.0%   (Bar 1 — of what Chrome renders, what do we render at all)
MEAN VISUAL    81.1%   (Bar 2 — DEFERRED, per Part 21.2 item 5; do not micro-tune this)
```

Bar 2 stays deferred. A browser that is pixel-exact on one site and broken on a thousand others is
not what "usable" means here.
