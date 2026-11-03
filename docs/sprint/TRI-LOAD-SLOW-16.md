# TRI-LOAD-SLOW-16 — do the 16 LOAD-SLOW sites drive under a looser M2 budget? (node R0.4.a.c.a.b)

Sweep: `docs/bench/tri-sweep-20261103-001932.tsv` (+ `.attrib.tsv`), run by
`scripts/tri-sweep.sh --corpus docs/sprint/TRI-LOAD-SLOW-16.corpus --jobs 2 --m1-timeout 120 --m2-timeout 180`
on `chrome_ver=145.0.7632.116`. Corpus = the 16 LOAD-SLOW sites of `TRI-FULL-M2.md` (wired.com excluded: it is an
engine fault, not a budget issue). The a11y phase ran at its default 90 s. Machine load during the run: ~13 on 16 cores.

Gate: `scripts/tri-full-gate.sh docs/bench/tri-sweep-20261103-001932.tsv 16 145.0.7632.116` → green (rows=16,
bad_rows=0). Mutants on copies of the file: 14 rows → red; pin altered on all rows → red; one empty m2_tag → red.

## Result

M1 (120 s): 15 of 16 reachable; pagerduty.com hit the M1 120 s limit and was SKIPPED for M2 and a11y.

M2 (180 s, 2 jobs): **10 of 15 measured OK**, 5 TIMEOUT, 0 NAVFAIL, 0 CRASH.

| url | sweep 054452 (90 s, 4 jobs) | this run (180 s, 2 jobs) | standalone re-drive (TRI-FULL-M2) |
|---|---|---|---|
| github.com | TIMEOUT | OK 83.5 | 54 s |
| reactjs.org | TIMEOUT | OK 2.8 | 18 s |
| nasa.gov | TIMEOUT | OK 66.7 | 32 s |
| rottentomatoes.com | TIMEOUT | OK 10.0 | 85 s |
| wikihow.com | TIMEOUT | OK 85.5 | 15 s |
| bonappetit.com | TIMEOUT | OK 56.1 | 37 s |
| berkeley.edu | TIMEOUT | OK 74.6 | 10 s |
| newyorker.com | TIMEOUT | OK 43.7 | 37 s |
| gizmodo.com | TIMEOUT | OK 66.2 | 20 s |
| wix.com | TIMEOUT | OK 57.4 | 61 s |
| salesforce.com | TIMEOUT | **TIMEOUT** (>180 s) | 78 s |
| time.com | TIMEOUT | **TIMEOUT** (>180 s) | 20 s |
| sentry.io | TIMEOUT | **TIMEOUT** (>180 s) | 29 s |
| tailwindcss.com | TIMEOUT | **TIMEOUT** (>180 s) | 15 s |
| newrelic.com | TIMEOUT | **TIMEOUT** (>180 s) | 50 s |
| pagerduty.com | TIMEOUT | SKIPPED (M1 120 s) | 14 s |

Headline as printed by the script (denominator = the 15 reachable): **M2 drive-rate FULL 36.4%** (n=15, 5 non-OK
scored 0); measured-only **54.6%** (n=10). The earlier full sweep reported FULL 63.3% over n=164, so the two are on
different denominators and are not comparable as a rate. The 16-site rows are compared per site above.

## Reading

- The looser budget does **not** clear all 16. 10 of the 16 now print a row. 5 still exceed 180 s with 2 jobs,
  although all 5 printed in 15-78 s standalone in TRI-FULL-M2. Under this run's load (~13 on 16 cores),
  contention is the likely cause. This run cannot prove it, because the M2 phase did not record per-site wall time.
- The 5 remaining sites are what the gap is made of after a looser budget: 5 of 15 reachable sites score 0 in M2 for
  a reason this run cannot name.
- The a11y column is unusable in this run: 14 of 15 a11y-score runs TIMEOUT at 90 s (F1 0), against 144 of 164 OK in
  the full sweep. This is an instrument/load effect, not a measurement of the sites. It is not part of this node's
  gate and is recorded as a follow-up.

## Caveats

- One run, not a band. Load was not controlled.
- The M2 denominator changed (15 reachable here, 164 in the full sweep). Quote the per-site rows, not a rate.
- pagerduty.com timed out at the M1 120 s limit here; a different M1 limit changes the reachable set.
