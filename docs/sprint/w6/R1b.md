# W6.R1.b — Chrome target-count distribution: fly-unified and opendining, 5 repeat captures

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN; every capture JSON carries `chrome_ver: Chrome/145.0.7632.116`). Same
instrument as W6.R1 (`bench/agent-tasks/r1_agf.py capture`, 1024x768, 4 s settle, Chrome CDP accessibility tree plus
`agf-probe` on manuk). The repeat was written to a new directory (`--out docs/sprint/w6/raw/R1b`, a flag added to the
capture for this node) so the W6.R1 captures were not overwritten.

## Command

    MANUK_CHROME_PIN=145.0.7632.116 python3 -I bench/agent-tasks/r1_agf.py capture --runs 5 \
        --sites fly-unified opendining --out docs/sprint/w6/raw/R1b

Log: `docs/sprint/w6/raw/R1b/capture.log`. Exit 0; `loaded=True` and `rc=0` on all 10 captures.

## Count distribution (scorable Chrome targets, as in R1 `scored` and manuk `targets`)

| site | run | Chrome count | manuk count | source |
|---|---|---|---|---|
| fly-unified | 1 | 49 | 48 | R1b |
| fly-unified | 2 | 49 | 48 | R1b |
| fly-unified | 3 | 49 | 48 | R1b |
| fly-unified | 4 | 49 | 48 | R1b |
| fly-unified | 5 | 49 | 48 | R1b |
| opendining | 1 | 109 | 10 | R1b |
| opendining | 2 | 109 | 0 | R1b |
| opendining | 3 | 109 | 10 | R1b |
| opendining | 4 | 109 | 0 | R1b |
| opendining | 5 | 109 | 0 | R1b |

Earlier batch (W6.R1, 3 runs, for comparison; tabulated from `docs/sprint/w6/raw/R1/`):

| site | run 1 | run 2 | run 3 | manuk |
|---|---|---|---|---|
| fly-unified | 49 | 4 | 4 | 48, 48, 48 |
| opendining | 109 | 109 | 109 | 10, 0, 10 |

Per-site distributions:

| site | pooled captures | Chrome count distribution | modal | manuk distribution |
|---|---|---|---|---|
| fly-unified | 8 (R1 3 + R1b 5) | 49: 6, 4: 2 | 49 | 48: 8 |
| fly-unified, R1b only | 5 | 49: 5 | 49 | 48: 5 |
| opendining | 8 (R1 3 + R1b 5) | 109: 8 | 109 | 10: 4, 0: 4 |

## Falsifier

The node's falsifier: "a count that never returns to 49 means the run-1 count is the outlier". Computed by
`bench/agent-tasks/r1b_counts.py --ref fly-unified=49` (per batch, `run1_outlier` = run 1 equals 49 and no later run
returns to 49):

| batch | fly-unified sequence | run1_outlier | verdict |
|---|---|---|---|
| W6.R1 (3 runs) | [49, 4, 4] | True | RED for that batch, literally |
| W6.R1b (5 runs) | [49, 49, 49, 49, 49] | False | GREEN |

Reading. In the original batch, 49 never came back after run 1, so the literal rule fires there. The fresh repeat
then gives 49 in all five runs, including run 1. Across all eight fly-unified captures the value 49 occurs six times
and 4 occurs twice, both 4s in the original batch. The 4 is the value that did not reproduce, not run 1. The W6.R1
caveat that "run 1 is the outlier in target count" is therefore not supported: it was a per-batch reading, and the
repeat contradicts it. This node does not decide the cause of the 4-count (a transient in Chrome's AX tree on that
batch is the best reading; it was not investigated).

Opendining Chrome is 109 in all eight captures: no split. The manuk side is intermittent: 10 in four captures and 0 in
four, with no pattern across runs. That is a manuk-side failure on this replica, to be classified separately (it
matches the known REAL opendining issue noted in W6.R1).

## Controls (the tabulator must be able to go red)

`r1b_counts.py` was mutation-tested on copies of the R1b fly-unified captures in a scratch directory:

| mutant | construction | expected | result |
|---|---|---|---|
| A | run 1 = 49, runs 2–5 truncated to 4 targets | RED, exit 1 | RED, exit 1, `run1_outlier=True` |
| B | run 2 truncated to 4, runs 1 and 3–5 = 49 | GREEN, exit 0 | GREEN, exit 0 |
| real R1b | unmodified | GREEN, exit 0 | GREEN, exit 0 |

An earlier version keyed the verdict to the modal count, and mutant A stayed GREEN there (modal 4). That was a bug in the
tabulator, found by this mutation, and the rule was changed to the named reference count.

## Outputs

- `docs/sprint/w6/raw/R1b/fly-unified_r{1..5}.json`, `opendining_r{1..5}.json`, `capture.log`, `counts.txt`
- `bench/agent-tasks/r1b_counts.py` (tabulator and falsifier)

## Not measured

- Why Chrome gives 4 targets on fly-unified for two of three runs in the original batch.
- Why manuk gives 0 targets on opendining in half the captures.
- AGF on the R1b captures (not part of this node's gate).
