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

## The 11 paths (node R0.4.a.b.a.a.a.a)

chrome_ver=145.0.7632.116. `scripts/tri-sweep.sh` now keeps each usa.gov oracle path+rect file under
`docs/bench/oracle-paths/` (`KEEP_PATHS_RE`, default `usa\.gov`). Four fresh one-site runs captured both shas:
`tri-sweep-20261025-065726` / `-065745` / `-065805` = 2286d97a1714 (318 lines), `-065823` = a6a6db2122b8 (307 lines).
Gate: `scripts/usagov-paths-diff-test.sh` recomputes the table below from the stored files.

Result: all 11 keys are in the larger DOM and none is only in the smaller one. Nine sit in the footer's
`div:nth-of-type(2)` subtree (an `h4`, an `address` with a link, a `p`, and their wrapper divs); two are a `span` and its `a`
in the header. In the smaller DOM the header wrapper `div` has height 0 (was 44) and the footer `div:nth-of-type(2)` is 99px
tall (was 169), so the body is 69px shorter (3656 vs 3725). This names what Chrome drew in one build of the page and not the
other; why the origin or Chrome varies is not examined here.

<!-- paths-diff:begin | key | x | y | w | h | -->
| path key | x | y | w | h |
|---|---|---|---|---|
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)` | 66 | 3395 | 250 | 97 |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)` | 66 | 3403 | 250 | 89 |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)` | 66 | 3403 | 250 | 16 |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/h4:nth-of-type(1)` | 66 | 3403 | 250 | 16 |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(2)` | 66 | 3437 | 250 | 54 |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(2)/address:nth-of-type(1)` | 66 | 3464 | 250 | 27 |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(2)/address:nth-of-type(1)/div:nth-of-type(1)` | 66 | 3464 | 250 | 27 |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(2)/address:nth-of-type(1)/div:nth-of-type(1)/a:nth-of-type(1)` | 66 | 3466 | 250 | 23 |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(2)/p:nth-of-type(1)` | 66 | 3437 | 250 | 27 |
| `body:nth-of-type(1)/header:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/span:nth-of-type(1)` | 642 | 168 | 199 | 19 |
| `body:nth-of-type(1)/header:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/span:nth-of-type(1)/a:nth-of-type(1)` | 642 | 153 | 199 | 44 |
<!-- paths-diff:end -->
