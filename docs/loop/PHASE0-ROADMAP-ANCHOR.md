# PHASE-0 ROADMAP ANCHOR — the verified bound, the remaining ledger, and how we know it converges

_Anchored 2026-07-23 at tick 461. This is the FIXED reference the loop is measured against until
the Phase-0 certificate holds. Every full corpus sweep updates the ACTUALS columns; if actuals
drift >20% past the bound, the drift itself becomes a tick: re-derive, don't rationalize._

## 1. The claim, verified against measured data

**Phase 0 completes in ~85–150 ticks from tick 460** (so by roughly tick 545–610). Verification:

- **Cadence (measured, not assumed):** ticks 330→461 landed in 43 hours = **72 ticks/day**.
  The bound's own math assumed 25–40/day, so the calendar estimate (3–6 days) is conservative;
  at measured cadence the midpoint is **1.5–2.5 days of loop time**.
- **Convergence (measured twice):** t380 full sweep → t392 re-sweep collapsed the ledger
  627→392 clusters in ONE cycle; the two largest families (img/svg inline ~80 sites each,
  `<br>` missing box 64→5) vanished wholesale; all four jarring invariants improved.
  Head-heavy, cause-driven behavior — the opposite of a Presto tail.
- **Bound integrity so far:** the original 100–150 bound (set ~tick 330) has spent ~130 ticks
  and delivered the ENTIRE exit instrument, the full media/codec chain, and most of the S/M
  list — on budget with the 1.5× subsystem-overrun factor already priced in.
- **The cut line is half the proof:** 16 named exceptions (DRM, WebRTC, WebGPU/Figma-class,
  canvas-office EDITING, Web Audio, push, HTTP/3, …) mean the target is "almost no site is
  jarring," not "every capability exists." A bounded goal is what makes a bounded plan possible.

## 2. The remaining ledger (from tick 461)

### A. Named build items — ~49–88 ticks

| item | size (ticks) | status | verified by |
|---|---|---|---|
| **Rich-editing subsystem** (contenteditable commands, caret, typing; IME composition already landed t332) | 10–20 | ENTRY LANDED t456–457; in progress | new g_editing gates + Gmail-compose-class page gate |
| **Software WebGL backend** | 7–15 | not started; **down-scopeable** (Maps degrades to raster) | g_webgl context + honest-strings gate |
| WebAuthn/passkeys | 3–6 | not started | real-flow gate (register+assert round-trip) |
| Password vault UX (crypto core exists) | 3–6 | not started | save-prompt/fill-picker shell gates |
| bidi full line reordering | 3–4 | not started | RTL fixture gates + corpus RTL sites |
| Live CSS animation/transition timeline | 4–8 | end-state-only today | g_animation upgraded to timeline assertions |
| Form widget painting remainder (date/color/time pickers, select popup, accent-color) | 2–4 | not started | paint-diff gates vs measured Chrome |
| Visual-effects bundle (filter, backdrop-filter, mix-blend, clip-path) | 4–6 | not started | reftest-style paint gates |
| MathML | 3–6 | not started | Wikipedia-formula fixture gate |
| multicol | 3–4 | not started | layout gate + news-site corpus rows |
| Print output | 2–3 | @media print applies; output missing | printable-surface gate |
| ch/ex real font metrics (Parley borrowable) | 2–3 | stubbed | font-metrics gate + ~194 WPT |
| misc S items (residuals) | ~3 | — | per-item gates |

### B. Ledger-driven jarring fixes — ~30–60 ticks (the fuzzy bucket, see §4)

Current invariant state (t392 sweep, share of scored sites FAILING → required ≤5%):

| invariant | t380 | t392 | Δ/cycle | distance to bar |
|---|---|---|---|---|
| overlap | 45.2% | 43.7% | −1.5 | 38.7 pts |
| h-overflow | 33.5% | 30.3% | −3.2 | 25.3 pts |
| **reading-order** | 71.6% | 63.0% | −8.6 | **58.0 pts** ← the fat-tail risk |
| dead-target | 47.2% | 32.8% | −14.4 | 27.8 pts |

Points-per-cycle is NOT the right extrapolation (fixes target CAUSES; a killed cause removes
many points at once — dead-target dropped 14.4 in ~4 ticks). The ledger's cluster count per
invariant after each sweep is the honest progress meter.

### C. Exit verification — ~5–10 ticks

test262 run · 100-tab RSS benchmark · large-DOM interactivity probe · repeat full-corpus
sweeps until the certificate holds on consecutive runs.

**Total: ~84–158 ticks — consistent with the 85–150 claim.**

## 3. Definition of done (unchanged, mechanical)

The FIDELITY-SCORING-REDESIGN certificate, measured by the rebuilt instrument on the stratified
corpus: **Bar 0 (zero crash/hang) + all four jarring invariants ≥95% clean + shape ≥0.75 on
≥95% of sites + interactivity ≥95% + only the 16 named exceptions unmet.** Never ready_pct,
never WPT count, never a vibe.

## 4. Risk register (ranked)

1. **Reading-order invariant variance** — 58 points to close; the widest error bar in the plan.
   Decision gate: after the next TWO full sweeps, if the reorder CLUSTER COUNT is not shrinking
   ≥30% per cycle, stop grinding and re-derive: either (a) the residue is one structural cause
   (float/abspos ordering — fix the cause, not the sites), or (b) the invariant OVER-COUNTS
   benign reorderings (flex `order`, positioned navs) and needs refining against measured Chrome
   behavior — refine the definition with evidence, never retune the bar to pass.
2. **Rich-editing overrun** — the one remaining true subsystem; historical L-overrun is 1.5–2×
   (priced in at the 20-tick top). If it exceeds ~25 ticks, decompose and interleave.
3. **WebGL scope** — 7–15 ticks for a capability whose absence mostly DEGRADES (raster Maps
   fallback). This is the designated pressure-relief valve: if the total tracks toward the top
   of the bound, down-scope WebGL to context+honest-strings and move the rest post-certificate.
4. **Wall flakes** (G_INTERACT timing, loaded-first-verify ceiling trips) — ~4/day, ~9 min each,
   all self-healing to date. Watch frequency; a rolling-min ratchet is the ready fix if it grows.

## 5. Re-anchoring protocol

- Each full corpus sweep appends one line here: `sweep @tick N: clusters X, invariants a/b/c/d,
  ticks-spent-since-anchor S` — actuals vs bound, in one place.
- Drift >20% over the 158 top → a mandatory re-derivation tick (measure why, update the bound
  with evidence, journal it). The bound is falsifiable on purpose; that is what makes it a bound.
