# USAGOV-DRIFT — run-to-run band of usa.gov M1 SHAPE on the pinned chrome

Node R0.4.a.b.a.a. chrome_ver=145.0.7632.116 on every row here. Gate: `scripts/usagov-drift-test.sh`.

## Method

- Five fresh runs of `scripts/tri-sweep.sh` on a one-site corpus holding only `gov_edu https://usa.gov/`
  (`docs/bench/usagov-only-corpus.txt`, the usa.gov line of `docs/bench/oracle-corpus.txt`), `--m1-timeout 120 --no-m2 --no-a11y`, run back to back
  2026-10-25 06:13:12 to 06:14:50.
- Phase A (M1) is the same per-site code path the 10-site and 265-site sweeps use (`run_m1`, isolated `--out`
  per site, `timeout` wrapper). M2 and a11y are skipped because they do not feed SHAPE. The M1 timeout matches the
  120 s band in `docs/sprint/TRI-BAND-DIFF.md`.
- Each run persisted its own file under `docs/bench/`. Each row below cites one file and one line.

## Five fresh runs

<!-- rows: run | line | status | cov | shape | vis | paths | chrome_ver -->
| run | line | status | cov | shape | vis | paths |
|---|---|---|---|---|---|---|
| 061333 | 2 | OK | 99.7 | 82.1 | 29.0 | 308 |
| 061352 | 2 | OK | 99.7 | 84.0 | 29.0 | 319 |
| 061412 | 2 | OK | 99.7 | 84.0 | 29.0 | 319 |
| 061431 | 2 | OK | 99.7 | 82.1 | 29.0 | 308 |
| 061450 | 2 | OK | 95.6 | 82.3 | 31.6 | 319 |

Over these five SHAPE values:

- min **82.1**, max **84.0**, spread **1.9 points**, mean 82.9 (n=5). Values seen: 82.1 x2, 84.0 x2, 82.3 x1.

## Every usa.gov row on disk (11 runs, same pin)

The five fresh rows above plus the six persisted usa.gov rows from earlier sweeps on this pin
(013937 and 024255 and 054452, 020324, 022514, 023405). Each line's `cov`, `vis`, `paths`, and `shape` are
read from the file.

<!-- rows: run | line | cov | shape | vis | paths -->
| run | line | cov | shape | vis | paths |
|---|---|---|---|---|---|
| 013937 | 9 | 99.7 | 84.0 | 29.0 | 319 |
| 020324 | 8 | 95.6 | 82.3 | 31.6 | 319 |
| 022514 | 8 | 95.6 | 82.3 | 31.6 | 319 |
| 023405 | 8 | 99.7 | 82.1 | 29.0 | 308 |
| 024255 | 8 | 99.7 | 84.0 | 29.0 | 319 |
| 054452 | 8 | 99.7 | 84.0 | 29.0 | 319 |
| 061333 | 2 | 99.7 | 82.1 | 29.0 | 308 |
| 061352 | 2 | 99.7 | 84.0 | 29.0 | 319 |
| 061412 | 2 | 99.7 | 84.0 | 29.0 | 319 |
| 061431 | 2 | 99.7 | 82.1 | 29.0 | 308 |
| 061450 | 2 | 95.6 | 82.3 | 31.6 | 319 |

## What the band is made of

Across the 11 rows there are exactly three (cov, vis, paths, shape) signatures, and each signature always
carries the same SHAPE:

| signature (cov / vis / paths) | SHAPE | runs |
|---|---|---|
| 99.7 / 29.0 / 319 | 84.0 | 013937, 024255, 054452, 061352, 061412 |
| 99.7 / 29.0 / 308 | 82.1 | 023405, 061333, 061431 |
| 95.6 / 31.6 / 319 | 82.3 | 020324, 022514, 061450 |

So the 2-point band is not per-run noise on a fixed page. The `paths` column is the oracle path count (the
Chrome-side DOM the engine is scored against), and it moves between 308 and 319 with the signature. The
coverage and visual columns move with it. This points at the page content the two sides saw in each run, not at
a random error in SHAPE itself.

**Not established:** which side changed. The per-run oracle DOM is not stored, and the scratch dir is overwritten
per run, so the change cannot be attributed to the Chrome capture or to the engine's fetch from here. Follow-up
row R0.4.a.b.a.a.a stores that DOM per run.

## Consequence for the 10-site band

usa.gov contributes 82.1 to 84.0 to any mean that includes it. On the 3-site mean of R0.4.a.b.a, that is a
0.6-point effect (see `docs/sprint/TRI-BAND-DIFF.md`). The 14-point gap there is still composition.
