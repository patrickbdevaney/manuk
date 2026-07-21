# WALL-TIME AUDITS — keep the per-tick wall lean, without cutting a gate

The wall runs every tick, so a needless second is taxed forever. The ratchet's WALL invariant catches
*regression*; this catches *standing bloat*. Cadence: **every 20 ticks**, enforced by `scripts/wall-audit.sh`
and `tick.sh`. The rule is absolute: **report, never delete** — no gate dropped, no floor widened, no check
moved to CI to fake a fast local wall. Only optimisations that buy the same assertion for fewer seconds.

---

## Audit #1 — tick 93 (wall 61s)

**Where the seconds go:**

| section | s | % | what it is |
|---|---|---|---|
| `T` crate tests | 17 | 28% | `cargo test -p <crate>` across the touched crates |
| `P` parity | 15 | 25% | 72/72 box probes vs headless Chrome (launches a browser) |
| `G6` interaction/hittest | 6 | 10% | UI-thread cost + click hit-testing |
| `G1` fidelity | 4 | 7% | cached-snapshot render fidelity |
| `F` perf floors | 2 | 3% | EPOCH-1 F1/F2 (run alone, deliberately) |
| `B` build | 1 | 2% | incremental, output in RAM — already lean |
| everything else | ~0 | — | the ~20 parallel gates, hidden inside the concurrency |

**Finding: the wall is already lean (61s ≪ the 300s self-audit target), and the two costs are honest.**

* **`T` (crate tests, 28%)** — the one admissible lever. Each `cargo test -p` builds and runs a crate's
  test binary separately. **`cargo-nextest`** shares the binary and parallelises test *execution* harder
  than `cargo test`, for the *same* assertions — the self-audit already names it as a Tier-0 speed item.
  Filed as the next wall-lever; not done this tick (it is a toolchain change, its own scoped work).
* **`P` (parity, 25%)** — browser-launch-bound (it starts headless Chrome to diff structure). Trimming it
  means either not launching the browser (loses the differential) or caching its output (staleness risk on
  the exact thing parity exists to catch). **No rigor-preserving cut found; it stays.**
* Nothing is recomputed that a prior step produced; the parallel gates are already concurrent; the perf
  floors are serial *by design* (a benchmark sharing the machine is not a benchmark). No bloat to remove.

**Verdict: no cut this audit — the wall is lean. The standing lever is `nextest` for `T`.**

**Next audit due: tick 113.**

## Audit #2 — tick 113 (wall 51s)

**Where the seconds go:**

| section | s | % | what it is |
|---|---|---|---|
| `P` parity | 17 | 33% | 72/72 box probes vs headless Chrome (launches a browser) |
| `T` crate tests | 16 | 31% | `cargo test -p <crate>` across the touched crates |
| `G6` interaction/hittest | 5 | 10% | UI-thread cost + click hit-testing |
| `G1` fidelity | 4 | 8% | cached-snapshot render fidelity |
| `F` perf floors | 2 | 4% | EPOCH-1 F1/F2 (run alone, deliberately) |
| `F4` / `B` / the ~20 parallel gates | ~2 | ~4% | hidden inside the concurrency |

**Finding: the wall got LEANER since Audit #1 (61s → 51s, −10s) with no gate dropped** — the same section
mix, just faster on today's tree. It sits far under the 65s ratchet ceiling and the 300s self-audit target.

* **`P` (parity, now the top cost, 33%)** — browser-launch-bound; the differential *is* the point, and
  caching its output would stale the exact thing parity exists to catch. **No rigor-preserving cut; it stays.**
* **`T` (crate tests, 31%)** — the one standing lever is unchanged and unclaimed: **`cargo-nextest`** shares
  the test binary and parallelises execution harder than `cargo test` for the *same* assertions. It is a
  toolchain change (its own scoped tick), deliberately not smuggled into a capability tick.
* Nothing is recomputed; the parallel gates are already concurrent; the perf floors are serial by design.
  **No bloat to remove this audit.**

**Verdict: no cut — the wall is lean and improving. Standing lever remains `nextest` for `T`.**

**Next audit due: tick 133.**

## Wall audit @ tick 325 (2026-07-21) — reconciled after the counter unfreeze

The wall was investigated exhaustively THIS window by the observer (harness-owned): the 93s→694s
regression was root-caused to the disk-hygiene cron calling ramdisk `--flush` unconditionally every
3min (deleting RAM incremental state under live compiles) plus the deps-prune force-running under the
25G floor at disk-94%. Both fixed (flush now refuses under a live compiler; 10G of dead gate binaries
reclaimed → 29G free). The WALL mark was re-baselined 72→189 (72 was a lucky-low min-lock). MEASURED
after the fix: a warm quiet-box verify is **68s green** (build 0s) — comfortably under both the 189
mark and the 93s ceiling. No standing bloat to cut agent-side; the wall is lean. The one queued lever
(observer): unifying the two gate feature-variants would halve the ~90G live binary mass and relink time.

**LAST_WALL_AUDIT set to 325.**

**Next audit due: tick 345.**
