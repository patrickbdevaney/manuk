# Reliability Doctrine — truth is COMPUTED, not ASSERTED

_Written 2026-07-26 (tick ~600), observer. The goal: make the loop's recurring failure modes
self-detecting and self-correcting, so they never propagate as false signal — the achievable, honest form
of "extinct." A load flake can still occur and an LLM can still misread; the doctrine ensures neither ever
SURFACES as a wrong conclusion, because every layer is reconciled against a source that cannot lie._

## The four failure modes, and the one root cause

The user named four (all observed repeatedly this session):

1. **False REDs** — the wall reports a gate failed when the engine is fine (manuk-shell/manuk-agent
   "INSTRUMENT FAULT — no verdict" under load, ~7× this session).
2. **Inaccurate instruments** — a metric that lies (vacuous `0/0=100%`, `.SIG` keying, the `mag_band`
   printer, RSS-eviction "returned nothing", `@supports` answering parse-not-render — six in one session).
3. **Re-implementing done work** — a commit "implements" something already implemented (the stale-
   pessimistic constellation; "the map said `missing`" when the gate was green).
4. **False positives of presence** — a capability marked `works` that is a silent stub ("the map said
   `works`" for `srcset`/`navigator.plugins`, which were absent; the "honest answer ≠ fixed answer" trap).

**They are one bug wearing four masks: TRUTH IS ASSERTED IN PROSE (the map's `status`, the board's steers,
a commit's claim, a gate's expected-value-from-memory) INSTEAD OF COMPUTED FROM A FALSIFY-PROVEN,
BEHAVIOUR-EXERCISING, REALITY-RUN GATE — and nothing mechanically ties the assertion to reality, so it
drifts.** Every failure above is an assertion that drifted from what the engine actually does:

| Failure | The assertion | The reality it drifted from |
|---|---|---|
| False RED | "no verdict ⇒ failed" | no verdict ⇒ *unmeasured* (re-measure, don't fail) |
| Inaccurate instrument | "this new metric is trustworthy" | untrusted until falsify-proven (RED-provable) |
| Re-implement done work | map `status = missing` | the named gate is green *right now* |
| False presence | map `status = works` | no green **behaviour-exercising** gate backs it |

## The principle

> **Truth must be COMPUTED from falsify-proven, behaviour-exercising, reality-run gates — never asserted in
> prose. Every human/agent-readable layer (the map, the board, the certificate, a commit's claim) is a
> PROJECTION of that computed truth, and a RECONCILIATION gate fails the tick the moment a projection
> drifts from its source.** Assertion is allowed only where it is immediately checked against computation.

This is the same discipline the certificate redesign (`DAILY-DRIVER-CERTIFICATION.md`) applies to the
Phase-0 measurement — generalised to the whole loop.

## The mechanical guards (per failure mode: state · fix · direction)

### 1. False REDs → re-measure, never fail on a busy box
- **Done (t600, `verify.sh` `_crate_suite`):** a no-verdict is a HARNESS condition, so the wall now
  RE-MEASURES the gate alone on a SETTLED box (load < 1.5), up to 3×, before believing it. A verdict on a
  quiet box is the truth; only 3 quiet-box no-verdicts is a real hang/OOM. The load-induced flake now
  self-heals INSIDE the wall — the tick never sees it. (`bad` on a real `test result: FAILED` is
  immediate and never retried — a real red still stops the tick at once.)
- **Direction (agent):** the flaky gates are timing/ratio tests (G_INTERACT, alloc, agent) that flake
  because they measure WALL-CLOCK under contention. Re-express them against a LOAD-INVARIANT quantity
  (allocation counts, instruction counts, a monotone frame budget) and the no-verdict cannot occur at all.

### 2. Inaccurate instruments → no metric is trusted until falsify-proven
- **Done:** `falsify.sh` mutation-tests the gate wall; the pre-commit hook requires a gate be RED-proven;
  the certificate now falsifies each of its own terms (t583, agent).
- **Fix (observer to build):** extend the "must ship with a RED proof" rule from GATES to INSTRUMENT
  METRICS (the cert numbers, the RSS figure, any published %). A metric committed without a falsify test
  that demonstrably moves it is a hypothesis, not a measurement — flag it. Every ratio carries its N;
  `0/0` is `UNSCORED-against-bar`, never a pass (generalise `CERT_MIN_SHAPE_SAMPLE`).

### 3 & 4. Re-implementation AND false presence → the map is a PROJECTION of the gate wall
Both are map-drift, and they have ONE fix: **`status` must be COMPUTED, not hand-set.** The constellation
already names each row's `gate` (audit #33 made it machine-readable, unmapped-gates → 0). The end state:
- A **map-reconciliation gate** (`scripts/` — observer-owned) recomputes each row's status from the CURRENT
  wall result of its named gate: green gate ⇒ `works`; red ⇒ `broken`; no/absent gate ⇒ `unmeasured` (never
  `works`, never `missing` — "we don't know" is its own status). It FAILS the tick if a hand-set `status`
  disagrees with its gate, or if a `works`/`gated` row has no behaviour-exercising gate.
- Consequence: a row cannot claim `works` without a green gate (kills false presence #4), and cannot claim
  `missing` while its gate is green (kills re-implementation #3 — "already done?" is answered by the gate,
  not by reading prose). The agent's standing RE-PROBE-BEFORE-BUILDING rule becomes mechanical.
- A `works` gate that only checks PRESENCE/PARSE (not behaviour) is itself a false-presence risk (the
  `@supports` parse-not-render bug): the reconciliation should flag gates that assert an intermediate
  (`typeof x === 'function'`, "it parses") rather than the OBSERVABLE OUTCOME (it renders / it functions).

### 5. Agent context / rule-following → the board is short, dated, and self-flagging
The agent reads the board every tick and works from it. Audit #33: "the board steers at finished work."
- **Done (t599):** a CLARITY line names the only live CO-#1s and marks the rest historical.
- **Fix (observer):** PRUNE the finished steer-blocks (careful pass); keep the board SHORT — only live,
  DATED orders — with history in git, not on the board. A steer that names a capability whose gate is now
  green should auto-flag as stale (same reconciliation as the map). The board's authority-of-the-moment is
  only as good as its freshness; a cluttered board dilutes every steer in it.

## The reconciliation backbone (the meta-instrument)

STATUS.md's meta-instrument #3 already says it: *"8 of 30 process defects were caught by a number that did
not add up."* Make reconciliation a standing WALL gate, not a manual check. Everything that should balance,
must, or the tick fails:
- sites sampled == scored + FAIL + EXCLUDED · parsed == probed == scored (the certificate, §6).
- map `status` == its gate's current result (§3&4 above).
- board steers == not-yet-green capabilities (§5).
- every published metric has a falsify proof on file (§2).

A number that doesn't add up is the single most reliable defect detector this project has. Wiring it in is
how "the instruments cannot lie" stops being a hope and becomes a property.

## Honest scope

"Conclusively extinct" — stated precisely: **no failure mode ever SURFACES as a wrong conclusion.** The
flake still happens; it self-heals inside the wall. The LLM can still misread; the map it reads is
gate-derived truth and reconciliation catches the drift. The metric can still be wrong on first write;
falsify catches it before it is trusted. That is bulletproof-ENOUGH to reach the daily-driver certificate
with certitude, because the certificate — and every number feeding it — is computed, falsify-proven, and
reconciled, not asserted. Perfection is not the target; a system that cannot fool itself is.
