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

## Audit #27 — tick 817 (1661s, and the accounting gap is now 73% — the THIRD audit running)

```
   236s  P (parity, 72/72 vs live Chrome)        ███ 14%
   117s  T (crate tests)                         █ 7%
    64s  B (workspace build)                     █ 4%
    10s  G6 · 8s D · 5s G1 · 3s F · 1s G_SELECTOR · 1s F4
          · then G_VIEWPORT, G_TEARDOWN, G_STALE_NODE reading 0s
   ────
   445s  attributed         1661s  total         1216s (73%) UNATTRIBUTED
```

**#25 named this by subtraction, #26 named it again, and it has widened every time: 508s (58%) at tick
796, now 1216s (73%).** The instrument's own table is the finding, for the third consecutive audit.

⚠ **The four rigor-preserving questions cannot be AIMED while this holds, and that is the point worth
recording rather than re-answering them.** Every one of *redundancy / parallelism / caching / scope* is
a question **about a named section** — and the largest cost on the wall has no name. Answering them
against the 27% that is labelled is optimising the wrong thing, which is exactly what #26 said in its
own words. The column of gates reading `0s` remains the tell: each stands up its own test binary and
its own SpiderMonkey runtime, so a gate cannot cost nothing, and whatever those startups really cost is
inside the unattributed 1216s.

### The four questions, answered only where they can be

*Redundancy* — unchanged and still the standing lever: per-gate SpiderMonkey startup × gate count,
whose fix is `cargo-nextest` (one shared test binary). `scripts/`-side, not this agent's to make.
*Parallelism* — gates launch concurrently; the perf floors are deliberately serial and must stay so.
*Caching* — incrementals on the ramdisk, fetches snapshot-cached. *Scope* — the three gates added this
window (`G_ROWLESS_TABLE` t815, `G_ORPHAN_TABLE_CELL` t816, `G_FLEX_PERCENT_LINEBREAK` t817) are each a
single `#[test]` on the **existing** `manuk-page` binary: no new build target, and the cheapest
possible shape for what each asserts.

**Nothing trimmed, and nothing should be.** Dropping a gate, widening a floor, sampling instead of
covering, or exiling a check to CI are all inadmissible by construction; every admissible saving is
harness work this agent does not own (PART VII).

⚠ **REPORTED, NOT PATCHED.** The actionable item for the observer is **instrument the unattributed
remainder before hunting bloat in the labelled 27%** — three audits have now produced the same
subtraction and no new information, which is itself the signal that the next audit under the current
timer will also produce nothing. A fourth reading of "58% → 73% is unnamed" is not worth its wall time.

⚠ **Also observed this window, and it is a measurement note rather than a regression:** `F2 pipeline
large/mid` read **8.11x (bar 7.5x) and FAILED the tick**, on a run where `large` was *unchanged*
(233.92 → 232.72 ms) and `mid` was **17% FASTER** (34.75 → 28.68 ms). The ratio degraded because the
DENOMINATOR shrank. A ratio gate exists to divide out machine speed, but it only does that when both
legs move together; when the small page benefits more from a quiet box than the large one, the control
moves the gate on its own. Recorded here for the observer as a gate-shape observation — **not** acted
on, since retuning a ratchet gate to land one's own tick is precisely what is forbidden.

## Audit #28 — tick 837 (925s, and the receipt now says the gap out loud: `unattributed_seconds: 925`)

```
  total (receipt `seconds:`)        925s      target 300s      3.1× over
  build_seconds                      26s      (incremental, RAM-backed — not the problem)
  load1 at the time                 7.66
  the audit's own histogram:
     P   parity 72/72 vs headless Chrome     230s   25%   ← largest NAMED cost
     T   crate tests                          42s    5%
     B   workspace build                      26s    3%
     G6 · G1 · D · F · F4 · the G_* wall      23s    2%
     ────────────────────────────────────────────────
     accounted                               321s   35%
     UNACCOUNTED                             604s   65%
```

**THE ACCOUNTING GAP IS THE FOURTH AUDIT RUNNING** (#26 73%, #27 73%, now 65%), and this time the
receipt states it in its own field rather than leaving it to be derived: **`unattributed_seconds:
925`** — the attribution mechanism attributed *none* of the wall, while the audit's histogram
attributes 321s of it. **Two instruments disagree about 604 seconds and one of them says, correctly,
that it does not know.** That is the reconciliation signal this project ranks above every gate
(META-INSTRUMENT #3: *8 of 30 process defects were caught by a number that did not add up*), and it
has now fired four times without being closed.

What the gap is *not*: it is not the build (26s, measured), and it is not a section the histogram
lists (they sum to 321s). It is time inside the wall that no section claims — which means either a
section runs outside the timing harness, or the sections are timed as CPU while the wall is
measured as clock, and at `load1 7.66` those diverge by exactly the factor observed.

**NOTHING WAS TRIMMED, AND NOTHING SHOULD BE UNTIL THE 604s IS NAMED.** Every admissible
optimisation the audit prompts for — share a SpiderMonkey runtime across JS gates, reuse one
headless Chrome across the 72 parity fixtures, narrow a whole-workspace build to one crate — targets
the 321s that *is* attributed. Optimising the attributed third while two thirds is unexplained is
tuning the part you can see because it is the part you can see. **The next wall tick is the
attribution, not the trim.**

⚠ PART VII: `verify.sh`, the receipt writer, and the timing harness are all observer-owned. This is
reported, not patched. The one thing an agent tick can say is which number to chase first, and it is
`unattributed_seconds`.

⚠ Also standing, from #27 and re-observed this window: **`F2` is a ratio gate that reddens from its
DENOMINATOR.** It failed at 7.70x on a loaded box and passed on a quiet re-run of the identical
tree; `mid` read 34.35ms on the red run, the fastest all session. Re-run, never retune — but it cost
a full 925s wall to learn that, twice in three windows.

---

## Audit #29 — tick 857 (wall 68s)

**Where the seconds go:**

```text
    27s  T  (crate tests)   █████████ 40%
    10s  G6 (clickability)  ███ 15%
     6s  G1 (fidelity)      ██ 9%
     5s  P  (parity 72/72)  █ 7%
     4s  D                  █ 6%
     3s  F  (perf floors)   █ 4%
     1s  F4 · 1s B · 0s G_VIEWPORT / G_TEARDOWN / G_STALE_NODE / G_SILENT_FAIL
```

**Verdict: the wall is lean, and nothing is trimmed.** 68s against a re-baselined mark of 189s and a
Tier-0 target of 300s. The audit's own note applies literally — *"an audit that finds the wall already
lean is a fine result — say so."* Working its four admissible questions in order:

1. **Redundancy** — `T` is 40% and is seven crate suites, each a distinct crate's unit tests. They do
   not stand up overlapping SpiderMonkey runtimes (the JS gates that do are the `G_*` binaries, which
   together total under 2s here). `cargo-nextest` remains the named future win and is still not worth
   a dependency at 68s.
2. **Parallelism** — the gates run concurrently under `CARGO_BUILD_JOBS`; `F`/`F4` are deliberately
   serial because a benchmark sharing the machine is not a benchmark. Nothing has gone accidentally
   serial: the largest single item (`T`, 27s) is itself seven suites.
3. **Caching** — incrementals are in RAM, live fetches are snapshot-cached. `G6` (10s) re-`curl`s the
   Wikipedia page each wall, which is the one recomputation left — and it is **load-bearing, not
   waste**: t853's regression was found precisely because G6 measures a *real page* that a fixture
   could not have reproduced. Caching it would trade the gate's whole value for 10 seconds.
4. **Scope** — no gate builds materially more than it asserts on; `B` is 1s.

⚠⚠⚠ **THE FINDING THIS AUDIT ACTUALLY HAS IS ON THE OTHER SIDE OF THE LEDGER, AND IT IS NOT A TRIM.**
The wall runs **19 of 104** `manuk-page` gates. Two real defects surfaced this window *only* because
t853 ran the whole crate for an unrelated regression sweep:

* **t854** — `g_reflect_numeric` did not fail, it **spun** (`user 2m57s` of a 3m00s cap) on an
  unclamped `colspan="2147483648"`. A Bar-0 hang, invisible for the gate's entire existence.
* **t855** — `static_import_scanner_…` had been **permanently red** since t624 superseded one clause
  of a five-clause `is_empty()`.

Neither is a wall-time problem and neither can be fixed by trimming. But *"the wall is 68s"* and
*"the wall checks 18% of the gates"* are the same sentence read from two ends, and only one of them
gets audited every 20 ticks. **Recorded as a coverage observation for the harness owner** — the
choice of which gates ride the per-tick wall is `scripts/` territory (PART VII), so this audit
reports it and does not touch it. A cheap middle path, if it is wanted: a rotating slice, or a
`--full` lane run off the tick path at the same cadence as this audit.

⚠ Coverage is sacred and nothing here proposes otherwise. No gate dropped, no floor widened, nothing
moved to CI.

## Audit #30 — tick 878 (wall 250s at t877; the dominant cost is diagnosed and half-fixed already)

```text
  187s  P · parity (72/72 vs headless Chrome)     75%
   28s  T · crate tests                           11%
   17s  B · build                                  7%
   10s  G6 4% · G1 5s · D 5s · F 2s · F4 1s · everything else 0s
```

**P is 75% of the wall, and audit #21 already found and half-fixed it.** That audit measured the same
gate at 175s of a 227s wall and named the cause: the loop spawned a full headless Chrome per fixture,
serially. The Chrome half is now captured **8-way in parallel** (`CHROME_JOBS` in `parity.rs`), which
is ~9 rounds × 2.4s ≈ **22s** of the 187. Caching Chrome's answers across runs was considered there
and **rejected on rigor** — correctly, and it stays rejected: *"a gate whose expected value came from
MEMORY tests the memory"*, and a Chrome update that changed a box is exactly what this gate exists to
notice.

**So the residue — ~165s, ≈2.3s × 72 — is the MANUK half, and it is serial by a constitutional
constraint, not by a false dependency.** Each capture runs our engine including SpiderMonkey, and this
project's standing rule is that **two JS contexts in one process tear down messily and segfault
nondeterministically** — the reason every `g_*` gate carries "one `#[test]` on purpose", and the
reason audit #21 parallelised only the Chrome side (Chrome is a separate PROCESS, so N of them are
safe by construction).

### Answers to the audit's four admissible questions

1. **REDUNDANCY — "share one SpiderMonkey runtime between gates".** ⚠ **PERMANENTLY INADMISSIBLE FOR
   THIS PROJECT, and recorded here so no future audit re-proposes it.** It is the first lever the
   audit script suggests and it is the one thing this codebase has a standing rule against. The ~1.5s
   of runtime startup per JS gate is not recoverable in-process.
2. **PARALLELISM.** The Chrome half is parallel (8). The manuk half is not, and the only admissible
   way to make it so is **subprocesses** — which is exactly what the Chrome half does and what
   `manuk-wpt fidelity --jobs N` already implements for the sweep. Same corpus, same assertions, no
   coverage lost, segfault rule respected by construction. **This is the open lever.** Not taken at
   t878: it is a real change to the parity runner and needs its own falsification (a `--jobs N` that
   silently drops a fixture would make the wall faster and the gate weaker, which is the failure mode
   this audit exists to refuse).
3. **CACHING.** Settled by audit #21 and not reopened.
4. **SCOPE.** T (28s) builds and runs whole-crate suites; B (17s) is the incremental build. Neither is
   the lever while P is 75%.

t878 added one `manuk-page` gate (`G_CLICK_POINT`); it contributes 0.44s of run plus its link, in T.

**Nothing was trimmed.** The wall is not lean, but its dominant cost has a known cause, a rejected
shortcut, and one open, rigor-preserving lever that is a build rather than a tuning.

## Audit #31 — tick 899 (wall 75s — already lean, and the interesting number is a VARIANCE, not a total)

```text
    30s  T (crate tests: css 28 · layout 125 · paint 22 · dom 11 · net 97 · agent 126 · shell 74)  40%
    13s  G6                                                                                        17%
     7s  B  ·  5s  P  ·  5s  G1  ·  5s  D  ·  3s  F  ·  1s  F4  ·  0s  everything else
   ────
    75s  total, against a 300s ceiling — 4x headroom
```

**Audit #30 (t878) reported 250s and named its dominant cost as the SERIAL manuk half of P (parity),
with subprocess parallelism as the open lever. That lever is still open and is no longer worth
taking: P is now 5s of 75.** The wall came down without it — audit #30's own half-fix plus whatever
else landed since — so the honest entry is that the ranked lever has been overtaken by events. It
stays recorded as available; it is not ranked.

### Answers to the four admissible questions

1. **REDUNDANCY.** `T` is ~483 crate tests in 30s — **not** dominated by the ~1.5s SpiderMonkey
   startup this audit's script keeps pointing at, because most of `T` is `manuk-agent` (126),
   `manuk-layout` (125) and `manuk-net` (97), none of which stand up a JS runtime. The
   shared-runtime idea remains **permanently inadmissible** for the JS gates (audit #30's standing
   note: two JS contexts in one process tear down messily and segfault nondeterministically), and
   here it would not have bought anything anyway.
2. **PARALLELISM.** Gates run concurrently; the perf floors (`F`) are serial **on purpose** — a
   benchmark sharing the machine is not a benchmark. Nothing has gone accidentally serial.
3. **CACHING.** Settled at audit #21, unchanged: incrementals live in RAM, live fetches are
   snapshot-cached.
4. **SCOPE.** No gate builds materially more than it asserts on at these magnitudes. `B` is 7s.

**Nothing was trimmed, and trimming a 75s wall would be theatre.**

### The one observation worth carrying forward — a VARIANCE inside the gate section

Across five consecutive ticks this session the gate section read:

```text
  t895   gate 885s   (the first tick that touched engine/js)
  t896   gate  98s
  t897   gate 101s
  t898   gate  75s   (docs-only)
  t899   gate  73s   (docs-only)
```

**885s is not a gate getting slower; it is a rebuild living inside the gate section.** A tick that
touches `engine/js/src/` invalidates every gate binary, and that cost is billed to `gate` rather than
to `build` (which read 33s on the same tick). The ratchet mark therefore prices a docs tick and an
engine tick as if they were the same shape, and an engine tick can look like a 10x wall regression
when nothing regressed.

⚠ **NAMED, NOT ACTED ON — this is harness territory (CONSTITUTION PART VII) and `scripts/` is
observer-owned.** Recorded here for the observer, with no change proposed and none made. The agent-side
adaptation that costs nothing: batch release rebuilds away from a tick landing so the two do not
contend.


---

## Audit #33 — tick 919 (2026-08-04)

**Total 70s**, against a banked mark of 189s and a Tier-0 ceiling of 300s. The wall is **lean**, and
the audit's own text says an audit that finds it lean is a fine result — so this one says so rather
than manufacturing a trim.

```text
    28s  T   (crate tests)   ████████████ 40%
    11s  G6                  ████ 16%
     6s  D                   ██  9%
     5s  P                   █   7%
     5s  G1                  █   7%
     3s  F   (perf floors)   █   4%
     1s  F4 · 1s B · the twenty-odd named gates at 0s each
```

**Against the four admissible questions:**

1. **REDUNDANCY** — `T` is 40% and is the only candidate worth naming: seven crate suites, each
   standing up its own binary. `cargo-nextest` shares the test binary and parallelises harder, and
   the self-audit has named it before. **Not acted on: `scripts/verify.sh` is observer-owned (PART
   VII).** Recorded for the observer with no change proposed.
2. **PARALLELISM** — the gates run concurrently under `CARGO_BUILD_JOBS`; the perf floors are
   deliberately serial (a benchmark sharing the machine is not a benchmark). Nothing has become
   accidentally serial: `F` is 3s, which is what it costs when it has the box to itself.
3. **CACHING** — incrementals are in RAM, live fetches are snapshot-cached. Nothing found.
4. **SCOPE** — no gate builds more than it asserts on; the twenty-odd named gates cost 0s each
   because they share the already-built `manuk-page` test binary.

⚠ **THE ONE HONEST OBSERVATION THIS AUDIT ADDS.** Every wall this window measured **63-70s warm** and
**930-1050s when a release rebuild was in flight**, a 15× spread that has nothing to do with the
gates. Four ticks in this window paid it, and one (`t918`) took a `manuk-shell` **false RED** from the
parallel-build race that passes 74/74 when run alone. **The wall's cost is dominated by whether a
build is contending with it, not by what it asserts** — which is the same finding as audit #32 and is
harness territory. The agent-side adaptation that costs nothing and was used here: land the tick, let
the wall own the box, and do release rebuilds between ticks rather than beside them.

---

## Audit #34 — tick 941 (2026-08-05) — 369s, and the growth is MINE

**Total 369s**, against a Tier-0 ceiling of **300s** — 23% over, and the first audit in this ledger
to find the wall genuinely past its target rather than lean. `t940`'s self-audit had already flagged
the same number independently.

```text
   235s  P   page gates          ████████████████ 64%
    58s  T   crate tests         ███ 16%
    38s  B   build               ██ 10%
    17s  G6                      █  5%
     5s  G1 · 4s D · 3s F · 1s F4 · the rest at 0s
```

⚠⚠ **The section that grew is `P`, and this window put six binaries into it.** `engine/page/tests/`
is now **403 files**, each its own test binary — its own link, its own process, its own SpiderMonkey
runtime start (~1.5s apiece by this audit's own note). Six are from ticks 930-935:
`g_intrinsic_min_max`, `g_intrinsic_min_max_cssom`, `g_intrinsic_flex_grid`,
`g_anonymous_table_row`, `g_table_row_height_distribution`, `g_inline_box_leading`.

Every one is a real ratchet tooth, RED-proven three ways, and would be added again. **The point of
recording it is that the cost is chosen rather than absorbed:** a loop adding ~6 gates per window to
a section that is 64% of the wall is choosing a slower wall every window, and the ledger should say
so in the same place it says the wall is slow.

**Against the four admissible questions:**

1. **REDUNDANCY** — `P` at 64% is the whole story, and `cargo-nextest` (shares the test binary,
   parallelises harder than `cargo test`) is the named remedy, as it was in #33 for `T`. **Not acted
   on: `scripts/verify.sh` and Cargo config are observer-owned (PART VII).**
2. **PARALLELISM** — gates already launch concurrently under `CARGO_BUILD_JOBS`; the perf floors are
   deliberately serial and must stay so. Nothing found accidentally serialised.
3. **CACHING** — incrementals already live in RAM, live fetches are snapshot-cached. Nothing new
   found recomputed.
4. **SCOPE** — a narrower per-gate build target is real and is a `verify.sh` concern.

⚠ **What was deliberately NOT done: consolidating the six new gates into fewer binaries.** It is the
only lever on the agent's side of the line and it is barred by a standing constraint — **one
`#[test]` per JS gate, or SIGSEGV** — because each loads a real `manuk_page::Page` with SpiderMonkey.
Trading a known-good gate architecture for wall seconds is what this audit's own last paragraph
forbids.

**Verdict: the wall is NOT bloated with redundancy — it is 403 gates doing 403 distinct things.** It
is slow for a good reason and a fixable one, and the fixable half is tooling that PART VII reserves
to the observer. Handed over with a 235s line item so `cargo-nextest` can be priced against a number
rather than a feeling.

## Audit #35 — tick 962 (2026-08-05) — 78s, and the number that moved is a VARIANCE

```text
   ══ WALL-TIME AUDIT @ tick 961 — total 78s ══
     31s  T          ████████████████████████████████████████  40%
     17s  G6         ██████████████████████                    22%
      8s  B          ██████████                                10%
      5s  P · 5s G1 · 5s D · 3s F · 1s G_FORM · 1s F4 · the rest at 0s
```

⚠⚠⚠ **THE HEADLINE IS THAT #34's 369s BECAME 78s WITH NO GATE REMOVED, AND `P` WENT 235s → 5s.**
Audit #34 measured `P` at 64% of a 369s wall and handed `cargo-nextest` to the observer with a
priced line item. Whatever changed since — build state, the observer's tooling, or contention on the
box during #34's measurement — **the section that was 64% of the wall is now 6%**, and `P` is where
403 test binaries live. This audit's job is to say which, and it honestly cannot from the receipt
alone: `sections` is a per-run file and the ledger keeps only the last one.

⚠⚠ **SO THE FINDING IS ABOUT THE INSTRUMENT, NOT THE WALL: A SINGLE-SAMPLE WALL NUMBER IS A
READING OF THE BOX AS MUCH AS OF THE CODE.** #31 already named this ("the interesting number is a
VARIANCE, not a total") and #34 then reported 369s as a growth trend on one sample. Both readings
were real; neither was a trend. **This same session watched a green wall take 1148s** (tick 962,
under a `manuk-css` edit that cascades the whole workspace) — 15× the 78s measured 40 minutes
earlier, same tree shape, same gates. The wall is bistable on load, exactly as the loop's own
`status-update.sh` comment says (`load1` past ~3 balloons gate runtime ~10×), and an audit that
reports one sample as a level is going to keep alternating between "lean" and "bloated".

**Against the four admissible questions, at 78s:**

1. **REDUNDANCY** — `T` (31s) and `G6` (17s) are 62% of the wall and share nothing: one is the text
   suite, the other is the clickability gate on a real page. No two gates found standing up
   overlapping SpiderMonkey runtimes for the same assertion. `cargo-nextest` remains the named
   remedy for `P` when `P` is large, and remains observer-owned (PART VII).
2. **PARALLELISM** — gates launch concurrently under `CARGO_BUILD_JOBS`; the perf floors (`F`) are
   deliberately serial and must stay so. Nothing found accidentally serialised.
3. **CACHING** — incrementals in RAM, live fetches snapshot-cached. Nothing new found recomputed.
4. **SCOPE** — a narrower per-gate build target is real and is a `verify.sh` concern.

⚠ **NOTHING TRIMMED, AND THAT IS THE RESULT.** 78s is well inside the 300s re-measure trigger, and
the one lever on the agent's side — consolidating gates into fewer binaries — is barred by the
standing *one `#[test]` per JS gate, or SIGSEGV* constraint. Two gates were ADDED this window
(`G_TAB_STOP`, `G_SELECT_LISTBOX`), both RED-proven, both chosen rather than absorbed, per #34's
rule that the cost be recorded where the slowness is.

**Handed over, so the next audit is not another single sample:** the honest instrument for this
ledger is `sections` banked PER RUN with `load1`, not the last run overwritten. That is a
`verify.sh`/`status-update.sh` change and is observer territory; recorded here rather than acted on.

## Audit #36 — tick 983 (2026-08-06) — 1216s, and the receipt field built to explain it reads ZERO by construction

```text
  total 1283s · gate 1216s · build 67s · disk 62% · load1 8.83

  Where the seconds go, as the histogram reports them:
     237s  P    parity (§1.1 — 72/72 vs headless Chrome)   19%
     101s  G3   affordance completeness (§1.8)              8%
      88s  T    crate tests                                 7%
      67s  B    build (workspace)                           6%
      14s  G6 · 9s D · 6s G1 · 3s F · 1s F4                 2%
     ────
     526s  ATTRIBUTED (43%)          690s  MISSING (57%)
```

⚠⚠⚠ **THE HISTOGRAM ACCOUNTS FOR 43% OF THE WALL, AND THE MISSING 57% IS EXACTLY THE BLIND SPOT
`verify.sh` DOCUMENTED FOR ITSELF AT TICK 235 — WITH THE INSTRUMENT IT BUILT TO CLOSE IT REPORTING
ZERO.** The comment at `scripts/verify.sh:156` names it in full: `head_` only records time *between*
section headers, so the `cargo test --no-run` prewarm and the ~23 concurrent `_launch` gate
invocations — everything before the first `head_` at line 194 — are attributed to nothing. The fix
was a receipt field, `prewarm_launch_seconds`. It reads **0**, on this run and on t981's:

```text
   .git/manuk-verify-receipt:  seconds: 1216   build_seconds: 67   prewarm_launch_seconds: 0
                               unattributed_seconds: 1216      ← the ENTIRE gate wall
```

**Why it is zero, precisely:** `_PREWARM_END=$SECONDS` is assigned at `verify.sh:102`, inside the
prewarm loop — and `_PREWARM_END=0` is executed at **line 163**, sixty-one lines *later* in the same
top-to-bottom script. The initialiser runs after the assignment and clobbers it. So line 712's
`unattributed_seconds: $(( SECONDS - _BUILD_SECONDS - _PREWARM_END ))` degenerates to
`total − build` — the whole gate wall, by construction, on every run that has ever been recorded.

That is the answer to a question the loop has now asked twice and could not resolve: t981's self-audit
read *"the receipt attributes NONE of the 1118 seconds"* and correctly concluded that every remedy the
audit names is a **build-time** remedy against a 47-second build — but it could not say where the
other 1071s went, because the field that would say is dead. **A diagnostic that returns a constant is
worse than an absent one: t981 reasoned from `unattributed = everything` as if it were a measurement.**

⚠ It is `scripts/`, it is observer-owned, and PART VII forbids me to touch it. **One line here and on
with browser work** — but the line is now actionable rather than a shrug: moving `_PREWARM_END=0`
above line 97 (or deleting it) makes the receipt localise the 690s on the next run, and costs nothing.

**Against the four admissible questions, at 1216s:**

1. **REDUNDANCY** — `P` (237s, the largest attributed cost) is 72 parity cases against headless
   Chrome; coverage is sacred and none of it is duplicated elsewhere. `G3` (101s) and `T` (88s) share
   nothing. No two gates found standing up overlapping SpiderMonkey runtimes for the same assertion.
2. **PARALLELISM** — the ~23 `_launch` gates are concurrent; `F`/`F4` are deliberately serial and
   must stay so. Nothing found accidentally serialised. ⚠ But note: **the concurrency is precisely
   what the histogram cannot see**, so "is the parallel section actually parallel" is not answerable
   from this instrument at all until the field above is fixed.
3. **CACHING** — incrementals in RAM, live fetches snapshot-cached. Nothing new found recomputed.
   `load1: 8.83` on this run against `78s @ load1 low` at #35 — the bistability #35 named is intact
   and this sample sits at the loaded end of it.
4. **SCOPE** — narrower per-gate build targets remain real and remain a `verify.sh` concern.

⚠ **NOTHING TRIMMED, AND THAT IS AGAIN THE RESULT — but for a new reason.** #35 found the wall lean
and could say so with confidence. This audit finds 57% of the wall *unmeasured*, which is not the
same as finding it lean: there is no admissible trim to propose against seconds nobody can name. The
one actionable item this audit produces is the one-line ordering fix above, and it is not mine to
make. The trend across the last four samples — 78s (t962) · 369s (t941) · 1118s (t981) · 1216s (t983)
— reads as growth only if the bistability is ignored; `load1` should be read beside every one of them.

## Audit #37 — tick 1003 (2026-08-07) — 1332s, and the confession I was about to repeat is FALSE

```text
   G3  (affordance / full shell suite)   114s     9%
   T   (crate tests)                      91s     7%
   B   (build)                            69s     5%
   G6 · D · G1 · P · F · …                36s     3%
   ────────────────────────────────────────────────
   attributed                            310s    23%
   UNATTRIBUTED                         1022s    77%
```

⚠ The `_PREWARM_END=0` defect audit #36 localised is **unchanged** and now hides an even larger share
(77%, up from 57%). Still one line, still `scripts/`, still not mine. Reported, not touched.

⚠⚠⚠ **MEASURED RATHER THAN INFERRED — the biggest hidden term is LINK TIME, and it is per-gate.**
`touch engine/layout/src/lib.rs`, then build exactly the 19 `manuk-page` gate binaries `verify.sh`
launches:

```text
   relink of the wall's 19 manuk-page gate binaries after ONE engine/layout edit    100s
   → ~5.3s per gate binary, each ~125 MB (SpiderMonkey statically linked into every one)
   → 24 linked test executables on disk total 3.0 GB
```

Paid on **every tick that touches a shared crate** — which is every layout tick.

⚠⚠⚠ **AND THE FINDING I DID NOT EXPECT: THIS WINDOW'S FIVE NEW GATES COST THE WALL NOTHING.** Audit
#34 concluded *"the growth is MINE"*, and I came into this one ready to write it again with five more
gates as the evidence. One grep refutes it:

```text
   g_cascade_logical_physical · g_border_collapse · g_float_wrap_containing_block
   g_margin_collapse_through · g_float_after_inline_text        — none launched by the wall
```

`verify.sh` launches **24** things, 19 of them `manuk-page` gates, from a hand-curated list that has
not changed. A new gate file is compiled only when someone asks for it. **Adding a gate does not tax
the wall; adding a gate TO THE LAUNCH LIST does — and that list is observer-owned.**

> **A confession is a claim and it needs a measurement like any other.** #34's sentence was true when
> written; inheriting it as a standing fact about a window where it is false would have argued for
> precisely the thing this audit forbids — dropping gates.

⚠⚠ **427 gate files, 19 in the wall.** The standing `gates-not-in-the-wall` fact, and it cuts both
ways: the wall is cheap *because* it is narrow, and 408 gates are regression-checked only when
something runs them. Enlarging the list is seconds-for-coverage, which only the owner may price.

**The four questions:** (1) REDUNDANCY — ~1.5s of SpiderMonkey startup × 19 ≈ 29s, recoverable with a
shared test binary (`cargo-nextest`) without making any gate less independently failable; harness.
(2) PARALLELISM — the 24 launches are concurrent, `F`/`F4` deliberately serial; nothing accidentally
serialised. (3) CACHING — incrementals on a ramdisk, fetches snapshot-cached; the uncached cost is the
**link**, and the rigor-preserving fix is a faster linker (mold/lld), not a smaller check. (4) SCOPE —
gates already build one crate's test target, not the workspace.

**NOTHING TRIMMED, AND THAT IS THE RESULT.** Every optimisation the numbers point at lives in
`scripts/` or the workspace build profile. The one an engine tick could make — fewer, larger test
binaries — is blocked by its own measured constraint: **one `#[test]` per JS gate, because more than
one per binary SIGSEGVs** on SpiderMonkey teardown.

## Audit #38 — tick 1024 (2026-08-08), wall total **87s**

```text
   32s  T                 37%    (the cargo-test bucket)
   20s  G6                23%    clickability
    6s  G1                 7%    real-site fidelity
    6s  D                  7%
    5s  P                  6%    parity, 31 pages vs headless Chrome
    3s  F                  3%    perf floors (deliberately serial)
    1s  F4 · 1s B          2%
   ────────────────────────────
   74s attributed · 13s (15%) unattributed
```

⚠⚠⚠ **THE HEADLINE IS THE ONE THIS LEDGER HAS BEEN WAITING FOR: 87s AGAINST A 300s TIER-0 TARGET — 29%
OF BUDGET — AND THE UNATTRIBUTED SHARE IS 15%, DOWN FROM 77% AT AUDIT #37.** ⚠ **Stated as a
measurement, not a fix I can claim**: audit #37's 77% was dominated by the `_PREWARM_END=0` defect it
localised to one line of `scripts/`, and this audit does not establish whether that line changed or
whether this run was simply warm where #37's was cold. Both walls in this session read `gate 87s ·
build 1s` — a **build of 1s** says warm, and #37's 1330s total says cold. **The honest reading is that
the two audits measured different thermal states, and the 15% is a WARM-wall number that must not be
diffed against a cold one.** That is the same class as the sweep denominators (t1022): a comparison is
only a comparison when both sides were produced the same way.

### The four questions

**(1) REDUNDANCY** — unchanged from #37: ~1.5s of SpiderMonkey startup × 19 launched `manuk-page`
gates ≈ 29s, recoverable with a shared test binary without making any gate less independently
failable. Still `scripts/` + the build profile. Still **not mine to take.**

**(2) PARALLELISM** — nothing newly serialised. `F`/`F4` are deliberately serial (a benchmark sharing
the machine is not a benchmark); the 24 launches remain concurrent.

**(3) CACHING** — unchanged: the uncached cost is the **link**, and the rigor-preserving fix is a
faster linker, not a smaller check.

**(4) SCOPE** — unchanged; gates build one crate's test target.

### ⚠⚠ The one thing this window ADDED to the wall, and it was priced rather than assumed

Tick 1020 added `tests/wpt/corpus/media-interaction.html` — a fifth parity page-worth of probes, and
the first thing this loop has put **into** the wall in some time. Measured directly, two runs each
side, same box, same minute:

```text
   parity WITH  media-interaction (31 pages)    3.94s · 3.84s
   parity WITHOUT it              (30 pages)    3.80s · 3.89s
```

**Zero, and the ranges overlap.** ⚠ That is not luck and it is worth writing down as a rule, because
it tells the loop how much more of this it can afford: `parity.rs` bounds Chrome at **`CHROME_JOBS =
8` in flight**, so 30 pages and 31 pages are both **four launch rounds** (8+8+8+6 vs 8+8+8+7).

> **A parity fixture is FREE until it crosses a multiple of eight.** The marginal cost of the next
> page is zero up to 32; the 33rd buys a whole extra Chrome round. So the rigor-preserving way to add
> real-site coverage to the wall is to **fill the current round before opening a new one** — and the
> loop currently has **one free slot** (31 of 32).

This is the counterpart to #37's finding that *adding a gate does not tax the wall; adding it to the
launch list does*. Parity has no launch list — every `.html` in `tests/wpt/corpus/` is swept — so it
is the **one place an agent tick can add wall coverage without the observer**, and now it has a price
tag: free, eight at a time.

**NOTHING TRIMMED, AND THAT IS AGAIN THE RESULT.** At 29% of the Tier-0 budget the wall is not the
constraint on anything, and every optimisation the numbers point at lives in `scripts/` or the build
profile.

---

## Audit #39 — tick 1044 (2026-08-08), wall total **944s** — and the 944 is MINE, not a gate's

⚠⚠⚠ **THE HEADLINE NUMBER IS A COLD RELINK CAUSED BY THE OLD-BINARY CONTROL PROTOCOL, AND NOBODY HAD
EVER PRICED THAT.** Two walls, same session, same box, ten ticks apart in wall terms but ninety
minutes apart in clock terms:

```text
   tick 1042    gate 101s · build  1s · total  102s
   tick 1043    gate 944s · build 30s · total  974s
```

Nothing was added to the wall between them. What happened between them is the **t1043 pricing run**:
the old-binary control (t799's rule, and the one this loop trusts most) requires reverting
`engine/css`, rebuilding release, measuring, restoring, and rebuilding release again. `manuk-css` is
upstream of `manuk-layout`, `manuk-page` and `manuk-wpt`, so both rebuilds invalidate **every
`manuk-page` gate binary** — about 25 of them, each a ~350MB mozjs link.

**Measured directly, the twice-test on the settled tree:**

```text
   cargo test --no-run -p manuk-page --features stylo,spidermonkey    run1  604.25s
   …the identical command again                                       run2    0.82s
```

**604s of the 944 is one cold relink of the gate binaries, and the cache is healthy** — run 2 is
instant, so nothing is deleting the working set (the board's own twice-test, and it passes). This is
*not* the t846 wall-self-purge class and it is not standing bloat: it is a **one-shot cost the agent
imported into the wall by doing an out-of-band rebuild and then landing immediately.**

### The fix is free, it is entirely agent-side, and it is already written down

`memory/tick-landing-mechanics.md` step 1 says *"pre-warm out of band"* before landing a tick that
touches `engine/page`/`engine/js`. **The rule generalises and I did not apply it:** after the
old-binary control restores the tree, run

```bash
cargo test --no-run -q -p manuk-page --features stylo,spidermonkey
```

**before** `tick.sh`, and the 604s happens outside the wall instead of inside it. The wall then reads
its real number. Nothing about the gates changes; the seconds move off the measured path because they
never belonged on it.

> **AN OLD-BINARY CONTROL COSTS THE *NEXT* WALL, NOT ITS OWN TICK.** The protocol is two release
> rebuilds plus one cold gate relink — roughly 20 minutes — and all of it is invisible at the moment
> you decide to run the control. Price it into the decision: it is still worth paying (it changed the
> verdict three times, and twice more this session), but **batch it** — one control run can price
> several ticks' worth of fixes, and one cold relink then serves all of them.

### The four questions

**(1) REDUNDANCY** — unchanged from #37/#38: ~1.5s of SpiderMonkey startup × 19 launched `manuk-page`
gates ≈ 29s, recoverable with a shared test binary. Still `scripts/` + the build profile, still **not
mine to take.** ⚠ Note this window's own datum makes the case sharper: the same 25-binary set is what
costs 604s to relink, so a shared binary would cut both the startup tax *and* the relink tax.

**(2) PARALLELISM** — nothing newly serialised. `P` at 235s and `T` at 45s are the two real
in-wall costs; `F`/`F4` remain deliberately serial.

**(3) CACHING** — the finding above IS this question's answer for this window: the uncached cost is
the link, exactly as #37 and #38 said, and this is the first audit to catch the loop *creating* a
cold link for itself rather than inheriting one.

**(4) SCOPE** — unchanged; gates build one crate's test target.

### The parity budget, carried forward

#38 established that a parity fixture is **free until it crosses a multiple of eight** (`CHROME_JOBS
= 8`), with one free slot at 31 of 32. **Still 31 — nothing was added to `tests/wpt/corpus/` this
window.** t1043 and t1044 both put their new assertions into `engine/page/tests/g_form.rs`, an
already-launched gate, which is the other zero-cost place.

**NOTHING TRIMMED, AND NOTHING NEEDED TRIMMING.** The wall's standing cost is unchanged since #38;
the entire delta this window was self-inflicted, one-shot, and has a free agent-side remedy that is
now written down.
