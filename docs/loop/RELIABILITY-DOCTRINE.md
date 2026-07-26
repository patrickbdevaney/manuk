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

**BUILT (t601, `scripts/map-reconcile.sh`)** — the LIGHT, SOUND half first: read-only (no cargo, safe
mid-tick), it verifies the invariant that KILLS false-presence at the assertion layer — *a row claiming
`works`/`gated`/`partial` must name a gate whose test actually EXISTS* (fuzzy `G_FOO`→`g_foo*.rs` + const
grep; multi-gate cells tokenized so ANY real backing gate satisfies it). First honest run: **26 drift rows
of 357** — 20 BARE assertions (status claimed with gate `-`) + 6 DANGLING (cites a gate name with no test).
The *result*-reconciliation half (green-gate⇒works / red⇒broken) needs a per-gate result ledger that does
not exist yet (there is no sound way to recompute 300 statuses without re-running the wall) — deferred.
ROLLOUT (per the doctrine's own no-brick rule): report-only today (`exit 0`); the map's owner (the agent)
drives the 26 to 0 (cite the real gate, or set `missing`/`unmeasured`); THEN `--strict` wires it into the
wall as a failing gate. The check itself ate its own dog food — its v1 flagged 90 false positives from
un-tokenized multi-gate cells; shipping that would have been the very inaccurate-instrument mode it targets,
so it was falsify-corrected before commit.

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

## Is this ALL the loop's failure modes? No — there are THREE categories, one deeper principle

Honest audit (the doctrine above covered only the first): the four named failure modes are the
**MEASUREMENT-TRUTH** category. The loop has two more categories this had not yet named.

**Category A — MEASUREMENT TRUTH (the four above).** *The loop lies about progress.* Root: assertion vs
computation. Coverage: false-RED **built** (t600); falsify / fixed-denominator / reconciliation **designed,
partly built** (the certificate).

**Category B — OPERATIONAL / SUBSTRATE reliability.** *The mechanical substrate fails under real load /
disk / concurrency / a changed environment — independent of any measurement.* The paid-for incidents:
poisoned **wall-bank** (a green wall measured under load1≥3 / disk≥93% / a live compiler gets banked and
sticks, blocking every commit — and the gate phase itself stamps load 4.5–6.5, so the current guard *still
misses it*); **ramdisk `--flush`** deleting incremental under a live compiler → bistable wall + false-RED;
**disk-hygiene prune** eating a live gate binary → false-RED; **silently-dead crons** (an escaped-quote
crontab round-trip killed hygiene + the watchdog for hours); **uncontained agent** (systemd-run unavailable
→ OOM/hang); **git races** (atomicity `checkout` wipes uncommitted observer work; a bare commit sweeps the
agent's staged work); **stale systemd-scope accumulation**; **CI/env drift** (Chrome's IPv6 bind, the musl
toolchain). Root: **the substrate's state is ASSUMED, not VERIFIED.** Fix theme: idempotency + fail-safe
defaults + **post-condition verification** — verify the cron actually ran (log heartbeat), the scope was
created (containment check), the wall env was valid *at bank time* (not during the gate phase), the commit
isolated the intended paths — never assume. Status: **BUILT (t601, `scripts/ops-check.sh`)** — the
consolidated OPERATIONAL-INVARIANTS self-check runs from cron (*/15) AND every observer heartbeat, and
turns each incident above into a standing checked invariant: it AUTO-HEALS the safe ones (un-escapes a
corrupted crontab — the silent-cron-death bug; `reset-failed` + stops empty stale scopes) and ALERTS on the
rest (hygiene-log stale ⇒ reaper dead; /home ≥92%; double-spawn; uncontained agent; a stale-high wall-bank
needing re-baseline). Fail-safe by construction: it only READS + a whitelist of safe heals + logs, NEVER
kills the live agent/supervisor, NEVER re-baselines the wall (observer's call), and always `exit 0` so it
can never fail a tick. Residual (still reactive): the git-race discipline (commit-immediately + pathspec)
and CI/env drift (external) — both are DISCIPLINE, hard to gate mechanically.

**Category C — OBSERVER reliability (me).** *The overseer itself drifts.* This session: I relayed in-flight
findings as conclusions ~5× (the "quantised", "class-failure", and RSS-thesis reversals), and the project
has caught observer wrong-mount reads (`df /` vs `/home`) and observer contention (uncapped work OOMing the
box). Root: **the observer ASSERTS from a partial read instead of waiting for the settled/committed truth.**
Fix: report only committed/settled facts, label hypotheses as hypotheses, cap all heavy work, measure the
right mount. Status: acknowledged and disciplined; not mechanically guarded (an overseer is hard to gate —
the honest mitigation is the discipline, stated).

### THE DEEPER PRINCIPLE that unifies all three: **DON'T ASSUME — VERIFY, with a falsify-proven check.**
- Measurement-truth: don't *assert* capability, *compute* it from a gate.
- Operational: don't *assume* an operation succeeded or a condition holds, *verify* its post-condition.
- Observer: don't *conclude* from a partial read, *wait* for the committed number.

No belief — about the engine, the substrate, or the progress — is trusted until it is VERIFIED against
reality by a check that can itself detect the failure it guards against. The reconciliation backbone is
that principle made mechanical. "Truth is computed, not asserted" is its measurement-facing half; "verify
every post-condition, assume nothing" is its operational half. Same law.

## Honest scope

"Conclusively extinct" — stated precisely: **no failure mode ever SURFACES as a wrong conclusion.** The
flake still happens; it self-heals inside the wall. The LLM can still misread; the map it reads is
gate-derived truth and reconciliation catches the drift. The metric can still be wrong on first write;
falsify catches it before it is trusted. That is bulletproof-ENOUGH to reach the daily-driver certificate
with certitude, because the certificate — and every number feeding it — is computed, falsify-proven, and
reconciled, not asserted. Perfection is not the target; a system that cannot fool itself is.
