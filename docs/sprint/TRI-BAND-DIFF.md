# TRI-BAND-DIFF — why the cited 10-site M1 SHAPE (85.0–85.7) and today's band (71.3–71.7) differ

Node R0.4.a.b.a. chrome_ver=145.0.7632.116 on every row compared here. Gate: `scripts/tri-band-diff-test.sh`.

## Result

The cited run and today's runs measure different reachable sets, so the mean is taken over different sites.

- **Cited run `docs/bench/tri-sweep-20261025-013937.tsv`** (the run R0.4.a reported; the only file on disk with a 85.x
  mean): M1 timeout was the script default `M1TMO=30` (`scripts/tri-sweep.sh:24`). Only 3 of 10 sites finished M1 in
  30 s, so the mean is over 3 sites: (100.0 + 72.4 + 84.0) / 3 = **85.5**. This reproduces the cited 85.5 exactly.
- **Today's band** (`docs/bench/tri-sweep-20261025-{022514,023405,024255}.tsv`, `--m1-timeout 120`): 6 sites finish M1,
  mean **71.4**. Three of them are the same fast sites (blog 100.0, docs 72.4, usa.gov 82.3 today) and average 84.9; the
  other three (apple 49.2, github 74.3, youtube 50.0) average 57.8. Composition alone gives (3×84.9 + 3×57.8) / 6 = 71.4.
- **Usa.gov drift is small.** Its SHAPE moved 84.0 → 82.1–82.3 across runs on the same pin. Holding the 3-site
  composition fixed, the drift is about 1.7 points on one site, i.e. about 0.6 on the 3-site mean. It does not explain 14.

So the 14-point gap is **composition, not measurement drift**: a 30 s M1 timeout kept the three slowest-to-fidelity sites
out of the cited mean, and the 120 s timeout let them in.

## What is NOT established

- **85.0 and 85.7** (the other two cited values, `docs/sprint/loop.log` line 357) have no run file on disk. The only
  persisted 3-site run is 013937 (85.5). The sweep's scratch dir is overwritten per run (`scripts/tri-sweep.sh:25`),
  so the err logs of 013937 are gone and the 30 s cause is read from the default flag and the loop log, not from the
  per-site error text. A 3-site mean of 85.0 or 85.7 would need usa.gov at 82.6 or 84.6, which no persisted run shows.
  Treat those two values as unreproduced.
- Whether the sites that were UNREACHABLE at 30 s would have measured the same SHAPE at 30 s is not tested here.

## Per-site comparison

Cited line is in `tri-sweep-20261025-013937.tsv`; today line is in `tri-sweep-20261025-022514.tsv`. Shape is the
`shape` column (empty when the site was not measured).

| site | cited line | cited M1 | cited SHAPE | today line | today M1 | today SHAPE | changed | cause |
|---|---|---|---|---|---|---|---|---|
| blog | 013937.tsv:2 | OK | 100.0 | 022514.tsv:3 | OK | 100.0 | no | — |
| docs | 013937.tsv:4 | OK | 72.4 | 022514.tsv:2 | OK | 72.4 | no | — |
| ecommerce | 013937.tsv:3 | UNREACHABLE | | 022514.tsv:4 | BLANK | | label | R0.4.a.a classifier: amazon origin answers 202 with a zero-byte body (`docs/sprint/M1-EMPTY.md`) |
| forum | 013937.tsv:5 | UNREACHABLE | | 022514.tsv:5 | NAVFAIL | | label | R0.4.a.a classifier: navigation failure, not a timeout |
| corp | 013937.tsv:6 | UNREACHABLE | | 022514.tsv:6 | OK | 49.2 | reachable | M1 ran past 30 s; finished inside 120 s |
| finance | 013937.tsv:8 | UNREACHABLE | | 022514.tsv:7 | NAVFAIL | | label | R0.4.a.a classifier: navigation failure, not a timeout |
| gov_edu | 013937.tsv:9 | OK | 84.0 | 022514.tsv:8 | OK | 82.3 | shape | same URL, same pin: SHAPE drift 82.1–84.0 across today's runs; cause not established |
| code | 013937.tsv:7 | UNREACHABLE | | 022514.tsv:9 | OK | 74.3 | reachable | M1 ran past 30 s; finished inside 120 s |
| media | 013937.tsv:10 | UNREACHABLE | | 022514.tsv:10 | OK | 50.0 | reachable | M1 ran past 30 s; finished inside 120 s |
| news | 013937.tsv:11 | UNREACHABLE | | 022514.tsv:11 | BOTWALL | | label | R0.4.a.a.a.a classifier: bot-wall 403 |

## Machine-checked rows

The gate reads the rows below (site, cited line, cited status, cited SHAPE, today line, today status, today SHAPE)
and checks each against the TSV line it cites. It also recomputes the 3-site cited mean and the 6-site today mean.

<!-- rows: site | cited_file:line | cited_status | cited_shape | today_file:line | today_status | today_shape -->
| site | cited | cited_status | cited_shape | today | today_status | today_shape |
|---|---|---|---|---|---|---|
| blog | 013937:2 | OK | 100.0 | 022514:3 | OK | 100.0 |
| docs | 013937:4 | OK | 72.4 | 022514:2 | OK | 72.4 |
| ecommerce | 013937:3 | UNREACHABLE | | 022514:4 | BLANK | |
| forum | 013937:5 | UNREACHABLE | | 022514:5 | NAVFAIL | |
| corp | 013937:6 | UNREACHABLE | | 022514:6 | OK | 49.2 |
| finance | 013937:8 | UNREACHABLE | | 022514:7 | NAVFAIL | |
| gov_edu | 013937:9 | OK | 84.0 | 022514:8 | OK | 82.3 |
| code | 013937:7 | UNREACHABLE | | 022514:9 | OK | 74.3 |
| media | 013937:10 | UNREACHABLE | | 022514:10 | OK | 50.0 |
| news | 013937:11 | UNREACHABLE | | 022514:11 | BOTWALL | |
