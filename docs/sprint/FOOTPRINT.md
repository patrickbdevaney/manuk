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

- **CHALLENGE** (W4.E0.b rule: the page text/DOM contains `__cf_chl` or `Just a moment`, logged per row in summary.tsv; the old broad "challenge" text rule is retired). Re-checked in W4.E0.b.a with the specific markers on both arms: basescan.org is a Cloudflare challenge on every run on both arms (`__cf_chl`). **zoomph.com is settled as not a challenge by this rule:** `admin.zoomph.com` redirects to `authentication.zoomph.com/login`, and no marker matched on either arm in any of 10 runs (both arms OK). That is a login page with no Cloudflare marker, not a bot-challenge interstitial. The marker rule says nothing about whether the login page is bot-gated otherwise. The Chrome side now has a content check too, so its OK rows are checked for the same markers.
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

## Pages on real URLs, unique-URL control (W4.E0.c.a): the N=10 ratio was inflated by repeats

_Measured 2026-11-17, `chrome_ver = 145.0.7632.116`, `manuk-mcp` built from the tree at this commit. Raw rows:
`docs/sprint/footprint/runs-real-pages-n5.tsv`. Summary: `docs/sprint/footprint/summary-real-pages-n5.tsv`. Run with
`MANUK_CHROME_PIN=145.0.7632.116 python3 -I scripts/w4e0-footprint.py --real-pages-n5 --out docs/sprint/footprint`._

Same 5 paired URLs as the section above, but N = 5: each URL is open exactly once per run (`unique_targets()` refuses
any list shorter than 5 or with a repeated registered domain). 3 runs, both arms.

| arm | total PSS (MB) | per page (MB) | runs | landed_ok |
|---|---|---|---|---|
| manuk-proc | 959–974 | 191.9–194.7 | 3 | 5/5 each run |
| chrome-tabs | 739–780 | 147.7–155.9 | 3 | 5/5 each run |

**manuk / chrome-tabs per-page PSS: 1.25–1.30×** (3 runs), against 2.22–2.24× at N = 10 on the same URLs.

Read: the repeats mattered. At N = 10 each site was open twice, and Chrome's tabs got cheaper per page (about 85 MB at
N = 10 against about 150 MB at N = 5), so Chrome shares work between same-site tabs. manuk's per-page cost did not move
(about 190 MB in both sets), since each manuk page is its own process. The N = 10 ratio of 2.22–2.24× is therefore an
overstatement for unique sites. Neither number is a general claim: 5 URLs on live sites, one Chrome, one chrome_ver.
The unique-URL 1.25–1.30× is the number to quote for "manuk-proc against one Chrome with one tab per site".
Wall clock is not claimed (manuk-proc 47–60 s, chrome-tabs 12–13 s; live sites, navigates in sequence).

Gate evidence: `--real-pages-n5-mutant` (fixture pages checked against the real hosts) comes out `WRONG_PAGE`,
`landed_ok=0/5` on both arms (`docs/sprint/footprint/runs-real-pages-n5-mutant.tsv`). `--selftest` refuses a 4-URL list
and a list with `m.comix.to` added beside `comix.to`. The per-page comparison against chrome-procs is still W4.E0.c.b.

## Wall clock on the URL set, 10 runs (W4.E0.d): not settled

Note: `BASELINE-URLS.txt` has 10 URLs, not 20. Run: `--url-runs 10`, rows in `footprint/runs-urls.tsv`, summary in
`footprint/summary-urls.tsv`, chrome_ver=145.0.7632.116. Rule fixed in `wall_verdict()` before the run: a wall ratio is
quotable only with 10 runs, a band that excludes 1.0, and no overlap with the earlier runs (1.66–1.96× and 3.1–3.8×).

| metric | value over 10 runs | verdict |
|---|---|---|
| paired URLs (OK in all 10 runs, both arms) | 7 | |
| manuk / chrome per-URL wall, median per run | **1.66–5.81×** (2026-10-31 sweep: 1.67–3.85×) | NOT_SETTLED: overlaps the prior 1.66–1.96× range |
| manuk / chrome per-URL PSS, median per run | 0.21–0.25× (2026-10-31 sweep: 0.42–0.48×) | band reported, not part of the wall claim. **Moved between sweeps, not a manuk change:** Chrome's per-URL PSS rose about 50% on every URL (e.g. comix.to 345→496 MB, puentedemando.com 375→524 MB) while manuk's stayed within about 10% (manuk-proc means in `runs-urls.tsv`). The denominator is Chrome-side box state; do not quote either band as a stable ratio. |

Why the wall stays unsettled: per-URL walls on live sites are bimodal between runs. Example: comix.to manuk ran
10.3–13.6 s in runs 1–6 and 9.3–9.5 s in runs 7–10; app.ordertime.com ran 11.2 s once and 3.7–6.0 s in the others. Per-run
ratios therefore move with which mode each URL lands in.

Outcomes on the 10 URLs × 10 runs (100 rows per arm):

| arm | OK | CHALLENGE | FAIL | TIMEOUT |
|---|---|---|---|---|
| manuk-proc | 70 | 10 (basescan.org, `__cf_chl`, every run) | 20 | 0 |
| chrome-proc | 70 | 10 (basescan.org, `__cf_chl`, every run) | 10 | 10 |

Re-run in W4.E0.b.a (2026-11-17, same pin, same 10 URLs × 10 runs, `runs-urls.tsv` overwritten; the counts above are unchanged). Outcome counts are identical to the 2026-10-31 sweep; the CHALLENGE rows now carry the matched marker.

`virtual.lapampa.edu.ar` times out at 30 s on manuk and at 60 s on Chrome; `mkp.gem.gov.in` fails on both arms.

**Claim:** none. The earlier per-URL wall ratios are not replaced by this table. The 10-run band is the number to quote, and
it is now 1.66–5.81× (W4.E0.b.a re-run), not a single ratio. The wall band widened and still spans the earlier ranges, so the verdict stays NOT_SETTLED.

## Pages on real URLs, with chrome-procs (W4.E0.c.b): one Chrome process per page is 0.57–0.65× manuk-proc's PSS

_Measured 2026-11-17, `chrome_ver = 145.0.7632.116`, `manuk-mcp` from `target/release` at this commit. All three arms in
one session, so the three numbers are comparable with each other: `manuk-proc`, `chrome-tabs` and `chrome-procs`, N = 10,
round-robin over the 5 paired URLs (each URL twice, as in W4.E0.c), 3 runs. Raw rows:
`docs/sprint/footprint/runs-real-pages-procs.tsv`. Summary: `docs/sprint/footprint/summary-real-pages-procs.tsv`. Run with
`MANUK_CHROME_PIN=145.0.7632.116 python3 -I scripts/w4e0-footprint.py --real-pages-procs --out docs/sprint/footprint`._

| arm | total PSS (MB) | per page (MB) | runs | landed_ok |
|---|---|---|---|---|
| manuk-proc (10 processes) | 1589–1896 | 158.9–189.6 | 3 | 10/10 each run |
| chrome-tabs (1 Chrome, 10 tabs) | 990–1004 | 99.0–100.4 | 3 | 10/10 each run |
| chrome-procs (10 Chromes, 1 tab each) | 2793–2910 | 279.3–291.0 | 3 | 10/10 each run |

Per run, the ratio of PSS on the same session: manuk-proc / chrome-procs **0.59, 0.57, 0.65** (band **0.57–0.65×**);
manuk-proc / chrome-tabs **1.66, 1.60, 1.89** (band **1.60–1.89×**).

What this says: on the same live pages, a manuk process per page holds about 0.6× the summed PSS of a Chrome process per
page. Against one shared Chrome with the same pages in tabs (chrome-tabs), the manuk arm holds 1.6–1.9× the PSS. Both
statements come from this table and only from this table.

**Not claimed, and not reconciled here:**
- **The earlier 0.30–0.31× figure** (the URL sweep: manuk one page vs Chrome one process) is a different comparison,
  made on a different page set and in a different session. It does not match 0.57–0.65× and this section does not explain
  the gap. Quote the 0.57–0.65× band for "real pages, same session, process per page", not the older figure.
- **Session drift.** The manuk-proc / chrome-tabs ratio moved from 2.22–2.24× (N=10, repeated URLs, 2026-10-31) to
  1.25–1.30× (N=5, unique URLs, 2026-11-17, W4.E0.c.a) to 1.60–1.89× here (N=10, repeated URLs, same day as c.a). Repeats and
  live-site state both change it; a second session of this set is owed (see the node's follow-up row).
- **Wall clock is not claimed.** manuk-proc took 110–138 s for the 10 navigates (in sequence) and chrome-procs 53–135 s;
  chrome-tabs 14–16 s. The live sites are slow and noisy. Not a claim.

**Harness note (disclosed).** The first attempt of this session crashed on run 1 at manuk-proc: `manuk-mcp` was silent for
the 60 s page timeout on a live page, and the exception was not caught, so no rows were written. The harness now records a
silent navigate as a `FAIL` run (`run_manuk_proc`), which is the rule the node sets: a run that is not `OK` on every page is
reported as FAIL, not averaged. The rerun is the one reported above; it had **zero** FAIL runs. The rerun is the only full
set, so it is not a best-of-two.

**Gate evidence.**
- `--real-pages-procs-mutant` navigates fixture pages while checking against the real hosts. All three arms come out
  `WRONG_PAGE` with `landed_ok=0/10` (`docs/sprint/footprint/runs-real-pages-procs-mutant.tsv`), and no summary is written.
- `--real-pages-procs` exits 1 and writes no summary unless every arm has exactly 3 OK runs (`arms_short_of_runs()`).
- `--selftest` checks that the chrome-procs ratio is derived from OK runs only (a WRONG_PAGE run is excluded from the band),
  that the chrome-tabs ratio line is unchanged, and that an arm with a WRONG_PAGE run is reported short.

## Pages on real URLs, with chrome-procs, second session (W4.E0.c.b.a): the ratio moves between sessions, 0.57–0.70×

_Measured 2026-11-17, `chrome_ver = 145.0.7632.116`, `manuk-mcp` from `target/release` at this commit. Same harness, same five
paired URLs, N = 10 round-robin, 3 runs, all three arms in one session, as in W4.E0.c.b. The URL list is read from a copy of
`docs/sprint/footprint/runs.tsv` placed in the session dir, so both sessions use the same pairing. Raw rows:
`docs/sprint/footprint/session-2/runs-real-pages-procs.tsv`. Summary: `docs/sprint/footprint/session-2/summary-real-pages-procs.tsv`.
Run with `MANUK_CHROME_PIN=145.0.7632.116 python3 -I scripts/w4e0-footprint.py --real-pages-procs --out docs/sprint/footprint/session-2`._

Both sessions side by side (per-page PSS in MB; ratios are per run, min–max over the 3 runs):
`docs/sprint/footprint/session-compare-procs.tsv`.

| session | manuk-proc (MB/page) | chrome-tabs (MB/page) | chrome-procs (MB/page) | manuk-proc / chrome-procs | manuk-proc / chrome-tabs |
|---|---|---|---|---|---|
| s1 (W4.E0.c.b) | 158.9–189.6 | 99.0–100.4 | 279.3–291.0 | 0.57–0.65× | 1.60–1.89× |
| s2 (this node) | 178.2–191.4 | 99.6–103.1 | 272.2–281.6 | 0.64–0.70× | 1.73–1.90× |
| pooled, 6 runs | | | | **0.57–0.70×** | **1.60–1.90×** |

**What moved, and what did not.** chrome-procs per page is stable across sessions (272–291 MB, within about 7%). chrome-tabs is
stable (99–103 MB). The drift is in the manuk-proc arm: its low end moved from 158.9 to 178.2 MB/page, and its run-to-run spread
was 19% in session 1 and 7% in session 2. So the ratio's session-to-session shift comes mostly from the manuk-proc side. Two
sessions with three runs each cannot say why. The live pages, the browser, and the harness sampling all vary between sessions,
and this table does not separate them.

**What this says, and the band to quote.** On these five live pages, with one manuk-mcp process per page versus one Chrome process
per page, manuk-proc holds **0.57–0.70×** the summed PSS of chrome-procs across both sessions (6 runs). Against one shared Chrome
with the pages in tabs, manuk-proc holds **1.60–1.90×**. Quote the pooled band, with the two sessions named. Do not quote either
session's band alone as the answer. The 0.30–0.31× URL-sweep figure is reconciled in the section below (W4.E0.c.b.b).

**Not claimed.** Wall clock (not claimed; in the reported set, manuk-proc took 71–112 s for the 10 navigates, chrome-procs 44–62 s, chrome-tabs
13–19 s, on live sites). Any claim about the Oct-31 sweep (N = 10 repeated URLs, 2.22–2.24×) is not part of this table.

**Attempt record (disclosed; both sets kept).**
- Attempt 1 of this session: `docs/sprint/footprint/session-2/attempt1-runs-real-pages-procs-FAIL.tsv`. manuk-proc and chrome-tabs
  OK in all 3 runs; chrome-procs OK in runs 1 and 2, and **FAIL in run 3** (a Chrome process raised one of the exceptions the harness
  maps to FAIL; the cause was not logged). The harness correctly exited 1 with `NOT_SETTLED` and wrote no summary. This is the red
  case for the three-OK-runs rule on real data.
- Attempt 2 (the reported set above): rerun in full with the same settings. Zero FAIL rows. This is a rerun after a failed
  attempt, not a best-of-two: attempt 1's raw rows are kept and the run is not selected by its numbers. The W4.E0.c.b precedent
  is the same kind of rerun.
- Known harness defect, not fixed here: a FAIL row's `note` reads `landed_ok=10/10` even though its `landed` list is empty, because
  the note is computed from the list without checking status. Non-OK rows are never averaged, so the summary is not affected, but
  the raw row is misleading. Follow-up node W4.E0.c.b.a.a.

**Gate evidence.**
- `--real-pages-procs-mutant` (session-2 dir): all three arms `WRONG_PAGE`, `landed_ok=0/10`
  (`docs/sprint/footprint/session-2/runs-real-pages-procs-mutant.tsv`); no summary written.
- `--real-pages-procs` exited 0 and wrote the summary because each arm had exactly 3 OK runs. On attempt 1, the same command
  exited 1 with `NOT_SETTLED` (chrome-procs at 2 OK runs), so the rule is exercised on live data in both directions.
- `--selftest` OK at this commit.

## Reconciling 0.30–0.31× with 0.57–0.65× (W4.E0.c.b.b)

_Measured 2026-11-18, `chrome_ver = 145.0.7632.116`, one session. Raw rows: `docs/sprint/footprint/session-3/runs-urls.tsv`
(sweep) and `session-3/runs-real-pages-n5-procs.tsv` (N=5 concurrent). Arithmetic: `python3 -I docs/sprint/w4ebbb/reconcile.py`
(outputs in `docs/sprint/w4ebbb/`). New harness flag: `--real-pages-n5-procs`._

The two figures are different statistics on different arms, over the same pages:

| figure | arms | pages per measurement | statistic | value |
|---|---|---|---|---|
| 0.30–0.31× (URL sweep, `runs.tsv`) | manuk-mcp process vs one fresh Chrome process (r01 driver), each loading **one** URL alone | 1 | **median** of the per-URL manuk/chrome PSS ratios, 5 paired URLs | 0.300–0.310 |
| 0.57–0.65× (W4.E0.c.b) | 10 manuk-mcp processes vs 10 Chrome processes alive at once | 10 (5 URLs, each twice) | summed PSS manuk / summed PSS chrome | 0.57–0.65 |

Re-measured in this session on the same 5 paired URLs (sweep, 3 runs, same harness):

| statistic on the single-URL sweep | this session (3 runs) | original sweep (3 runs) |
|---|---|---|
| median of per-URL ratios | **0.322–0.333×** | 0.300–0.310× |
| pooled (sum manuk / sum chrome) | **0.464–0.469×** | 0.423–0.426× |
| mean per page, manuk / chrome | 213–215 / 458–460 MB | 211–212 / 498–500 MB |

So the 0.30× case reproduces approximately (0.32–0.33× now; the Chrome single-process cost per page fell from about 499 to
about 459 MB between sessions, which this note does not explain). Two things then separate it from 0.57–0.65×:

1. **The statistic.** The median is set by the light pages (client.dkitrxmdwoqruvsi.net is 25–31 MB on manuk against about
   360 MB on Chrome, 0.07×); the heavy pages (comix.to, puentedemando.com at 350–450 MB on manuk) sit at 0.7–0.9×. Pooling
   sum over sum on the identical rows gives 0.46–0.47× here (0.42–0.43× on the original rows).
2. **How many pages are alive at once.** On the same 5 URLs, one page each, all alive together (N=5, three arms in one session):
   manuk-proc 192–195 MB/page, chrome-procs 319–320 MB/page, **ratio 0.60–0.61×** (chrome-tabs 151–161 MB/page, manuk-proc /
   chrome-tabs 1.21–1.28×). Chrome's per-page PSS falls from about 459 MB alone to about 319 MB with five running together
   (shared library and zygote pages are split across more processes); manuk's falls only from about 214 to about 194 MB.
   Whether that drop is shared-library splitting is a reading, not something this node isolated.

Reading: the ratio is not a property of the engine alone. Median of single-page runs: 0.32–0.33×. Same rows pooled: 0.46–0.47×.
Five concurrent processes: 0.60–0.61×. Ten concurrent: 0.57–0.70× (6 runs over two sessions). Quote the concurrency and the
statistic with the number. The 7-URL variant (adds admin.zoomph.com and ebanking.bdm-sa.com, both light on manuk) gives median
0.21–0.22× and pooled 0.39× in this session, so the paired-set choice moves the figure too.

Gate: `reconcile.py --selftest` goes red on swapped arms (ratio inverts above 1.5) and on a truncated run set; a wrong URL set
exits 1 (`RED: paired_urls=0`). The N=5 run landed 5 of 5 pages on every arm in all 3 runs; the page-landing mutants of the
earlier nodes cover the same code path (`--real-pages-n5-mutant`, `--real-pages-procs-mutant`). Not tested: the 10-URL sweep's
remaining sites (CHALLENGE and FAIL rows are excluded from every figure above).
