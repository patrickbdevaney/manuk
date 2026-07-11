# Manuk — DECISIONS (append-only ADRs)

_Tradeoffs, constitution amendments, axis changes, north-star-drift rationale. Long-horizon
coherence lives here. Newest last. See [[CONSTITUTION]]._

## ADR-001 — Adopt the autonomous perpetual loop (2026-07-11)

Established `docs/loop/` as the deterministic, resumable operating system for indefinite
self-directed engineering toward a global maximum. Invariants: parity 72/72, build green,
memory-safety + no vendored-engine internal patches, honesty, coherence (commit+push), no local
axis regression. Absorb outstanding beneficial work (tiers, RESEARCH_V2, IMPLEMENTATION
follow-ons) into the LEDGER first, then drive new innovation via UCB explore/exploit.

## ADR-002 — SpiderMonkey is the shell default (2026-07-11)

Now that interactive JS works (persistent per-page context + click dispatch), a plain
`cargo build -p manuk-shell` should produce a browser that runs the real interactive web. The
parity harness (`manuk-wpt`) stays on its own lean features so the gate stays fast. Cost: heavier
default build; accepted — a browser without JS is not the product.

## ADR-003 — Disk hygiene is a loop responsibility (2026-07-11)

The loop prunes stale debug artifacts each tick and does periodic fuller cleans (keeping release
artifacts), never touching `~/.cargo` registry or vendored engine trees. Prevents an indefinite
loop from filling the disk.

## ADR-004 — Amended mission: maximal traversal + ambidextrous spine (2026-07-11)

**Amendment (user-issued).** Two clarifications that sharpen, not replace, the north star.

**1. Maximal traversal, earned by capability — not a checklist, not a disguise.**
The ambition is near-total traversal of the *real* internet: Chromium/Gecko-parity breadth across
the **kinds** of site the web is actually made of — real-time virtualized feeds (X), session-heavy
professional platforms (LinkedIn, Indeed), media-rich client apps (Instagram), complex
authenticated dashboards (cloud consoles, banking, account pages). Named sites are *representative
points in that space*, never a list to clear; the target is the whole space they stand in for.

Manuk earns that access the way any real browser does: **by being one.** It presents its own
genuine TLS handshake and its own genuine engine fingerprint. Chrome has a Chrome fingerprint,
Firefox a Firefox one, Safari a Safari one — **Manuk has a Manuk fingerprint, and it is earned,
not hidden.** This *strengthens* the existing honesty invariant (§1.4): the strategy for coverage
is closing the **capability gap** — full JS/DOM depth, full layout/CSS fidelity, real-time-feed-
grade performance — until *being genuinely Manuk* is sufficient anywhere a real browser is
welcome. **A fifth real browser, not a disguise wearing someone else's face.** Impersonation is
therefore not merely forbidden, it is off-strategy: it would substitute for the very capability
that is the point.

**Loop consequence.** Prioritize by *traversal-blocking capability*, not by site. Ask of each
candidate: "which class of the real web does this unblock?" This raises: JS/DOM depth (the
remaining WebIDL surface), layout/CSS fidelity (now VISUAL-verifiable), virtualized-feed
performance (scroll/recycle/incremental-relayout under a live feed), and session/auth durability
(cookies, storage partitioning, OAuth, long-lived logins).

**2. Ambidextrous spine — one engine; the split is who's driving, not which binary.**
A human drives the headful GUI. An agent drives **either** headless mode (no window; scale and
throughput) **or the same headful GUI, visibly and live**. Both are the identical engine and page
pipeline; they differ only in whether a window is presented and who issues the actions. This is
"shared core, diverge at consumption" made literal: one browser, two ways of holding the wheel.

**Loop consequence.** No agent-only or human-only fork of the page pipeline. The automation
surface (L30) and the shell must drive the *same* `Page`/action path — an agent action in headful
mode must go through the same code a human click does. Divergence is a defect.

## ADR-005 — EPOCH: the long-horizon systemic audit (2026-07-11)

**The flaw this fixes (with evidence).** Over Ticks 1–17 the loop drove capability hard and
quality not at all:

| axis | start | now | Δ |
|---|---|---|---|
| JS | 55 | 77 | **+22** |
| COMPAT | 42 | 57 | **+15** |
| RENDER | 70 | 82 | **+12** |
| PERF | 55 | 58 | +3 |
| MEM | 55 | 55 | **0** |
| STABILITY | 55 | 55 | **0** |

Capability **+49**; quality **+3**. Every tick added a feature and verified it *in isolation* —
build, parity, one test, one screenshot. **Nothing ever verified that the whole engine is fast,
lean, and does not hang.** The loop optimized exactly what it measured. The north star said
"production-grade, blazing fast, stable" and the machinery did nothing to make that true. That is
a hole in the *design*, not in any one tick.

**The rule.** Per-tick gates stay as they are (they are the right granularity for a feature). Add a
**rare, intensive, whole-system gate** — an **EPOCH** — at major-milestone cadence. An epoch is not
a feature tick: it is a dedicated arc that treats the browser as one machine and ruthlessly
optimizes it end to end (see CONSTITUTION §10).

**Trigger (a drift detector, not just a counter).** An epoch is DUE when either:
- **(a) cadence** — ≥ 20 ticks since the last epoch; or
- **(b) drift** — `(ΔRENDER + ΔJS + ΔCOMPAT) − (ΔPERF + ΔMEM + ΔSTABILITY) > 25` since the last
  epoch.

(b) is the important one: it fires precisely when features outrun the machine, which is the failure
mode above. By this rule an epoch is **due right now** (drift = 49 − 3 = 46).

**Why not just do perf work as ordinary ticks?** Because systemic optimization is not decomposable
into feature-sized units. Latency comes from the *interaction* of cascade, layout, paint, event
loop and I/O; an O(n²) only shows at scale; a hang only shows under a real session. A tick-sized
slice would measure the part and miss the machine. It must be a distinct, infrequent, deep arc —
"epochal step jumps," as the mission puts it — not a recurring tax.

**Output is binding.** An epoch publishes numbers and then **converts them into invariant floors**
(CONSTITUTION §1): once a latency/memory budget is measured and set, a later tick that regresses it
FAILS, exactly as a parity regression fails today. That is what stops the drift from re-accruing.
