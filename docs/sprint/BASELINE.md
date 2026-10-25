# R0.1 desktop baseline — this box

_Measured 2026-10-24 on `main` (HEAD c28d82bd + the sprint's untracked files). Machine: AMD Ryzen 9 5900X
(24 threads), 60 GB RAM, `/home` NVMe. Oracle Chrome: `MANUK_CHROME_PIN=145.0.7632.116` (the `CHROME_PIN` shim).
**chrome_ver = 145.0.7632.116 on every row below.** Note: GRAPH.tsv's R0.1 note says "Chrome here is 156"; that is
not what this box has. `google-chrome --version` says 145.0.7632.116, matching `docs/sprint/CHROME_PIN`. The 156
re-pin is node CHROME.156 and has not happened._

## 1. Build (cold)

| | value | source |
|---|---|---|
| command | `cargo build --release -p manuk-shell -p manuk-agent -p manuk-bidi` | — |
| starting state | no `target/` directory; no sccache; CARGO_BUILD_JOBS=16 | `ls target` before the build |
| crates compiled | 486 | `/tmp/r01/build.log` (cargo output, not committed) |
| wall time | **4 m 23 s** (264 s) | `/usr/bin/time -v`: "Elapsed (wall clock) 4:24.04" |
| result | `Finished release profile [optimized]`, exit 0 | same log |
| compiler warnings | 54 `warning:` lines, 0 errors | same log |
| `target/release/` size | **2.2 GB** (deps + bins + incremental) | `du -sh target/release` |
| `manuk` (shell, GUI) | 60.3 MB | `ls -l target/release/manuk` |
| `a11y-dump` (manuk-agent) | 51.6 MB | `ls -l` |
| `manuk-bidi` | 51.8 MB | `ls -l` |

Binary sizes are the unstripped release artifacts as cargo emits them. They are not a stripped-size claim.

## 2. Headless page load, manuk vs Chrome, 10 corpus URLs

**URLs:** every 20th non-comment line of `docs/bench/corpus-crux-trend.txt`, first 10 (`docs/sprint/BASELINE-URLS.txt`).

**Measured command:** `scripts/r01-baseline.py --runs 3 --engines manuk` drives `target/release/a11y-dump <url>`
(manuk's own page load plus the a11y build, headless, JSON on stdout). A 60 s timeout applies per run.
Memory is sampled every 50 ms across the process tree. **PSS** is the number to compare, because it splits shared
pages across their users. RSS summed over the tree over-counts.

Raw rows: `docs/sprint/baseline/runs.tsv` (30 rows, `chrome_ver` on each).

| url | status (run 1/2/3) | wall s (3-run range) | peak PSS MB (3-run range) |
|---|---|---|---|
| comix.to | OK/OK/OK | 8.98–9.81 | 134–134 |
| client.dkitrxmdwoqruvsi.net | OK/OK/OK | 0.37–0.38 | 11–30 |
| www.mobile.bg | OK/OK/OK | 11.40–11.85 | 192–197 |
| mkp.gem.gov.in | FAIL×3 | — | — |
| app.ordertime.com | OK/OK/OK | 4.80–4.98 | 265–267 |
| www.puentedemando.com | OK/OK/OK | 18.66–20.96 | 825–842 |
| basescan.org | OK/OK/OK | 0.26–0.38 | 31–38 |
| ebanking.bdm-sa.com | FAIL×3 | — | — |
| admin.zoomph.com | OK/OK/OK | 1.92–2.11 | 67–74 |
| virtual.lapampa.edu.ar | FAIL×3 | — | — |

**Band over the 7 OK urls, per run** (sum of peak PSS / sum of wall): run 1 1544 MB / 49.0 s; run 2 1568 MB / 47.6 s;
run 3 1556 MB / 49.7 s. Spread across runs: 1.5% on memory, 4% on wall.

**The three FAIL urls are real failures on this box, not timeouts from the harness:**
- `mkp.gem.gov.in` — `Connection refused (os error 111)` at 8 s, exit 1.
- `ebanking.bdm-sa.com` — `invalid peer certificate: UnknownIssuer` at 0.5 s, exit 1. A TLS trust gap in manuk.
- `virtual.lapampa.edu.ar` — `timed out after 30.0s` at 30 s, exit 1 (manuk's own 30 s limit).

Under R0.4 these should be split into NAVFAIL / TIMEOUT / CRASH. Right now they are all just FAIL.

**Variance note.** The very first run (before the `chrome_ver` column was added) saw comix.to at 497 MB PSS in run 1
and 133–135 MB in runs 2–3. The re-run above has no such outlier. The table reports the re-run, and the
first-run outlier is recorded here so that a single-run number is not quoted as a band.

## 3. Chrome arm — BLOCKED on this box

`google-chrome --headless=new --dump-dom <url>` **never exits here**, for every variant tried, including
`about:blank` and a local file. Measured with a 25–60 s `timeout`:

- `--headless=new --no-sandbox --disable-gpu --no-first-run --dump-dom about:blank` → hang (rc 124), no output.
- Same with `--single-process --no-zygote`, `--in-process-gpu`, `--disable-dev-shm-usage`, and the legacy `--headless` → hang.
- `--remote-debugging-port=9333` does come up: `curl /json/version` answers `Chrome/145.0.7632.116`. So Chrome itself
  runs, and the `--dump-dom` path is what hangs.
- stderr shows only the usual zygote and vaapi warnings.

Attempted in the full run (before the box was inspected): all 30 Chrome rows were `TIMEOUT` at 60 s. Those rows
measured a hung browser, not a page load, and **were discarded**. They are not in `runs.tsv`.

The Chrome column is therefore not measured. The follow-up is node **R0.1.a** in `GRAPH.tsv`. Its gate needs a
working headless load path (CDP over the DevTools endpoint, or a flag set that actually exits).

## 4. Gates proven to go red

The memory sampler was checked with inputs that must fail:

| input | expected | observed |
|---|---|---|
| python child allocating and touching 200 MB, sleeping 0.6 s | ≈200 MB peak | RSS 209 MB, PSS 205 MB ✔ |
| nonexistent binary as `--manuk` | status SPAWN, no crash | `(0.0, 0, 0, -1, 'SPAWN')` ✔ |
| `a11y-dump http://127.0.0.1:9/` (connection refused) | status FAIL, exit≠0 | `FAIL`, exit 1 ✔ |
| `MANUK_CHROME_PIN` unset | refuse to write rows | `sys.exit(...)` — not exercised in a run, read from the source |

## 5. What this does not show

- `a11y-dump` is the agent's page load and a11y build, not a rendered-page screenshot. It is the number to compare
  with a Chrome headless load of the same URL, and the Chrome arm is still missing.
- 10 URLs, 3 runs. 3 of the 10 failed on manuk, so the memory and time totals cover only the 7 pages that loaded.
- No claim in this file is "faster" or "lighter". Those words need a SCOREBOARD row with a Chrome comparison, which
  does not exist yet.
