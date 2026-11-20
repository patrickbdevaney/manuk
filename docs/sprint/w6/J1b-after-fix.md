# W6.J1.b.a.a.a.a.d — re-run of the transition probe and the css-transitions drain after .a–.c

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Measurement only; no engine change in this node.

## Binaries

- **before** = `c4e30576` (the commit just before `.a`, `1bfcfdb4`). Built from the same tree with `engine/css`, `engine/js`,
  `engine/page` restored to `c4e30576` (the `tests/` additions in `.a`–`.c` are not part of the bin build). Copy kept at
  `/tmp/d-wpt/before/manuk-wpt`; not committed.
- **after** = HEAD `0df91d1a`, `target/release/manuk-wpt` built with `cargo build --release -p manuk-wpt`.

Both runs use `manuk-wpt wpt ... --timeout 120 --show-failures`, with `MANUK_MAX_DRAIN_MS` unset (default budget).

## 1. Probe `raw/J1b-drain-cause/probe-002.html` (2 s linear transition, sampled at 1 s and 2.6 s)

Raw output: `raw/J1b-drain-cause/probe-002.before.out`, `probe-002.after2.out`. Copied to `~/wpt/css/zzprobe/` for the run, then removed.

| property | binary | from | t0 (class change) | mid (1 s) | end (2.6 s) | transitionend |
|---|---|---|---|---|---|---|
| background-color | before | rgb(255, 0, 0) | rgb(0, 0, 255) | rgb(0, 0, 255) | rgb(0, 0, 255) | null |
| background-color | after | rgb(255, 0, 0) | rgb(255, 0, 0) | rgb(252, 0, 3) | rgb(252, 0, 3) | **null** |
| padding-left | before | 10px | 50px | 50px | 50px | null |
| padding-left | after | 10px | 10px | 10.359461px | 10.445042px | **null** |
| opacity | before | 1 | 0.2 | 0.2 | 0.2 | null |
| opacity | after | 1 | 1 | 0.99418205 | 0.99247044 | **null** |

Control `element.animate` mid opacity = 0.6 on both binaries (the script-animation path works; the CSS path does not).

**Gate reading.** Clause 1 (mid strictly between from and target, all three properties): **met**, on the after binary,
and not met on before (mid equals target). Clause 2 (a transitionend): **not met**, null for all three.
Clause 3, which is implicit in "mid(1 s)": after 1 s of a 2 s linear transition the expected progress is about 50%; the
measured progress is about 1% (background 3/255, padding 0.36/40, opacity 0.006/0.8), and it does not advance between
1 s and 2.6 s (end ≈ mid). So the transition is started, but it does not follow wall time.

Code read (not executed as a separate experiment): `tick_realtime()` is the only setter of the real-time clock, and
`flush_transition_events()` is reached only through `Page::tick_transitions` (engine/page/src/lib.rs:11321). The only
callers of `tick_transitions` are in `engine/page/tests/g_transition_events.rs`. A host or harness that does not call it
per frame never advances the clock and never dispatches the events. This is consistent with the numbers above and is the
first suspect for the follow-up rows.

## 2. css/css-transitions suite (120 runnable files)

Raw logs: `raw/J1b-drain-cause/css-transitions.before.log`, `css-transitions.after.log`. Per-file parse:
`perfile.before.tsv`, `perfile.after.tsv`, `perfile.diff.txt`.

| binary | subtests passing | subtests total | % | TH_TIMEOUT files |
|---|---|---|---|---|
| before (`c4e30576`) | 638 | 1544 | 41.3% | 21 |
| after (`0df91d1a`) | 637 | 3164 | 20.1% | 24 |

Pass count is flat (−1). The percentage fell because the denominator grew by 1620 subtests. The per-file logs show
more subtests reported for several files (e.g. `properties-value-*` now reports its full subtest set), which is the likely
source, but the split of the 1620 was **not** measured in this node, so the two percentages should not be compared.

Per-file changes that are visible in the logs (pass/total):

| file | before | after |
|---|---|---|
| properties-value-001 | 0 (0/560 FILE line) | 0/560 |
| properties-value-inherit-001 | 0 | 6/560 |
| properties-value-002 | 0/18 | 8/18 |
| properties-value-003 | 0/122 | 5/122 |
| properties-value-implicit-001 | 0/60 | 23/60 |
| properties-value-inherit-003 | 0/60 | 20/60 |
| non-rendered-element-001 | 3/4 | 2/4 (regression) |

`properties-value-001` (the 0/560 file under the drain-off arm of W6.J1.b.a) is still 0/560 under the default budget
after the fix. The per-file parse is approximate for files whose result line has no total.

## 3. Mutant

The gate asks for "revert one of .a–.c and mid(1 s) equals target again". This node reverted all three in one step: the
`before` binary is `c4e30576`, so `.a`, `.b` and `.c` are all absent. Result: mid equals target on all three properties,
and the events are still absent. The gate's clause 1 turns red on the mutant, so the probe can go red. Reverting one of
the three alone was not run (it would need two more full builds of about 4 minutes each).

## Verdict

Measured, not claimed: the transition now starts (mid strictly between on all three properties) but does not follow
wall time (≈1% at 1 s) and dispatches no transitionend in this harness. The css-transitions pass count did not move
(638 → 637 of a larger denominator). Follow-up rows: W6.J1.b.a.a.a.a.d.a (clock), .b (event delivery without a host tick),
.c (the non-rendered-element-001 regression and the denominator split).

## 4. Split of the +1620 denominator and the non-rendered-element-001 regression (node W6.J1.b.a.a.a.a.d.c)

chrome_ver=145.0.7632.116 (both runs). Tool: `scripts/attribute-subtest-split.py` (run as
`python3 -I scripts/attribute-subtest-split.py raw/J1b-drain-cause/css-transitions.before.log raw/J1b-drain-cause/css-transitions.after.log`).
Authority for totals is the `FILE <path> (p/t)` header, not the printed FAIL lines (see 4c).

### 4a. Per-file attribution of the header-reported totals

| file | before | after | dTotal |
|---|---|---|---|
| properties-value-001 | no header (perfile: 0) | 0/560 | +560 |
| properties-value-inherit-001 | no header (perfile: 0) | 6/560 | +560 |
| properties-value-inherit-002 | no header (perfile: 0) | 0/560 | +560 |
| shadow-root-insertion | no header (perfile: 0) | 0/1 | +1 |
| starting-of-transitions-001 | no header (perfile: 0) | 0/1 | +1 |
| zero-duration-multiple-transition | no header (perfile: 0) | 0/1 | +1 |
| transition-base-response-002 | 0/1 | no header (perfile: 0) | −1 |

Attributed sum: **+1682** over 7 files with a non-zero change (the other header files change pass count only, dTotal 0).

### 4b. Residual, not attributable from these logs

Summary: before 638/1544, after 637/3164, so dTotal = **+1620**. Attributed +1682, so the residual is **−62**, the net
change in subtests of files that print no header (all-pass or header-less files). Those files print no per-file total,
so the −62 cannot be split by file from these logs. Separately, the header-less files carry 372 total / 336 pass before
and 310 total / 274 pass after: **36 non-passing subtests in each run have no printed FAIL line and no header**. The
count is the same in both runs; their files are not located.

Verdict: attributed 1682 ≠ 1620, so the split is **incomplete** and the node stays open (falsifier clause). Follow-up
W6.J1.b.a.a.a.a.d.c.a takes per-file totals for every runnable file.

### 4c. Printed FAIL lines are not a complete per-file record

Header failing counts (t−p) against printed FAIL/Timeout lines, per file: before, 6 files disagree (e.g.
`events-001` header 0/9, 1 line printed). After, 9 files disagree, including `properties-value-001` (0/560, 39 lines
printed) and `properties-value-inherit-001` (6/560, 121 lines printed). The node note "the per-file FAIL lines are the
authority" does not hold for these files; the header is used above.

### 4d. non-rendered-element-001 (3/4 → 2/4)

- before (`c4e30576`): 3/4. One failing subtest printed: `Timeout — Transitions are canceled when an element is no longer rendered`.
  The gate's premise of two failing subtests in the before run does not hold; the header says one.
- after (`0df91d1a`): 2/4. Two failing subtests printed:
  1. `FAIL — Transitions do not run for an element newly rendered` (`assert_equals: No transition should run expected "rgb(0, 128, 0)" but got "rgb(255, 0, 0)"`). **This is the new regression.**
  2. `Timeout — Transitions are canceled when an element is no longer rendered` (also failing before).

So the regression is one new failing subtest: after the .a–.c change, a transition runs on an element that becomes rendered,
where the test expects none. Follow-up W6.J1.b.a.a.a.a.d.c.b.

## 4e. Per-file totals for every file (node W6.J1.b.a.a.a.a.d.c.a)

chrome_ver=145.0.7632.116. `MANUK_WPT_TOTALS=1` makes the wpt child print `  FILE <path>  (p/t)` for every file, clean ones
included (tests/wpt/src/main.rs). Logs: `raw/J1b-totals/{before,after}{,.2}.log` (before = c4e30576, after = 0df91d1a, each with
only that printer patch; `--show-failures --timeout 120`, host load ~30).

| run | before | after | dTotal | attributed | residual |
|---|---|---|---|---|---|
| run 1 | 638/2664 | 665/1544 | -1120 | -1120 (properties-value-001, properties-value-inherit-001, -560 each) | 0 |
| run 2 | 638/2104 | 594/1969 | -135 | -135 (z-index-interpolation -94, text-shadow-interpolation -41) | 0 |

The split is now always complete (residual 0): header-less files were the whole gap. Mutant: dropping one all-pass file's header from
the after log gives residual +1, exit 1; `--selftest` GREEN. **But the +1620 itself did not reproduce**: the subtest denominator
of the same binary moves by hundreds between runs (before 2664 vs 2104, after 1544 vs 1969) because the 560-subtest
properties-value files and the interpolation files report a different number of subtests when the run is cut off by the drain
deadline under load. So "+1620" was a load artefact of one pair of runs, not a property of the two commits. Gate clause
"attributed = 1620" cannot be met; BLOCKED with follow-up .a.
