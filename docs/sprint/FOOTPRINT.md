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

- **CHALLENGE** (rule replaced by W4.E0.b, applies to re-runs only; the numbers below were taken with the old broad "challenge" text rule): the page text/DOM contains a specific marker (`__cf_chl`, `Just a moment`), logged per row in summary.tsv, on both arms. It catches Cloudflare pages on basescan.org
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
- **Fork content check is separate from the memory runs.** W4.E0.a added a read-back to the harness (`extract` on each
  fork's page id, `# fixture 0` must be a whole line). A live check (`--live-fork-check N`) read 9 of 9 forks at N=10
  and 49 of 49 at N=50, so the 4.9 MB/page fork figure is for pages that hold the checkpoint's content. The recorded
  `runs.tsv` rows pre-date the read-back and were not re-taken.
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
- Selftest (`--selftest`) goes red when the fork-id check is removed or the pin is unset. Both mutants were run. The heading check goes red on an empty page and on a wrong expected heading (live: `fixture 7` expected, FAIL after 1 fork).

## Reproduce

```
cargo build --release -p manuk-mcp
MANUK_CHROME_PIN=145.0.7632.116 python3 -I scripts/w4e0-footprint.py --out docs/sprint/footprint
python3 -I scripts/w4e0-footprint.py --selftest
```

## Pages on real URLs (W4.E0.c): the fixture ratio does not carry over

_Measured 2026-10-31, `chrome_ver = 145.0.7632.116`, `manuk-mcp` built from the tree at this commit. Raw rows:
`docs/sprint/footprint/runs-real-pages.tsv`. Summary: `docs/sprint/footprint/summary-real-pages.tsv`. Run with
`MANUK_CHROME_PIN=145.0.7632.116 python3 -I scripts/w4e0-footprint.py --real-pages --out docs/sprint/footprint`._

Set: the 5 URLs that loaded OK in all 3 runs on both arms of the URL sweep (`paired_targets()` in the harness, derived
from `runs.tsv`, not picked by hand): comix.to, client.dkitrxmdwoqruvsi.net, www.mobile.bg, app.ordertime.com,
www.puentedemando.com. N = 10 pages, round-robin over them, so each URL is open twice in a run. Arms: `manuk-proc` (10
`manuk-mcp` processes, one navigate each) and `chrome-tabs` (one Chrome, 10 tabs). Every page must land on its own
registered domain, or the run is `WRONG_PAGE`. All 6 runs landed 10 of 10 pages.

| arm | total PSS (MB) | per page (MB) | runs |
|---|---|---|---|
| manuk-proc | 1901–1909 | 190.1–190.9 | 3 |
| chrome-tabs | 848–860 | 84.8–86.0 | 3 |

**manuk / chrome-tabs per-page PSS: 2.22–2.24×** (3 runs). On real pages manuk is heavier per page than Chrome's
tabs, not lighter. This reverses the fixture result at N=10 (0.27×). Two things differ from the fixtures: the real
pages are about 6× heavier per manuk process (190 MB against 31 MB for a 37 KB fixture), and Chrome's tabs get cheaper
on real pages as they share one browser.

The earlier 0.30–0.31× figure is a different comparison: manuk one page per process against Chrome one process per
page (`chrome-procs`-style), on the same 5 URLs. Do not put the two next to each other as if they were one number.

Caveats:
- **Duplicate URLs.** N = 10 over 5 URLs means every site is open twice. Chrome may share work between same-site tabs,
  which would make the chrome-tabs number smaller than a unique-URL run. W4.E0.c.a runs N = 5 with each URL once.
- **chrome-procs is not run on real URLs yet** (W4.E0.c.b). It is the arm that matches manuk's process-per-page model.
- **Wall clock is not in this claim.** manuk-proc took 68–71 s for the 10 navigates and chrome-tabs 13–14 s. The
  navigates run one after another and the live sites are slow and noisy (see above). Not a claim.
- **Set size.** `BASELINE-URLS.txt` has **10** URLs, not 20. The earlier "20 baseline URLs" wording in this file is
  wrong. The sweep had 30 rows per arm = 10 URLs × 3 runs. Only the 5 paired ones are used here.
- **Live pages.** The same sites can fail or change between runs. A run that loses a page is reported as FAIL, not
  averaged in.

Gate evidence: `--real-pages-mutant` navigates fixture pages while checking against the real hosts. Both arms come out
`WRONG_PAGE` with `landed_ok=0/10` (`docs/sprint/footprint/runs-real-pages-mutant.tsv`). The harness selftest covers
the same-site rule and the manuk landed-host parse.

## Wall clock on the URL set, 10 runs (W4.E0.d): not settled

Note: `BASELINE-URLS.txt` has 10 URLs, not 20. Run: `--url-runs 10`, rows in `footprint/runs-urls.tsv`, summary in
`footprint/summary-urls.tsv`, chrome_ver=145.0.7632.116. Rule fixed in `wall_verdict()` before the run: a wall ratio is
quotable only with 10 runs, a band that excludes 1.0, and no overlap with the earlier runs (1.66–1.96× and 3.1–3.8×).

| metric | value over 10 runs | verdict |
|---|---|---|
| paired URLs (OK in all 10 runs, both arms) | 7 | |
| manuk / chrome per-URL wall, median per run | **1.67–3.85×** | NOT_SETTLED: overlaps the prior 1.66–1.96× range |
| manuk / chrome per-URL PSS, median per run | 0.42–0.48× | band reported, not part of the wall claim |

Why the wall stays unsettled: per-URL walls on live sites are bimodal between runs. Example: comix.to manuk ran
10.3–13.6 s in runs 1–6 and 9.3–9.5 s in runs 7–10; app.ordertime.com ran 11.2 s once and 3.7–6.0 s in the others. Per-run
ratios therefore move with which mode each URL lands in.

Outcomes on the 10 URLs × 10 runs (100 rows per arm):

| arm | OK | CHALLENGE | FAIL | TIMEOUT |
|---|---|---|---|---|
| manuk-proc | 70 | 10 (basescan.org, `__cf_chl`, every run) | 20 | 0 |
| chrome-proc | 70 | 10 (basescan.org, `__cf_chl`, every run) | 10 | 10 |

`virtual.lapampa.edu.ar` times out at 30 s on manuk and at 60 s on Chrome; `mkp.gem.gov.in` fails on both arms.

**Claim:** none. The earlier per-URL wall ratios are not replaced by this table. The 10-run band is the number to quote, and
it is 1.67–3.85×, not a single ratio.
