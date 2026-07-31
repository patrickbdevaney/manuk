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

## Audit #3 — tick 326 (wall 189s warm / 706s audit-run cold)

Counter-unfreeze artifact: LAST_WALL_AUDIT stuck at 113 because TICK was frozen at 128 for ~200 ticks;
no 20-tick cadence was actually skipped. First wall audit since the counter was fixed.

**Where the seconds go (wall-audit.sh histogram, this run):** P (parity/prewarm) 172s ≈ the whole cost,
then T 30s, B 30s, D 11s, gates ≤6s each. The 706s total here is a COLD audit-run number; the banked
warm wall is 189s (RATCHET.tsv, re-baselined by the observer from a green build-0s receipt — the earlier
72s mark was a lucky-low min-lock, see wall-mark-min-lock-rebaseline).

**Rigor-preserving findings:** the dominant cost (P — the parity/oracle prewarm + release relink) is
HARNESS-OWNED (scripts/verify.sh, scripts/ramdisk.sh) and the observer is already actively managing it
(feature-variant unification in c059370, ramdisk incremental-flush guard, disk-hygiene stem-prune age
floor). There is nothing agent-actionable here that trims seconds without touching scripts/ — and the
agent must not edit the harness. No gate is redundant, over-scoped, or serialised in a way an
engine-side change could fix. **The wall is as lean as the agent can make it; the remaining bloat is
harness territory, deliberately left to the observer.** No gate cut, no floor widened.

**LAST_WALL_AUDIT set to 326.**

**Next audit due: tick 346.**

## Audit #4 — tick 346 (wall 66s warm / 348s first-run cold)

FINDING: the warm wall is LEAN and unchanged in shape from Audit #3 — a 2nd back-to-back verify.sh on a
quiet box comes in at **66s** (gate 66s, build 1s). The FIRST run of a session measures ~271-348s, but
that is the documented cold-parity + disk-reclaim cost, not gate bloat: with `/home` at ~90% verify's
own "reclaim before ENOSPC" step and the hygiene cron delete regenerable churn mid-build, forcing a
relink on run 1; run 2 finds everything warm (build 1s). This is the same bistable wall the observer
steers own (disk pressure), not a coverage or gate-scope regression.

NO TRIM: none of the four rigor-preserving levers (redundancy / parallelism / caching / scope) applies
without touching scripts/ (observer-owned) — the JS-runtime-startup redundancy (~1.5s/gate) is a
cargo-nextest change in verify.sh, which is out of agent scope. The wall is already at its warm floor;
the lever that matters (disk headroom so run 1 doesn't relink) is infrastructure the observer owns.
Ticks 344/345/346 added +2 crate tests (manuk-net) + 1 page gate (g_drag_reorder, not in the curated
_launch wall) — negligible wall cost, all under the warm floor. Wall stays lean; nothing cut.

## Audit #5 — tick 366 (wall 59s warm)

FINDING: the warm wall IMPROVED against Audit #4 (66s → 59s) while the window 347-365 added ~10 shell
suite tests (the whole media arc: G_MSE_JOIN, G_AUDIO_PUMP/JOIN, G_AV_MASTER, G_MUTED_OUT, G_IDL_FEED,
G_RATE, G_AV1_DRIVE, G_AVIF_PAINT, G_MP3_DRIVE) plus 3 engine media gates in the required-features
lane (outside the wall). Breakdown: T 21s (crate tests — where the new suite rides; +0 net vs the
pre-arc shape because the fixtures are small and decode in ms), P 13s (parity), B 10s (build), G6 8s,
G1 3s, F 2s. The bistable first-run cost (~490-540s after a Cargo.lock/feature change: re_rav1d,
avif-parse, symphonia features all landed this window) remains the documented cold-relink shape —
run 2 is always warm; marks were never retuned.

NO TRIM: same conclusion as #4 — the four rigor-preserving levers all live in scripts/ (observer-owned;
nextest/runtime-sharing named there already). Agent-side additions stayed under the warm floor by
construction: decode gates use small fixtures, the shell suite shares one binary, one JS test per
binary (the t354 rule) keeps mozjs startups at one. Wall stays lean; nothing cut; coverage grew.

## Audit #6 — tick 386 (wall 57s warm)

FINDING: warm wall held (59s → 57s vs Audit #5) across a window (367-385) that ADDED coverage:
G_MIX rewrite + resampler claims (t375), scrollp in g_mse_join (t378), the containerq probe in
G_PROBE_CAPABILITIES (t379), the countable-dead-stylesheet claim in G_SILENT_FAIL (t383), and two
manuk-layout tests (t384 atomic inline replaced, t385 br geometry) — all riding EXISTING binaries,
zero new runtimes stood up. Breakdown: T 20s (crate tests, 35%), P 14s (parity, 25%), G6 6s, G1 4s,
F 2s, B 1s (warm). The cold-first-run shape after an engine change (~380-560s this window) remains
the documented relink cost — run 2 is always warm; marks never retuned. Off-wall additions: the
corpus oracle run (t380) is correctly OUTSIDE the per-tick wall (its 60-90min lives in the crawl
driver), and the oracle's new starved-fetch discard adds nothing to wall time.

NO TRIM: the admissible levers (nextest, runtime-sharing, section parallelism) all live in
scripts/ — observer-owned, already named in prior audits. Agent-side discipline held: claims ride
existing binaries, one JS test per binary, small fixtures. Wall stays lean; coverage grew again.

## Audit #7 — tick 406 (wall 68s warm)

FINDING: warm wall held (57s → 61-68s band vs Audit #6) across a window (387-405) that ADDED
coverage: an_inline_svg_paints_its_vectors in G_FIRST_PAINT (t394), a stacklimit pin in
G_PROBE_CAPABILITIES (t400), and THREE new page gates from the named-error harvest —
G_DOCUMENT_LOCATION (t402), G_GET_PROPERTY_VALUE (t403), G_CURRENT_SCRIPT (t404) — each a small
fixture, one #[test] per binary (the t354 rule), riding the parallel gate launch. Breakdown:
T 21s (crate tests, 31%), P 14s (parity, 21%), G6 14s, B 11s, G1 4s, F 2s — same shape as #5/#6;
the G6/B variance is box load (the re-keyed corpus crawl runs nice-19 off-path during this
window; the wall still lands warm, which is the contention recipe working as documented). The
cold-first-run shape after engine/js changes (346-499s this window) remains the relink cost —
run 2 warm every time; no mark touched.

NO TRIM: same conclusion as #5/#6 — the admissible levers (nextest, runtime-sharing, section
parallelism) live in scripts/ (observer-owned, already named). Agent-side discipline held: three
new gates cost ~0 marginal wall (parallel launch, small fixtures, no new runtime per assertion
beyond the one-test-per-binary rule). Wall stays lean; coverage grew again.

## Audit #8 — tick 426 (wall 57s warm)

FINDING: warm wall held (57-66s band, same shape as #5/#6/#7) across the busiest coverage window yet —
window 416-427 added ELEVEN new page gates (text-indent/line-clamp, then the binary-seam vein: Intl,
checkVisibility, IndexedDB getAllRecords, structuredClone-binary, Blob-binary, canvas ImageData,
TextDecoder encodings, template.content, live url.searchParams, computed custom properties). Warm
breakdown unchanged: T ~21s crate tests, P ~14s parity, G6 ~14s, B ~2-31s build (varies with which
crate changed — dom/lib.rs and stylo_map.rs touches this window rebuilt wider than a JS-shim tick, but
still <35s), F ~2s. Each new gate is a small fixture, one #[test] per binary (t354 rule), riding the
parallel gate launch — ~0 marginal wall.

NO TRIM: same conclusion as #5-#7. The persistent ~480-520s COLD readings this window are NOT gate
bloat — they are environmental contention (an observer oracle crawl running ~6h at nice-19 + swap
95-99% thrashing the memory-heavy P/F benchmarks). The SAME tree warms to 57-66s on a quiet re-run
every time (verified repeatedly this session: t421/423/425 each banked 57-61s after a contended
480-500s reading). The admissible levers (nextest, runtime-sharing, section parallelism) remain in
scripts/ (observer-owned, already named). Wall stays lean when warm; coverage grew by 11 gates. Mark
untouched.

## Audit #9 — tick 446 (wall 60s warm)

FINDING: warm wall held at 60s — same shape as #5-#8 (57-68s band) across the form-controls value vein
(ticks 438-446: select/option write API, element.form, table DOM, textarea/output/progress values,
valueAsNumber/valueAsDate, .text, and this tick's datetime-local/week). Breakdown this run: T 22s crate
tests (37%), P 14s parity floors (23%), G6 6s, G1 4s, F/F4/B ~1-2s each. Each new gate rode the parallel
launch as a single small fixture (one #[test] per binary, t354 rule) — ~0 marginal wall despite ~8 gates
added this window.

NO TRIM: same conclusion as #5-#8. T (crate tests) and P (perf floors, deliberately serial — a benchmark
sharing the box is not a benchmark) are legitimate coverage, not bloat. The admissible rigor-preserving
levers (cargo-nextest runtime-sharing to reclaim the ~1.5s SpiderMonkey-startup tax per JS gate, section
parallelism) all live in scripts/ (observer-owned, already named in prior audits). Wall stays lean when
warm; coverage grew this window with no wall cost. Mark untouched.

## Audit #10 — tick 467 (wall 63s warm)

FINDING: warm wall held at 63s — squarely in the #5-#9 band (57-68s) across the CSSOM pref-flip vein
(ticks 458-466: deviceMemory/platform, select.options.length, custom-element callbacks, clipboard
image read/write, execCommand copy, user-select, color-scheme, contrast-color) and this tick's
`<details name>` exclusive accordion. Breakdown this run: T 22s crate tests (35%), P 14s perf floors
(22%), G6 8s (13%), G1 4s (6%), F/B ~1-2s. The new G_DETAILS_ACCORDION rode the parallel launch as a
single small page fixture (one #[test], the t354 rule) — ~0 marginal wall.

NO TRIM: same conclusion as #5-#9. T (crate tests) and P (perf floors, deliberately serial) are
legitimate coverage, not bloat. The admissible rigor-preserving levers (cargo-nextest runtime-sharing
to reclaim the ~1.5s SpiderMonkey-startup tax per JS gate, section parallelism) all live in scripts/
(observer-owned, already named in prior audits — I do not touch them). Wall stays lean when warm;
coverage grew this window with no wall cost. Mark untouched.

## Audit #11 — tick 487 (wall 663s CONTENDED; warm quiet = 68s at t486 landing)

FINDING: this audit fired on a CONTENDED reading, and the breakdown proves it rather than hiding it.
Sections summed to ~305s (P 225s/34%, B 25s, T 23s, G3 12s, G6 8s, D 5s, G1 4s, F 2s) against a 663s
total — i.e. ~358s UNACCOUNTED, the classic contention overhead. The tell is P: the parity section
(72/72 vs headless Chrome) normally runs ~14s (audits #5-#10) and here blew to 225s — the headless-Chrome
oracle was fighting the 9 leaked 5-DAY-OLD Chrome procs + an observer sweep for the box (load 15m-avg 2.99
draining while this ran). The SAME tree warmed to 68s at the tick-486 landing an hour earlier (quiet box),
squarely in the standing 57-68s warm band. So this is contention, not standing bloat.

NO TRIM: same conclusion as #5-#10. P (parity, headless-Chrome oracle) and T (crate tests) are legitimate
coverage — cutting either to buy seconds is the inadmissible trade this audit refuses by construction. The
admissible rigor-preserving levers (cargo-nextest runtime-sharing to reclaim the ~1.5s SpiderMonkey-startup
tax per JS gate; section parallelism; not banking a wall measured under load) ALL live in scripts/, which
is observer-owned — and the observer landed exactly one of them THIS window (commit 0e4e7c9
"fix(harness): don't bank a wall measured under high load + un-poison LAST_WALL_TIME"), which directly
addresses the poisoned-663s banking that blocked this tick's first pre-flight. Coverage grew +1 gate this
window (t486 G_USER_ACTIVATION, 252→253) with ~0 marginal warm wall. Mark untouched (189, ceiling 245).

## Audit #12 — tick 507 (wall 65s warm)

FINDING: the wall is LEAN. Total 65s against the 245s ceiling / ~57-68s standing warm band — squarely
in range, measured on a warm quiet-enough box (not a contended reading like #11's poisoned 663s).
Section breakdown: T 23s (35%), P 14s (22%), G6 8s (12%), G1 4s (6%), F 2s, everything else ≤1s. T
(crate tests) and P (parity vs headless Chrome) are the two biggest and are legitimate coverage, not
bloat — the same conclusion as #5-#11.

NO TRIM. The only admissible rigor-preserving levers (cargo-nextest runtime-sharing to reclaim the
~1.5s SpiderMonkey-startup tax per JS gate; section parallelism; narrower per-gate build scope) ALL
live in scripts/verify.sh, which is OBSERVER-OWNED per the loop scope — noted for the observer, never
actioned agent-side, exactly as prior audits recorded. Coverage grew this window (t506 esmmodule:yes
pinned in G_PROBE_CAPABILITIES) with ~0 marginal warm wall. Mark untouched (189, ceiling 245).

## Audit #13 — tick 527 (wall 67s warm)

Ran `./scripts/wall-audit.sh run`. The wall's warm cost is ~67s (STATUS LAST_WALL_TIME), well under the
189s re-baselined mark / 245s ceiling; the 693s figures this session are the CONTENDED gate-phase-spike
readings (the same `manuk-shell tests FAILED` false-RED cluster, 3× this session), not the warm number —
poisoned by load, not by code, exactly the pattern Audit #11 recorded at t487.

Against the four rigor-preserving axes: the wall is dominated by (a) the mozjs RELEASE link (~350MB, once
per final-tree build) and (b) per-JS-gate SpiderMonkey runtime startup (~1.5s × N gates launched
concurrently under CARGO_BUILD_JOBS). REDUNDANCY (could gates share one runtime binary — cargo-nextest),
PARALLELISM (gate scheduling), CACHING (incrementals already in RAM), SCOPE (whole-workspace vs
per-crate-test-binary builds) — every admissible lever the checklist names lands in **scripts/verify.sh +
the Cargo/build config**, which per V1-SCOPE.md + the loop charter are DONE and OBSERVER-OWNED. The agent
builds engine/ capability only and must not trim the wall.

FINDING: **the wall is lean on the axes the agent may touch — there is nothing to trim without crossing
into harness territory.** The standing gate-phase-spike false-RED (warm-re-run-landed each time this
session) is a scheduling/isolation matter for the observer, recorded here as a data point, not acted on.
The wall is a build-latency (observer) axis; it is NOT a capability regression — THE RATCHET held every
tick this session. An audit that finds the wall already lean (on the agent's axes) is a fine result, and
this is one.

## Audit #14 — tick 547 (wall 126s on the tick-546 landing run; 67s warm)

Ran `./scripts/wall-audit.sh run`. Breakdown: **G3 62s (49%)** · T 23s (18%) · P 14s (11%) · G6 9s ·
G1 5s · F 2s · everything else ≤1s. Total 126s — under the 189s mark and the 245s ceiling.

**FINDING, and it is about the AUDIT, not the wall: "G3 = 49% of the wall" is a measurement of
CONTENTION, not of the gate.** G3 is the whole `manuk-shell` suite, which runs in ~12s green and
standalone. It cost 62s on the audited run because the observer's flaky-gate retry loop (verify.sh
~L318: up to 3× serial re-runs, each waiting up to 60s for load1 < 2.5) *fired* — the timing gate
`G_INTERACT` false-RED'd under the gate-phase launch spike. So the audit's own top line describes the
retry, and a reader optimising "G3" would be optimising a wait for the box to calm down.

This is lesson #4 in STATUS.md firing for the fifth time — **every number has a harness, and the
harness is part of the number** — and it is worth recording because it fired on the instrument built to
hunt wall bloat. The honest headline is the warm one: **67s**, banked at the tick-546 landing.

New this window: **the retry loop was measured insufficient twice at t545/t546.** Both ticks reported
`manuk-shell tests FAILED` after all three retries, and both were green on a standalone re-run minutes
later at load1 ≈ 1.9 (t546 confirmed 70 passed / 0 failed). The retry waits ≤60s for load1 < 2.5; this
box carries a persistent background load of ~3–5, so the wait expires and all three attempts run
contended. Recorded for the observer — the wait budget and the load threshold both live in
`scripts/verify.sh`, which is DONE and OBSERVER-OWNED, and the agent does not touch it. Cost this
window: two full-wall re-runs.

Against the four rigor-preserving axes: unchanged from #12/#13. REDUNDANCY (share one SpiderMonkey
runtime across JS gates — cargo-nextest), PARALLELISM, CACHING, SCOPE — every admissible lever still
lands in `scripts/verify.sh` + the Cargo/build config, all observer-owned. Coverage GREW this window
(t546 added the `manuk-wpt test262` runner and its 6 unit tests, plus 3 new runtime assertions in
manuk-js) at **~0 marginal warm wall**, because neither crate's tests are in the wall's crate list —
which is itself the standing note from `gates-not-in-the-wall`: gated ≠ watched.

NO TRIM. **The wall is lean on the axes the agent may touch.** Mark untouched (189, ceiling 245).

## Audit #15 — tick 567 (wall 62s warm)

Ran `./scripts/wall-audit.sh run` on a quiet box. **Total 62s** — the leanest reading recorded in this ledger.
Breakdown: **T 22s (35%)** · P 14s (23%) · G6 8s · G1 4s · F 2s · F4 1s · B 1s · every named gate ≤1s. Well
under the 189s mark and the 245s ceiling.

**The comparison with Audit #14 (tick 547) is the finding, and it is about the instrument, not the wall.** #14
reported `G3 62s (49%)` of a 126s run and I noted then that G3's number *was the flaky-gate retry loop firing*,
not the gate. This run confirms it from the other side: **G3 does not appear in the breakdown at all** — the
`manuk-shell` suite passed first time, so its cost vanished into the noise floor, and the total halved. Same
tree, same gates, same machine; the only variable was contention. **Two audits, 126s and 62s, and neither
number is "the wall" — the honest statement is a range with a cause, and the cause is scheduling.**

That also settles the self-audit item raised at t564 (*"verify wall: 644s EXCEEDS the 300s target"*): 644s was
the contended tick-563 landing. Measured warm walls across this session: **62 · 65 · 76 · 85 · 86 · 104 · 119 ·
120 · 126 · 246 · 644s.** The distribution is not a code property.

⚠ **Recorded for the observer, unchanged from #14 and now with a second mechanism.** (a) verify.sh's flaky-gate
retry waits ≤60s for `load1 < 2.5` while this box idles at 3–5, so all three attempts can run contended —
`G_INTERACT`'s scaling assert (`shell/src/tab.rs:729`, a ratio of ~60µs sums) false-RED'd on **7 ticks** this
session, each landing on a warm re-run. (b) `status-update.sh` declines to bank a wall whose receipt stamps
`load1 >= 3.0`, but **the gate phase creates that load itself** (~25 test binaries at `CARGO_BUILD_JOBS=8`), so
on this box every receipt stamps 4.5–6.5 and a poisoned number can never be replaced automatically — at t564 it
hard-blocked every commit until `LAST_WALL_TIME` was set by hand to a twice-measured 85s (journalled explicitly;
the MARK was not touched). Both live in `scripts/`, which is observer-owned, and neither was edited.

Against the four rigor-preserving axes: unchanged from #12/#13/#14. REDUNDANCY (share one SpiderMonkey runtime
across JS gates — cargo-nextest), PARALLELISM, CACHING, SCOPE — every admissible lever is in `scripts/verify.sh`
plus the Cargo/build config. Coverage GREW substantially this window (t546–t566 added the test262 runner,
6 fidelity/certificate bricks, 5 committed probe pages, and tests in manuk-text/manuk-css/manuk-wpt) at **~0
marginal warm wall**, because those crates' tests are not in the wall's list — the standing
`gates-not-in-the-wall` note, unchanged.

NO TRIM. **The wall is lean on the axes the agent may touch.** Mark untouched (189, ceiling 245).

## Audit #16 — tick 587

**230s total on a genuinely quiet box (`load1 0.32`)** — the first audit in a while taken under conditions
that measure the code rather than the machine. Nothing was trimmed. One number carries the whole result:

```text
 175s  P   parity          ███████████████████ 76%
  22s  T   crate tests     ██ 10%
  15s  B   build           █  7%
   7s  G6 · 5s G1 · 5s D · 2s F · 1s F4 · every named gate at or below 7s
```

**`P · parity` is 76% of the wall, and the cause is a false dependency.** `parity::run_parity` is a serial
`for` loop over ~30 fixture pages; each iteration calls `chrome::capture_boxes`, which **launches headless
Chrome**. 175s ÷ 30 ≈ **5.8s a page** — that is process startup, not box comparison. Nothing in the loop
carries state between pages, so they are independent and the serialisation buys nothing. This is audit
question #2 (*is the slowest section actually parallel, or serialised by a false dependency?*) with the
answer "serialised".

**The fix is bounded concurrency, and the bound is the whole difficulty.** This section's own comment in
`verify.sh` records why: *"Under load Chrome drops pages: the gate reported 65/65 probes — a 100% pass rate
— as a hard FAILURE, and that false RED is what kept two finished media ticks unlandable."* Unbounded
parallelism would run every wall at exactly the load that causes the drop, converting a slow gate into a
flaky one. **Coverage is sacred; trading 100s of wall for a gate that false-REDs is not an admissible
optimisation.** So the lever is named and scheduled, not taken: pick the bound empirically on a quiet box,
in its own tick, with the page-count floor watched across repeated runs.

**Everything else is already lean.** The gate wall proper — 25+ page gates, the JS conformance run, the
containment and interaction gates — totals under 30s combined. The three prior causes this ledger tracked
(hygiene-cron pruning mid-run, ramdisk incremental flush, feature thrash between target dirs) show no
signature in this reading: `B` at 15s means the incrementals survived, and the parity section did not swing
between warm and rebuilding.

⚠ **Standing item, unchanged from #14/#15 and still observer-owned:** `status-update.sh` declines to bank a
wall whose receipt stamps `load1 >= 3.0`, but the gate phase creates that load itself, so a genuinely fast
wall often cannot be banked. Measured this session: warm walls of **60 · 62 · 230 · 264 · 292 · 296s**, with
`load1` in the receipt ranging 0.3–6.5 on identical trees. The 85s standing mark remains honest.

Next wall audit due: tick 607.

## Audit #17 — tick 607 (wall 65s, `load1 ≈ 1.0`)

**Nothing trimmed, and for once that is the headline rather than a shrug: the wall is 65s against an 85s
banked mark, and audit #16's dominant cost is gone.**

```text
  22s  T   crate tests     ████████ 34%
  14s  P   parity          █████ 22%
   7s  G6 · 4s G1 · 4s D · 2s F · 1s F4 · 1s B · every named gate at or below 7s
```

**THE FINDING IS THE DELTA ON `P`, AND IT REVERSES AUDIT #16's WHOLE RESULT.** Twenty ticks ago parity was
**175s and 76% of a 230s wall** — a serial `for` loop over ~30 fixture pages at ~5.8s each, which #16
diagnosed as headless-Chrome process startup and named as the loop's biggest wall lever. It is now **14s
and 22%**, a **12× drop**, with no tick in this window having touched `verify.sh` (which is observer-owned
and which the agent must not edit). So either the snapshot cache is now serving those pages or the launch
path changed underneath; **either way the scheduled "bounded concurrency" tick #16 queued is no longer
worth taking, and this entry retires it.** Recorded explicitly because a named lever that quietly stops
being a lever is exactly the kind of stale priority §VI.3's third clause (t598) exists to catch — the board
would otherwise keep steering at a 175s problem that costs 14s.

**`T · crate tests` is now the largest single line (22s, 34%), and it is not a defect.** It is the whole
workspace's unit tests, which is the one section where cost scales with the thing we actually want more of.
Audit question #1 (redundancy — do two gates each stand up a SpiderMonkey runtime for overlapping
assertions?) is the only admissible lever pointed at it, and at 22s the ceiling on that saving is small
enough that the measurement would cost more than the fix returns. Not taken, and named as not-taken rather
than left as an open question.

**Nothing in the three historical wall-poison signatures shows up in this reading:** `B` at **1s** means
the incrementals survived intact (hygiene-cron did not prune mid-run, the ramdisk did not flush), and no
section swung between warm and rebuilding. The one caveat on the number itself: `load1 ≈ 1.0` is quiet but
not idle, so 65s is if anything a slight over-read.

**Admissible optimisations found: none.** Coverage unchanged, no gate dropped, no floor widened, nothing
moved to CI.

---

## Audit #18 — tick 627 (wall 67s warm, and that number is the problem)

**The script's answer: the wall is lean.** 67s total — `T` crate tests 23s (34%), `P` parity 14s (21%),
`G6` 7s, `B` 6s, `D` 5s, `G1` 4s, `F` 2s. Nothing redundant, nothing accidentally serialised, nothing
recomputed. **Nothing was trimmed and nothing should be.**

**AND THAT IS THE WRONG NUMBER, WHICH IS THE FINDING.** A wall audit measures a warm re-run. This
session banked **18 real wall readings** across 17 landed ticks, and they are bimodal:

```text
  docs-only ticks        gate  64 · 65 · 65 · 67 · 67 · 70 s
  any engine touch       gate 512 · 515 · 516 · 545 · 678 · 687 · 691 · 708 · 710 · 716 s
  the SAME tree, re-run  gate 64s      ← t610: 692s then 64s · t613: 512s then 64s · t615: 515s then 64s
```

**The median engine tick pays ~10x the audited wall**, and the audit cannot see it because by the time
it runs, the thing that cost the time has already been paid for.

**THE CAUSE, ISOLATED AT t610 AND CONFIRMED NINE TIMES SINCE.** The gate phase runs
`cargo run -p manuk-wpt --release` four times (parity, G1, G6, F). The workspace release profile is:

```toml
[profile.release]
opt-level = "s"
lto = true            # FULL fat LTO
codegen-units = 1
```

`manuk-wpt` is a **51MB** binary linking the whole engine. Any change to a crate beneath it invalidates
that link, and the relink lands **inside the gate phase**, where it is attributed to gate runtime —
`build_seconds: 33`, `unattributed_seconds: 692`.

**Three things this window proved about the trigger, each of which surprised me:**
1. **A test-only file counts.** t621/t622 touched `engine/page/tests/*.rs` and paid 710s / 691s.
2. **A REVERT counts.** t620 changed no engine source in its final tree — it reverted a parked
   subsystem — and paid 708s. Cargo sees a changed file, not a changed meaning.
3. **The second run is free.** Same tree, immediately after: 64s. Which is what makes the first reading
   look like a regression and is how t610 nearly had a ratchet mark loosened over it.

**THE RIGOR-PRESERVING SUGGESTION, and it is squarely in the audit's admissible list (#4, SCOPE).**
`manuk-wpt` is a **test harness**, not a shipped artifact. `lto = true` + `codegen-units = 1` is a
size/speed trade for the **browser**; the harness inherits it and pays the link cost on every engine
tick, for no shipped benefit. A `[profile.release.package.manuk-wpt]` override with `lto = "thin"` (or
off) and default `codegen-units` **changes no assertion, drops no gate, widens no floor, and moves
nothing to CI** — the four things the audit forbids. It buys the same checks for fewer seconds, which
is exactly what the audit asks for.

⚠ `Cargo.toml` and the profile are **harness/build configuration** — the observer's. This is a
measured recommendation with 18 data points, not an edit. **Two candidate confounders I could not
rule out and that should be checked before acting:** whether the parity/G1/G6/F numbers themselves
depend on release codegen quality (F1/F2 are perf floors — a slower harness binary could move them,
which would be a real trade rather than a free win), and whether `opt-level = "s"` alone already
dominates the link time.

**Nothing trimmed. LAST_WALL_AUDIT → 627. Next due: tick 647.**

## Audit #19 — tick 648 (722s, and 64% of it is invisible to this instrument)

**The script's answer, and the arithmetic that undoes it:**

```text
   180s  P (page gates)   25%       35s  B    5%       24s  T    3%
     8s  G6                1%        4s  G1   1%        4s  D    1%       2s  F   0%
  ────────────────────────────────────────────────────────────────────────────────
  attributed 258s   ·   TOTAL 722s   ·   UNATTRIBUTED 464s (64%)
```

**THE WALL-TIME AUDIT'S OWN ACCOUNTING DOES NOT RECONCILE.** The instrument built to find where the
wall goes cannot see where **most** of it goes. That is meta-instrument #3 — *"8 of 30 process
defects were caught by a number that did not add up, not by any gate"* — firing on the instrument
itself, and it is the same shape audit #18 found from the other direction (a warm re-run measuring
the wrong thing) one audit earlier.

**The cause is already named and dated, and this session confirms it to the tick.** Audit #18 found
`manuk-wpt` is 51MB under `lto = true, codegen-units = 1`, and any tick touching `engine/` pays a
release relink **inside the gate phase**, where the section timers do not observe it. Sixteen ticks
landed t633-648:

```text
  docs-only ticks          73 · 74 · 83 s        (t636, t638, t639, t645, t648)
  any engine/ touch       700 · 709 · 767 · 770 s (t634, t635, t637, t640-644, t646, t647)
```

Same wall, same gates, one variable — and the split is now three audits old.

**NOTHING WAS TRIMMED, and that is the correct outcome rather than a shrug.** The audit admits
exactly four optimisations: redundancy (a shared SpiderMonkey runtime / `cargo-nextest`),
parallelism, caching, and scope. **Every one of them lives in `scripts/verify.sh` or the build
profile, which are harness-owned.** There is no rigor-preserving trim available on the agent side,
and the inadmissible ones — drop a gate, widen a floor, sample instead of cover, move a check to CI —
are not on the table at any price. Reported, not touched, for the third audit running.

**What would make the next audit better:** the unattributed 464s is a *number*, not a mystery. Timing
the relink separately from the gate phase would move it from 64%-unknown to a named line item, and
only then can anyone say whether the wall is lean. Until that lands, "the wall is lean" is a claim
about 36% of the wall.

## Audit #20 — tick 668

```text
  t662  gate 897s · build 43s   (engine/js edit → release LTO relink)
  t665  gate 570s · build 33s   (engine/js edit)
  t667  gate 757s · build 36s   (engine/js + engine/page)
  332 gate files under engine/page/tests/   (+8 this session)
```

**Nothing trimmed, and the reason is the finding.** Against the four admissible questions:

1. **Redundancy** — not the cost here. **Every tick of this session touched `engine/js` or
   `engine/page`**, which forces a release-LTO relink *inside* the gate phase. That is the artifact
   audit #16 and the t610 journal already named; it is a property of *what was worked on*, not of the
   wall. A session of `engine/css` ticks would show a different number for the same wall.
2. **Parallelism** — gates launch concurrently under `CARGO_BUILD_JOBS=8` (mem-guarded down from 32
   cores); the perf floors are deliberately serial and must stay so.
3. **Caching** — incrementals live in RAM; live fetches are snapshot-cached. Nothing recomputed.
4. **Scope** — no gate builds more than it asserts on.

**This audit inherits #19's open item and cannot close it.** #19's parting note was that *"the
unattributed 464s is a number, not a mystery — time the relink separately from the gate phase, and
only then can anyone say whether the wall is lean; until then 'the wall is lean' is a claim about 36%
of the wall."* That remains exactly true, and this session is the strongest evidence for it: the
spread between t665's 570s and t662's 897s is **327 seconds inside the same unattributed bucket**,
across ticks that ran the same gates. Whatever moved, no line item names it.

So the honest verdict is **not** "the wall is lean". It is: *the wall's variance this session is
dominated by a cost no line item measures, and the audit is forbidden from making the number look
better by cutting coverage.* The instrumentation that would settle it lives in `scripts/verify.sh`,
which is **observer-owned — reported, not touched**, for the second audit running.

## Audit #21 — tick 689 (227s, and 77% of it is ONE serial loop)

```text
  175s  P  (parity, 72/72 vs headless Chrome)   ███████████████████ 77%
   23s  T  (crate tests)                        ██ 10%
   16s  B  (build)                              █  7%
    8s  G6 · 5s G1 · 4s D · 2s F · everything else 0s
```

**This audit does not end in "no line item names it", which is where #19 and #20 both landed.** The
dominant cost has a name, a file, and a mechanism, and the file is agent territory.

`tests/wpt/src/parity.rs` loops the **72 committed fixtures** and calls
`chrome::capture_boxes(&html, vw, vh)` **once per fixture, serially**. Each call spawns a full headless
Chrome; process startup is ~2.4s, and 72 × 2.4s ≈ 175s. The captures are **completely independent** —
nothing in fixture *n* informs fixture *n+1*, and the comparison against our own boxes happens after each.

That is admissible question **2 (PARALLELISM — "is the slowest section actually parallel, or serialised by a
false dependency?")**, and the answer is a false dependency. Bounded-concurrent captures take **175s → ~25s
with the identical assertion**: same live Chrome, same 72 fixtures, same 72 comparisons. No gate dropped, no
floor widened, no sampling, nothing moved to CI.

**Sized as its own tick, not trimmed here.** It is a change to the wall's core gate, and a wall audit that
edits the thing it is auditing in the same pass has no control.

### ⚠ CACHING Chrome's answers is REJECTED — on rigor, not on effort

The fixtures are committed and static, so Chrome's boxes for them look like a constant worth memoising, and
that would take `P` to ~0s. It is refused: it converts a **live** oracle into a **recorded** one, and *"a
gate whose expected value came from MEMORY tests the memory."* A Chrome update that changed a box would then
be invisible — which is the one thing the parity gate exists to notice. Written down so the next audit does
not re-derive it as a good idea.

**Nothing was trimmed. The wall is not lean, and for the first time in three audits the reason has an
address.**

## Audit #21 — CORRECTION (tick 694): the 175s was CONTENTION, not serial cost

Audit #21 read `P` at **175s** from the wall's per-section timing and attributed it to 72 serial headless-Chrome
spawns at ~2.4s each. The arithmetic worked, the conclusion was actionable, and **the baseline was wrong.**

Measured on a quiet box, same tree, same fixtures, back to back:

```text
  serial   (as audited)     14s     72/72 probes across 30 pages
  parallel (8 in flight)     4s     72/72 probes across 30 pages
```

**14 seconds, not 175.** Inside `verify.sh` the parity gate shares the machine with ~25 concurrently-launched
gate builds, and the per-section number it reports is that contention — not the cost of the work. The audit
instrument reads a wall-clock slice of a parallel wall and presents it as a line item, which is the same shape
as *"I widened the crawl from 4 jobs to 12 to make it finish sooner and watched the hang rate go from 12.5% to
49% on the same binary in the same hour."*

> **Every number has a harness, and the harness is part of the number.** Fifth occurrence in this project, and
> the first time it has caught a WALL-AUDIT finding rather than a capability one.

The change still stands on its own measurement — **14s → 4s, a real 3.5×, which is what 8-way concurrency buys
on a ~2s-per-capture workload** — and 3.5× is a normal number where 44× would have been a red flag worth
chasing. But the wall will **not** drop 150s, because most of that 150s is other gates competing for the same
cores. What the wall actually does is now an open measurement, and the next tick's own `P` line answers it.

⚠ **The admissible-optimisation list should gain a step 0:** *before optimising a line item, measure that line
item ALONE on a quiet box.* A contention-inflated slice makes the biggest number look like the biggest cost,
and those are different things.

## Audit #22 — tick 711 (242s, and the 71% line item costs THREE SECONDS)

The section table says:

```text
  172s  P · parity (72/72 vs headless Chrome)   ███████████████ 71%
   25s  T    21s  G6    16s  B    5s  G1    4s  D    2s  F    1s  F4    0s  the rest
```

**Measured alone, on a quiet box (load 0.27), same tree, same fixtures:**

```text
  $ time manuk-wpt parity
    TOTAL: 72/72 probes within tolerance across 30 page(s)
    elapsed: 3s
```

**Three seconds against a 172-second line item.** There is no parity optimisation to make. The 71% is
contention — the parity gate sharing 32 cores with ~25 concurrently-launched gate builds — and the
audit instrument reports a wall-clock slice of a parallel wall as if it were a line item's cost.

⚠⚠ **THIS IS THE SECOND TIME, AND THE FIRST TIME IT WAS ALREADY WRITTEN DOWN.** Audit #21 (t689) read
`P` at 175s and attributed it to 72 serial Chrome spawns; the **t694 CORRECTION** measured it at 14s
serial / 4s parallel and added an explicit *step 0: before optimising a line item, measure that line
item ALONE on a quiet box.* **Tick 710's wall-audit note repeated the retracted claim verbatim** —
"parity launches one headless Chrome per fixture, serially, and that is 71% of the wall" — because it
read the section table and did not run step 0. Retracted here.

The correction was three headers above the one I was appending to. *A ledger only protects you if you
read the entry above yours before adding the next one* — the same shape as t706's finding that a
build spec's unbuilt half is invisible to every ranking instrument, except this time the document was
the one I had open.

**Where the wall actually goes, then:** nowhere in particular. 242s of mostly-parallel gate builds on
32 cores, no single line item above a few seconds of real work. **The wall is lean and there is
nothing admissible to trim** — which Audit #21's own rules say is a fine result, and it is the result.

The one number worth watching is not a section but the whole: `LAST_WALL_TIME` 63s on a quiet box
against a 189s mark. The gate phase's 242s figure is itself a contended reading.

## Audit #23 — tick 733 (the wall is 75s; the number we quote is a BUILD)

```text
   24s  T · crate tests        ███████ 32%        5s  D · disk
   21s  G6 · clickability      ██████  28%        4s  P · parity
    5s  G1 · fidelity                             2s  F · perf floors
                                                  1s  B · build
                                              ≈ 75s of measured sections
```

**Nothing admissible to trim, and the protocol's four questions all come back empty.**
*Redundancy* — the two largest are `T` (crate unit tests, one binary per crate, already shared) and
`G6` (clickability over the fixture corpus); neither stands up a duplicate SpiderMonkey runtime the
other could share. *Parallelism* — the gates already launch concurrently and the perf floors are
serial **on purpose** (a benchmark sharing the machine is not a benchmark). *Caching* — incrementals
are in RAM, live fetches are snapshot-cached. *Scope* — no gate builds more than it asserts on.
Against a 300s target, at 75s, there is no candidate.

### ⚠⚠ AND THE NUMBER THE LOOP QUOTES IS NOT THIS NUMBER

The walls in this session ran **760–806s**. The sections above total **75s**. The difference is
**compiling and linking gate binaries after an engine edit** — a tick touching `engine/js` relinks
every affected test binary against mozjs. The docs-only ticks in the same session came in at
**65–66s**, which is this same wall doing no compilation.

So *"the wall"* is a **build measurement wearing a gate measurement's name**, and two things follow:

1. The self-audit item that went **red at t715 (589s) and green at t726** with not one line of work
   in between was never measuring leanness — it was measuring whether the previous tick had touched
   the engine. t726 said so at the time: *a number that flips on the weather is a schedule, not a
   threshold.*
2. Trimming *gates* cannot move it. The lever is the build, and the build is harness-owned.

⚠ `P · parity` measures **4 seconds** here. That is the **third independent confirmation** of Audit
#21's correction and Audit #22's retraction — the *"172s parity, 71% of the wall"* reading was
contention from a co-running fidelity sweep, never parity's cost. Three audits have now measured it
alone and got 3s, 3s and 4s. **It should stop being re-derived.**

**Trimmed: nothing. Found: the wall is lean and the metric is mis-named.** Harness-owned; recorded,
not acted on (V1-SCOPE PART VII).

## Audit #24 — tick 754 (the wall is 65s; the audit's cost is now invisible to the audit)

```text
   23s  T · crate tests        ████████ 35%       4s  P · parity
   14s  G6 · clickability      █████    22%       4s  D · disk
   13s  B · build              █████    20%       2s  F · perf floors
    5s  G1 · fidelity          █         8%       1s  F4
                                                  0s  every remaining gate
                                              ≈ 65s of measured sections
```

**Nothing trimmed, and the four questions come back empty again** — the same verdict as #23, on a wall
that has since got *faster* (75s → 65s) with three more gates in it.

*Redundancy* — the shell gates already share one `manuk-shell` invocation (t118 collapsed four concurrent
`cargo test -p manuk-shell` processes into one); `T` is the workspace suite, which is the assertion, not
overhead. *Parallelism* — gate sections launch concurrently and `F` is *deliberately* serial, because a
benchmark sharing the machine is not a benchmark; at 2s of 65s nothing has become accidentally serial.
*Caching* — `G1`'s live fetches are snapshot-cached (`.verify-cache`), incrementals are in RAM, and `B` at
13s is incremental. *Scope* — nothing builds more than it asserts on beyond `B`, which every later section
consumes.

At 65s the wall is **4.6× under its 300s Tier-0 ceiling**. Every candidate optimisation would buy a few
seconds in exchange for touching coverage, which the protocol forbids — and `scripts/` is observer
territory in any case.

### ⚠ THE FINDING WORTH CARRYING: THIS HISTOGRAM CANNOT SEE WHAT A TICK ACTUALLY WAITS ON

Every engine tick in the 745–754 window (`engine/css`, `engine/layout`, `engine/text`) forced a **full
release LTO relink**, and the wall runs *after* it: measured on this session's ticks, `total 937s` and
`total 839s` for engine ticks against `total 78s` and `total 132s` for instrument-only ones. The
histogram above reports the 65s and is blameless — but the number a tick waits on is dominated by a
rebuild the audit does not measure, and the ratio is **>10×**.

That is not wall bloat and there is nothing here to trim: it is the shape of the cost, recorded so a
future audit reading "65s, lean" does not conclude that a tick is cheap. If the loop ever wants tick
latency down, the lever is the **relink**, not this list.

### PROCESS NOTE (the reason this entry exists at all)

I first recorded this audit in `JOURNAL.md` and set `LAST_WALL_AUDIT` by hand in `STATUS.md` — and
`status-update.sh` immediately regenerated it back to 733, because the field derives from
`## Audit #N — tick M` headers **in this file**. The audit had been *done* and was about to come due
again on the next tick. Same shape as the ledger gotchas already recorded for the surface audit: **the
cadence reads a ledger, not the journal and not STATUS.** Record audits where their counter looks.

## Audit #25 — tick 775 (655s, and the audit that ran never reached the ledger the cadence reads)

**⚠ FILED LATE, AT TICK 776, AND THAT IS THE FIRST FINDING.** The audit was *performed* at tick 775 —
`scripts/wall-audit.sh run`, total **655s**, and its four-question verdict was written out in full in
`docs/loop/JOURNAL.md` under tick 775. It went into the **journal** and never into **this file**, which
is the only thing `status-update.sh` greps (`^## Audit #N — tick N`). So `LAST_WALL_AUDIT` stayed at
754, and the pre-flight blocked the next tick as *overdue* for work that had already been done.

That is memory's own rule firing again — **the LEDGER is what the cadence reads** — and it is worth a
line here rather than a quiet backfill: a report that is accurate, complete and filed in the wrong place
is, to every mechanical consumer, indistinguishable from a report that was never written.

### The measurement (tick 775, `wall-audit.sh run`, total 655s)

```text
   49s  T · crate tests        ██████    7%      3s  F · perf floors
   36s  B · build              █████     5%      1s  F4
   19s  G6 · clickability      ███       3%      0s  every remaining named gate
    6s  G1 · fidelity          █         1%
    5s  D · disk               █
    4s  P · parity             █
                                              ≈ 123s of NAMED sections — and ~530s that is not
```

**The named sections sum to ~123s; ~530s is the gate fan-out, spread thin across many small gates.**
The audit's own text names the cause: **~1.5s of every JS gate is SpiderMonkey runtime startup**, and
there are hundreds of them. That product — per-gate SpiderMonkey startup × gate count — is now the
single largest wall lever in the project.

### The four questions, and none of them yields anything admissible

*Redundancy* — the real cost is structural (the startup product above). The lever is `cargo-nextest`
(one shared test binary, harder parallelism), which is **`scripts/`, observer-owned**. *Parallelism* —
gates already launch concurrently under `CARGO_BUILD_JOBS`; the perf floors are deliberately serial,
which is correct. *Caching* — incrementals are on the ramdisk, live fetches are snapshot-cached.
*Scope* — the gates added in the 768–776 window (`g_user_timing`, `g_iface_surface_2`, `g_css_utf8`,
`g_doc_prototype`) are single-`#[test]` page gates on the **existing** `manuk-page` binary and the t775
layout regression went into `manuk-layout`'s existing suite, so none of them adds a build target.

**Nothing trimmed, and nothing should be**: every admissible saving is harness work this agent does not
own, and the inadmissible ones (drop a gate, widen a floor, sample instead of cover, launder to CI) are
refused by construction. Recorded for the observer, unchanged from #24's carry-forward and now
quantified: **per-gate SpiderMonkey startup × gate count ≈ 530s of a 655s wall, and it is `scripts/`-side.**

## Audit #26 — tick 796 (881s, and the audit's own table does not add up)

```
   184s  P (parity, 72/72 vs live Chrome)        █████ 21%
   120s  T (crate tests)                         ███ 14%
    40s  B (workspace build)                     █ 5%
    17s  G6 · 5s G1 · 4s D · 2s F · 1s G_IFRAME · then a column of gates reading 0s
   ────
   373s  attributed          881s  total          508s (58%) UNATTRIBUTED
```

**The accounting gap outranks anything in the ranking.** 58% of the wall is not in the table the
instrument prints, and the column of `0s` gates is the tell: each of those stands up its own test
binary and its own SpiderMonkey runtime, so a gate cannot cost nothing. This is #25's finding one turn
further — #25 named *"~530s is the gate fan-out, spread thin"* by SUBTRACTION; #26 is the same
subtraction on a bigger wall, and the subtraction is still the only way to see it. **Optimising the
21% while 58% is unnamed is optimising the wrong thing**, and this is meta-instrument #3's exact shape:
*8 of 30 process defects were caught by a number that did not add up, not by any gate.*

### The four questions

*Redundancy* — unchanged and still the lever: per-gate SpiderMonkey startup × gate count, whose fix is
`cargo-nextest` (one shared binary), which is `scripts/`-side. *Parallelism* — gates launch
concurrently; the perf floors are deliberately serial and must stay so. *Caching* — incrementals on
the ramdisk, fetches snapshot-cached. *Scope* — the four gates added this window
(`G_FORM_CONTROL_METRICS`, `G_CASCADE_LAYERS`, `G_LINE_BREAK_SOLIDUS`, `G_FLOAT_CONTAINING_BLOCK`,
`G_FLEX_ORDER`, `G_INLINE_BLOCK_BASELINE`) are single-`#[test]` page gates on the **existing**
`manuk-page` binary — no new build target, and each is the cheapest possible shape for what it asserts.

**Nothing trimmed, and nothing should be.** Every admissible saving is harness work this agent does not
own; the inadmissible ones are refused by construction.

⚠ **For the record: this wall read 63s at the start of the session and 881s at its end, on the same
machine.** The difference is not bloat — these ticks edit `engine/css` and `engine/layout`, the
shared-type crates that cascade furthest, so 881s is the *worst realistic tick* the Tier-0 item is
measured against (the 300s target is stated for that case). The audit that would settle it is the
attribution gap above, not a stopwatch.
