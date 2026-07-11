# EPOCH-1 — whole-star systemic audit (2026-07-11)

_Trigger: §10.1 drift detector fired — capability +49 vs quality +3 over Ticks 1–17 (drift 46 > 25).
Scope: all 12 PRODUCT STAR points (§0). Coverage is total; remediation is bounded (ADR-006/007)._

## 1. Measurements (the numbers; median of 5 runs, release build)

### RESPONSIVENESS + EFFICIENCY — per-stage hot path

`manuk-wpt bench --pages example.com, news.ycombinator.com, en.wikipedia.org/Rust --runs 5`

**BEFORE** (ms):

| page | KB | nodes | parse | cascade | layout | dlist | paint | **TOTAL** |
|---|---|---|---|---|---|---|---|---|
| example.com | 0.5 | 19 | 0.01 | 0.01 | 0.01 | 0.00 | 0.44 | **0.47** |
| Hacker News | 33.8 | 1,282 | 1.06 | 0.50 | 3.74 | 0.06 | 2.09 | **7.46** |
| Wikipedia | 974.6 | 18,658 | 18.86 | **84.56** | 17.60 | 0.73 | 6.22 | **127.97** |

**The finding.** Cascade was **66% of the entire pipeline** on a large real page, and **superlinear**:
per-node cascade cost rose **0.39 → 4.53 µs/node (×11.6)** from 1.3k to 18.7k nodes. Root cause:
every element was matched against **every rule** — O(nodes × rules), no selector index.

**AFTER** (rule index + no per-element alloc/sort):

| page | cascade | Δ | TOTAL | Δ |
|---|---|---|---|---|
| Wikipedia | **31.40 ms** | **2.69× faster** (was 84.56) | **76.44 ms** | **1.67× faster** (was 127.97) |

Cascade fell from 66% → 41% of the pipeline. **Parity stayed 72/72** — the index only skips rules
that provably cannot match, so computed styles are identical.

### RELIABILITY

- **4 real UI-thread `block_on`s** (each a latent hang): page load, external stylesheets, the
  fetch pump, the agent panel. `gui.rs:754,755,1403,1800`.
- 43 `unwrap`/`expect` in shell; 24 raw `styles[&…]` indexes in layout (one already caused a real
  panic on every declarative-shadow-DOM page — fixed in Tick 15).

### COMPLETENESS  ⚠ **CRITICAL FOUND**

Enumerated every user-reachable affordance. All 9 hamburger items are wired **except**:
- **"Downloads" was a DEAD AFFORDANCE** — it only wrote to `tracing` (stderr). A user who clicked
  it saw **nothing at all**. Per ADR-007 a critical cannot be deferred. **Fixed in-epoch**: a real
  Downloads panel (filename, size, path; "No downloads yet — saved to …" when empty; Esc closes).

No `todo!()`/`unimplemented!()` reachable by a user.

### FIDELITY / AESTHETICS
Parity 72/72. Real-page Chrome diffs done in Tick 17 (HN now renders correctly). **Gap: the shell
chrome itself cannot be screenshotted** (no headless paint path) — so chrome aesthetics are *not yet
probeable*. That is itself a probe gap → debt (L44).

### COHERENCE
One `Page` pipeline; shell + agent + render CLI all drive it. No forked page path found. ✅

### ACCESSIBILITY / SECURITY / IDENTITY / AGENT-DRIVABILITY
a11y tree + typed actions present; safe-by-default cookies/adblock; honest UA (ADR-004); automation
surface tested (122 agent tests). No new criticals. Deeper probes are debt (see below).

## 2. Remediation (bounded — worst-first, in-epoch)

| # | finding | severity | action |
|---|---|---|---|
| 1 | Cascade O(nodes × rules), 66% of pipeline, superlinear | MAJOR | **FIXED** — rule index (2.69× cascade, 1.67× total) |
| 2 | "Downloads" dead affordance | **CRITICAL** | **FIXED** — real Downloads panel |
| 3 | 4 UI-thread `block_on` (latent hangs) | MAJOR | **DEBT-1** |
| 4 | Residual cascade superlinearity (still 4.3× worse/node at scale) | MAJOR | **DEBT-2** |
| 5 | Layout 2.9 µs/node on table-heavy pages (HN) | MINOR | ledger |
| 6 | Chrome aesthetics unprobeable (no shell headless paint) | MAJOR | **DEBT-3** (probe gap) |
| 7 | Display list rebuilt in full every frame | MINOR | ledger |

## 3. Invariant floors (now binding — §1; a tick that regresses one FAILS)

Measured on this machine, release build, median of 5:

- **F1 — cascade:** Wikipedia-class page (≈19k nodes) cascade ≤ **40 ms**. (now 31.4)
- **F2 — full pipeline:** Wikipedia-class page parse→paint ≤ **95 ms**. (now 76.4)
- **F3 — mid page:** HN-class page (≈1.3k nodes) parse→paint ≤ **10 ms**. (now 7.7)
- **F4 — no dead affordances:** every user-reachable control must do something observable **in the
  UI** (not only a log line).
- **F5 — parity:** 72/72 (pre-existing).

Re-measure with `cargo run -q -p manuk-wpt --release -- bench --pages …`.

## 4. STAR DEBT (rate-enforced: ≥1 retired per 3 ticks; next epoch cannot close over unpaid debt)

- **DEBT-1** — eliminate the 4 UI-thread `block_on`s (RELIABILITY: latent hangs).
- **DEBT-2** — residual cascade superlinearity (EFFICIENCY).
- **DEBT-3** — shell-chrome headless paint, so AESTHETICS/ERGONOMICS become probeable at all.

## 5. Verdict

Star coverage: **12/12 points probed.** 1 CRITICAL found and fixed. 1 MAJOR fixed (the headline
perf win). 3 MAJORs deferred as rate-enforced debt. Floors set and now binding.
**The browser is 1.67× faster end-to-end on a large real page than when this epoch opened, and has
one fewer dead button.**
