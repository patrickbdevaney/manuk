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

## Raw HTML vs JavaScript: the 11 paths (node R0.4.a.b.a.a.a.a.a)

chrome_ver=145.0.7632.116. Gate: `scripts/usagov-fetch-has-test.sh`. Stored engine-fetch HTML: `docs/bench/engine-fetch/www.usa.gov.engine-fetch.html`
(sha256 34f9e0bd13c46c13ffa29001081090b570897d5bdf045bad9aebe3823d937ece, captured by `manuk-wpt fidelity` on 2026-10-25). `scripts/usagov-fetch-has.sh` parses it with Chrome's
JavaScript blocked (a CSP meta, since `--disable-javascript` does not stop scripts here) and checks each key.

<!-- raw-has:begin | class | key -->
| path key | class |
|---|---|
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)` | RAW |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)` | RAW |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)` | RAW |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/h4:nth-of-type(1)` | RAW |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(2)` | RAW |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(2)/address:nth-of-type(1)` | RAW |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(2)/address:nth-of-type(1)/div:nth-of-type(1)` | RAW |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(2)/address:nth-of-type(1)/div:nth-of-type(1)/a:nth-of-type(1)` | RAW |
| `body:nth-of-type(1)/footer:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(2)/p:nth-of-type(1)` | RAW |
| `body:nth-of-type(1)/header:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/span:nth-of-type(1)` | RAW |
| `body:nth-of-type(1)/header:nth-of-type(1)/div:nth-of-type(2)/div:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(1)/div:nth-of-type(1)/span:nth-of-type(1)/a:nth-of-type(1)` | RAW |
<!-- raw-has:end -->

## Chrome-received bytes (node R0.4.a.b.a.a.a.a.a.a)

chrome_ver=145.0.7632.116. `scripts/usagov-chrome-bytes.py` stores the main-document bytes Chrome itself receives (CDP `Network.getResponseBody`) and the post-JS DOM,
in `docs/bench/chrome-bytes/`. Two live captures: both Chrome-received files are byte-identical to the engine-fetch HTML (sha256 34f9e0bd13c46c13...), and in both
post-JS DOMs all 11 keys are present. So Chrome did not get different bytes, and page JS did not remove the nodes in these captures; the small oracle DOM (82.1) was
not reproduced (2 captures, both full-size). The 11 missing nodes in that one run came from something other than bytes or JS-removal that we have not caught.
Gate: `scripts/usagov-live-bytes-test.sh` (edited-bytes and removed-node mutants red).

## How often the small DOM shows up (node R0.4.a.b.a.a.a.a.a.a.a)

chrome_ver=145.0.7632.116. `scripts/usagov-chrome-bytes.py https://www.usa.gov/` run 20 times in a row (`docs/bench/chrome-freq/`).
For each run the 11 paths-diff keys were looked up in the post-JS DOM (`scripts/usagov-fetch-has.js`). Gate: `scripts/usagov-absent-freq-test.sh`.

<!-- absent-freq:begin | run | keys present | keys absent | dom sha | bytes sha -->
| run | keys present (of 11) | keys absent | dom sha (12) | bytes sha (12) |
|---|---|---|---|---|
| 10 | 11 | 0 | a650b484f51b | 34f9e0bd13c4 |
| 11 | 11 | 0 | 69504d6c8a7e | 34f9e0bd13c4 |
| 12 | 11 | 0 | 04aec89ffa06 | 34f9e0bd13c4 |
| 13 | 11 | 0 | 04aec89ffa06 | 34f9e0bd13c4 |
| 14 | 11 | 0 | 04aec89ffa06 | 34f9e0bd13c4 |
| 15 | 11 | 0 | a650b484f51b | 34f9e0bd13c4 |
| 16 | 11 | 0 | a650b484f51b | 34f9e0bd13c4 |
| 17 | 11 | 0 | d6eea14d914e | 34f9e0bd13c4 |
| 18 | 11 | 0 | 04aec89ffa06 | 34f9e0bd13c4 |
| 19 | 11 | 0 | a650b484f51b | 34f9e0bd13c4 |
| 1 | 11 | 0 | a650b484f51b | 34f9e0bd13c4 |
| 20 | 11 | 0 | a650b484f51b | 34f9e0bd13c4 |
| 2 | 11 | 0 | baf2422510d7 | 34f9e0bd13c4 |
| 3 | 11 | 0 | 04aec89ffa06 | 34f9e0bd13c4 |
| 4 | 11 | 0 | d6eea14d914e | 34f9e0bd13c4 |
| 5 | 11 | 0 | d6eea14d914e | 34f9e0bd13c4 |
| 6 | 11 | 0 | 78fe153b0f79 | 34f9e0bd13c4 |
| 7 | 11 | 0 | a650b484f51b | 34f9e0bd13c4 |
| 8 | 11 | 0 | d6eea14d914e | 34f9e0bd13c4 |
| 9 | 11 | 0 | a650b484f51b | 34f9e0bd13c4 |
<!-- absent-freq:end -->

Result: absent: 0/220 key-slots over 20 runs. All 20 Chrome-received byte files equal the engine-fetch sha (34f9e0bd13c4), and all 20 post-JS DOMs hold 11/11 keys.
Six distinct post-JS DOM shas (a650b484f51b x8, d6eea14d914e x4, 04aec89ffa06 x5, 78fe153b0f79, baf2422510d7, 69504d6c8a7e), so the DOM varies run to run but the 11 keys never drop out.
The 82.1 shape run (oracle sha a6a6db2122b8, 307 lines) was not reproduced in these 20 runs or in the 2 earlier captures: its absence is rare or depends on something this capture does not do (the tri-sweep oracle, its own wait point, or the viewport), and that is not established here.

## Small oracle DOM inside the tri-sweep oracle (R0.4.a.b.a.a.a.a.a.a.a.a)

12 fresh `tri-sweep.sh` runs of usa.gov alone (`--no-a11y --no-m2`), chrome_ver=145.0.7632.116; sidecars and oracle path files in `docs/bench/oracle-freq/`. Gate: `scripts/usagov-oracle-freq-test.sh`.

<!-- oracle-freq:begin | run | oracle sha (12) | keys present (of 11) -->
| run | oracle sha (12) | keys present (of 11) |
|---|---|---|
| 20261025-071059 | 2286d97a1714 | 11 |
| 20261025-071118 | 2286d97a1714 | 11 |
| 20261025-071136 | 2286d97a1714 | 11 |
| 20261025-071156 | 2286d97a1714 | 11 |
| 20261025-071213 | 2286d97a1714 | 11 |
| 20261025-071231 | 2286d97a1714 | 11 |
| 20261025-071250 | 2286d97a1714 | 11 |
| 20261025-071313 | 2286d97a1714 | 11 |
| 20261025-071333 | 2286d97a1714 | 11 |
| 20261025-071351 | a6a6db2122b8 | 0 |
| 20261025-071411 | 2286d97a1714 | 11 |
| 20261025-071430 | 2286d97a1714 | 11 |
<!-- oracle-freq:end -->

Result: small DOM (a6a6db2122b8) in 1/12 fresh tri-sweep runs (run 20261025-071351; 307 oracle lines, 0 of the 11 keys); the other 11 runs hit the larger DOM (2286d97a1714, 318 lines, 11/11 keys). Standalone Chrome captures (22) never lost the keys, so the drop is rare (~1 in 12) and arises in the tri-sweep oracle path, where the same run's engine fetch is byte-identical to the others; the specific cause (wait point, viewport, load order) is not yet established.
