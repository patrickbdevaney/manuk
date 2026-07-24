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
