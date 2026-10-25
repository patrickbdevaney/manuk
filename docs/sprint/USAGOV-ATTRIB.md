# USAGOV-ATTRIB — which side moved when usa.gov M1 SHAPE moves

Node R0.4.a.b.a.a.a. chrome_ver=145.0.7632.116 on every row. Gate: `scripts/usagov-attrib-test.sh`.

`scripts/tri-sweep.sh` now writes `tri-sweep-<ts>.attrib.tsv` beside each run: sha256 of the oracle's sorted
path+rect set (`oracle_sha256`) and of the HTML our own fetch returned (`engine_fetch_sha256`), per site.
Six fresh runs, one-site corpus, `--m1-timeout 120 --no-m2 --no-a11y`.

<!-- rows: run | status | cov | shape | vis | paths | oracle_sha(12) | engine_fetch_sha(12) -->
| run | status | cov | shape | vis | paths | oracle sha | fetch sha |
|---|---|---|---|---|---|---|---|
| 062009 | OK | 99.7 | 84.0 | 29.0 | 319 | 2286d97a1714 | 34f9e0bd13c4 |
| 062031 | OK | 99.7 | 84.0 | 29.0 | 319 | 2286d97a1714 | 34f9e0bd13c4 |
| 062051 | OK | 99.7 | 82.1 | 29.0 | 308 | a6a6db2122b8 | 34f9e0bd13c4 |
| 062109 | OK | 99.7 | 84.0 | 29.0 | 319 | 2286d97a1714 | 34f9e0bd13c4 |
| 062127 | OK | 99.7 | 84.0 | 29.0 | 319 | 2286d97a1714 | 34f9e0bd13c4 |
| 062144 | OK | 99.7 | 84.0 | 29.0 | 319 | 2286d97a1714 | 34f9e0bd13c4 |

## Result

- Two distinct oracle sha values (2286d97a1714 x5, a6a6db2122b8 x1); one engine-fetch sha across all six.
- Same oracle sha gives the same (cov, vis, paths, shape); the 82.1 / 308-path run is the one with the other oracle sha.
- The usa.gov SHAPE band (82.1 to 84.0) moves with the oracle's DOM; our fetched HTML did not change in these six runs.
  Why Chrome builds 308 vs 319 paths is not examined here.
