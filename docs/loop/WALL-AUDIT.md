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
