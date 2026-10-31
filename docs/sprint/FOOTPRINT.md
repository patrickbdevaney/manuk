# W4.E0 footprint: manuk vs pinned headless Chrome

_Measured 2026-10-31 on `main` at 146f026c, with `target/release/manuk-mcp` rebuilt from that tree (built 16:00 EDT).
**chrome_ver = 145.0.7632.116 on every row** (the pinned CfT build at `~/.local/chrome-cft/145.0.7632.116`; never the
system `google-chrome`). Raw rows: `docs/sprint/footprint/runs.tsv`. Summary (derived from the raw rows only):
`docs/sprint/footprint/summary.tsv`. Harness: `scripts/w4e0-footprint.py` (stdlib only; reuses the PSS walker and the
Chrome driver in `scripts/r01-baseline.py`)._

Every number below is one of: a 3-run min–max band from `summary.tsv`, or a value derived from one (pages/GB).
No claim is made beyond the bands. Nothing here is "faster" or "lighter" in the abstract.

## What was measured

| axis | what one run does | arms |
|---|---|---|
| pages | Hold N = 1, 10, 50 open pages in one tree. Summed PSS (`smaps_rollup`) sampled every 100 ms, peak over the 2 s post-load settle. Pages are distinct local fixtures (`127.0.0.1`, about 37 KB each), so the numbers are not shared-cache artefacts. | `manuk-proc`: N `manuk-mcp` processes, one page each. `manuk-fork`: one `manuk-mcp`, page 0 plus N−1 `fork`s of its checkpoint. `chrome-tabs`: one Chrome, N tabs. `chrome-procs`: N Chrome processes, one tab each. |
| fork | wall of one new page at a checkpoint | `manuk` `fork` (replay-based: one fixture load). Chrome: new browser context + target + navigate. |
| cold | wall from process spawn to first observation of a fixture page | `manuk-mcp` navigate. Chrome: spawn → DevTools port → target → load event. |
| urls | spawn, navigate to each of the 20 URLs in `docs/sprint/BASELINE-URLS.txt`, wall and peak PSS, 3 runs | `manuk-proc` (`manuk-mcp` navigate). `chrome-proc` (`r01-baseline.py` `run_chrome`). |

## Results (3 runs each; chrome_ver 145.0.7632.116)

### Pages: per-page PSS and derived pages per GB

| N | arm | total PSS (MB) | per page (MB) | pages/GB (derived) |
|---|---|---|---|---|
| 1 | manuk-proc | 30.7–30.8 | 30.7–30.8 | 33.2–33.3 |
| 1 | manuk-fork | 30.8–31.2 | 30.8–31.2 | 32.9–33.2 |
| 1 | chrome-tabs | 399–407 | 398.6–406.9 | 2.5–2.6 |
| 1 | chrome-procs | 398–404 | 398.4–403.7 | 2.5–2.6 |
| 10 | manuk-proc | 142 | 14.2 | 72.0–72.1 |
| 10 | manuk-fork | 70 | 7.0 | 145.4–146.0 |
| 10 | chrome-tabs | 517–520 | 51.7–52.0 | 19.7–19.8 |
| 10 | chrome-procs | 2042–2070 | 204.2–207.0 | 4.9–5.0 |
| 50 | manuk-proc | 619–620 | 12.4 | 82.6–82.7 |
| 50 | manuk-fork | 245 | 4.9 | 208.8–209.0 |
| 50 | chrome-tabs | 1004–1006 | 20.1 | 50.9–51.0 |
| 50 | chrome-procs | 9509–9530 | 190.2–190.6 | 5.4 |

Per-page ratio, manuk-proc ÷ chrome-tabs: **0.077× at N=1, 0.27× at N=10, 0.62× at N=50.** Against one Chrome process
per page (chrome-procs): 0.077× at N=1, 0.069× at N=10, 0.065× at N=50.

The manuk bands are under 0.3 MB wide at every N. Chrome's N=1 band is about 9 MB wide. Either way, the gap between
arms is far larger than the run-to-run spread.

The pages/GB column is **derived** (1 GiB ÷ per-page PSS). The harness does not fill a 1 GiB budget. It is a
projection from 1/10/50-page loads, and it assumes the per-page cost stays flat past 50.

### Fork and cold start

| axis | arm | wall (ms) |
|---|---|---|
| fork | manuk `fork` (replay of one fixture load), 27 forks at N=10 and 147 at N=50 | 36–43 |
| fork | Chrome new context + target + navigate, 3 runs | 65–70 |
| cold start (spawn → first observation of fixture) | manuk-proc, 3 runs | 55–57 |
| cold start (spawn → first observation of fixture) | chrome-proc, 3 runs | 305–314 |

manuk's `fork` is replay-based (the tool says so: "costs one page load"), so its number is a page load, not a memory
snapshot. The fork time did not change with N.

### The 20 baseline URLs: a small paired slice, and a noisy wall clock

Per-URL outcomes over 3 runs (30 rows per arm):

| arm | OK | CHALLENGE | FAIL | TIMEOUT |
|---|---|---|---|---|
| manuk-proc | 17 | 6 | 7 | 0 |
| chrome-proc | 24 | 0 | 3 | 3 |

- **CHALLENGE** is a heuristic: the observation text contains "challenge". It catches Cloudflare pages on basescan.org
  (`__cf_chl_rt_tk`) and a zoomph.com login page. The rule is broad, and the matched text is not stored, so the
  zoomph row is not confirmed as a bot challenge. The Chrome side has no content check, so its OK rows may include
  challenge pages too. This is a load-level comparison, not a content-level one.
- **FAIL / TIMEOUT** on both arms: `mkp.gem.gov.in` (manuk: connection refused; Chrome: FAIL) and
  `virtual.lapampa.edu.ar` (manuk: FAIL after 30 s; Chrome: TIMEOUT at 60 s). Also manuk-only: `ebanking.bdm-sa.com`
  (OK in 2 of 3 runs). These rows are kept in `runs.tsv`. For manuk FAIL rows the `note` column holds the page-id
  line, not the error text.

Paired comparison, URLs that loaded OK in all 3 runs on both arms: **5 URLs.**

| metric | band across the 3 runs | reading |
|---|---|---|
| manuk / chrome per-URL PSS | **0.30–0.31×** | stable across runs |
| manuk / chrome per-URL wall | 1.66–1.96× (this run) | **not settled** (see below) |

The wall ratio is not a band to quote. Per-URL walls on live sites swing widely between runs. For example, comix.to on
manuk ran 9.0, 9.1 and 19.4 s, and www.puentedemando.com ran 9.1 and 15.9 s. An earlier run of the same harness on a
stale `manuk-mcp` binary, which I discarded, gave 3.1–3.8× on a different 6-URL set. Both runs agree that manuk's
single-page wall clock is slower than Chrome's on these sites, which is the same direction as the 2.4–3.1× in
`docs/sprint/BASELINE.md`. Neither run supports a tight number, so none is claimed here.

Chrome's wall here comes from r01's driver, which includes browser startup and a DOM read. manuk's wall includes
spawn and the navigate call. The two are close to the same thing, but not identical.

## What this does and does not show

**Shown, with bands:**
- In the process-per-page model, manuk's fixed cost per page is about 31 MB against about 400 MB for one Chrome
  process. The gap narrows as N grows, because Chrome tabs inside one browser share most of their cost. At N=50,
  manuk-proc is at 0.62× of chrome-tabs per page, not 0.08×.
- Each added manuk fork costs about 4.4 MB (the increment over the first page, from N=10 and N=50), against about
  12 MB for each added manuk process.
- manuk's cold start to first observation of a fixture is about 0.18× Chrome's (57 ms against 305–314 ms).
- On the 5 paired real URLs, manuk's per-page PSS is 0.30–0.31× Chrome's across the 3 runs.

**Not shown, and not claimed:**
- **Forked pages are not content-verified.** manuk's single-page backend has no tab list, so the fork check only
  proves that N−1 forks each returned a distinct page id (`forks_valid`). It does not prove that each fork shows
  the checkpoint's DOM. The 4.9 MB/page figure depends on that being true. Verifying it is node W4.E0.b.
- **Pages/GB is a projection** from fixture pages of about 37 KB. Real pages are heavier, and the paired real-URL
  PSS ratio (0.30×) is the number to read for real pages.
- **No pages/GB claim against the 0.47× number in BRIEF.** That figure came from the 10 corpus URLs, not from this
  harness. This harness gives 0.30× on a 5-URL subset, which is a different sample.
- **Throughput under load** (N concurrent pages doing real work) is not measured. Only resident footprint is.
- **Lightpanda's self-reported 10× figures** are not tested here. They are the external reference only.

## Method notes that affect the numbers

- Peak PSS is summed over each arm's whole process tree. Shared pages are split across their users (PSS, not RSS).
  RSS is not recorded: the harness keeps PSS only, and the r01 driver's RSS is discarded here.
- `manuk-proc` pages are separate `manuk-mcp` processes, each under its own landlock/confinement (the default).
- The first full run was discarded. It used a `manuk-mcp` binary built at 15:01, before three `mcp/src` files changed
  (the W4.D2 taint commit). The fix was a rebuild, not a change to the numbers. The final run is the only one reported.
- `summary.tsv` is re-derived from `runs.tsv` with `python3 -I scripts/w4e0-footprint.py --out docs/sprint/footprint --summarize-only`.
- Selftest (`--selftest`) goes red when the fork-id check is removed or the pin is unset. Both mutants were run.

## Reproduce

```
cargo build --release -p manuk-mcp
MANUK_CHROME_PIN=145.0.7632.116 python3 -I scripts/w4e0-footprint.py --out docs/sprint/footprint
python3 -I scripts/w4e0-footprint.py --selftest
```
