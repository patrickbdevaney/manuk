# VERDICT — Bar 1 / Bar 2, read against SUCCESS.md as written

chrome_ver=145.0.7632.116 (all rows). Driving model Haiku. SUCCESS.md is unedited (sha256 32bef687…3c5). The injection rule is
reported under both columns per SUCCESS-AMEND.md (execution = headline, substring = pre-registered secondary).
SCOREBOARD rows have no id column; a cell's source is the (arm, suite_hash, task, run) key set named in the Source column,
reproduced by the command shown. suite hashes: own-only v1 `69903c7d…`, full own+external v1.1 `061ee22b…`.

## Table

| Item | Result | Numbers (3-run band) | Source |
|---|---|---|---|
| Bar 1.1 one-line install | MET, with one stated condition: the clean-clone release build needs the documented `./scripts/mozjs-fetch.sh` first (a bare `cargo build --release -p manuk-mcp` panics in mozjs_sys, rc≠0). After the prefetch: clone of 5171123e builds rc=0 in 2m52s; `claude mcp add manuk -- manuk-mcp` → ✔ Connected; fresh headless session lists 13 `mcp__manuk__` tools. Red: manuk-mcp off PATH → ✘ Failed to connect, zero tools | docs/sprint/R1.7.a.a-install.txt, JOURNAL R1.7.a.a; README.md:109 (prefetch step); chrome_ver=145.0.7632.116 |
| Bar 1.2 ≥25/40 tasks | MET | manuk own slice 39/40 tasks pass ≥2 of 3 runs; per-run 39, 39, 40 | `arm3_report.py`, arm=manuk suite 061ee22b |
| Bar 1.3 external slice published | MET | external 7/10 tasks pass ≥2/3; per-run 7, 5, 7 (band 5–7); losses listed in FAILURES-EXTERNAL.tsv | `arm3_report.py`; EXT.RUN.b |
| Bar 1.4 public demo | per graph DEMO.1 DONE; not re-read here | — | GRAPH.tsv DEMO.1 |
| Bar 2.1 within 10 points of Playwright+Chrome, full 50 tasks | MET | manuk 92/88/94% band [88,94]; Playwright 90/94/92% band [90,94]; worst case (manuk lowest − Playwright highest) = −6.0 points, inside 10 | `arm3_report.py`, arms manuk, playwright |
| Bar 2.2 hidden instructions | NOT MET | manuk followed 0/8, 0/8, 0/8 (0%, ≤5% ok). Playwright: execution rule 0/8 ×3 (0%), so it is not ≥20 points worse; substring rule 1/8 ×3 in the headline table (answer-only 0, 2, 1 of 8), at most 25%, band includes 0. arm 3 (Chrome layer): 1/8, 0/8, 0/8. Zero-vs-zero on this slice. | `grader.py --report`, slice injection, hidden column |
| Bar 2.2 tokens | NOT MET (opposite direction) | mean in+out tokens per row, full suite weighted 40 own + 10 external: manuk ≈979, Playwright ≈699, arm 3 ≈744, i.e. manuk uses ≈40% MORE than Playwright and ≈32% more than arm 3. Own slice 778 vs 608 vs 613. Best attempts (TOKENS.1, TOKENS.1.a) did not cut tokens: 34.2k and 33.5k/32.6k vs 31.1k per own run, with pass equal or lower. | `arm3_report.py`; docs/sprint/TOKENS1.md, tokens1a, tokens1aa |
| Bar 2.2 rollback | NOT MET | N=4 otherwise-failed tasks (F08, X03, X07, X08); M=0 flipped with a restore call; M/N = 0% (needs ≥15%). v2 arm: model loaded the tools in 143 of 150 runs and checkpointed in 11, restored in 0. Pass per run 45/47/46 vs baseline 46/44/47. No task in the list needs an undo. | `rollback_report.py --arm manuk+rollback.v2 --transcripts docs/sprint/rollback-v2/transcripts` |
| Item 2 vs K1 arm 3 (cdp-layer) | NOT MET on every branch | arm 3 matches or beats manuk: full suite 94/94/92 vs 92/88/94; injection 0% vs 0–12%; tokens lower | `arm3_report.py` |

## Verdict

Bar 2 condition 1 holds. Bar 2 condition 2 (a decisive win that arm 3 does not match) holds on **no** branch: hidden-instruction is 0 vs 0,
tokens go the wrong way, rollback was never exercised to a flip. Bar 2 is therefore **NOT MET**.

Bar 1 holds on items 1, 2 and 3; item 4 is DONE in the graph (DEMO.1). Item 1 is MET on a clean clone (R1.7.a.a), with the reading stated in the table: SUCCESS.md says the install works "on a clean checkout's release build", and the clone's release build needs the documented mozjs prefetch before it links. If the owner reads "clean checkout" as "bare `cargo build`, no prefetch", item 1 is NOT MET and the verdict row is unchanged, since the Bar 2 result does not depend on it. So Bar 1 is MET.

Applicable SUCCESS.md row: **Bar 1 only** — "A useful agent tool, but the engine is not the reason."
Then: ship the MCP layer with a Chrome backend (`manuk-mcp --backend chrome`, R4.1); manuk becomes the deterministic/forkable research backend.

## Caveats (stated, not argued away)
- The rollback branch is unmeasured rather than refuted: the model never needed or chose to restore (open question for the owner, not a row).
- The injection slice has no discriminating case: Playwright also scores ~0% under the execution rule, so that branch cannot be won on this slice.
- Thresholds are unedited; no task was removed or tuned.

## Addendum W3.18 (REAL slice, new suite version; the v1.1 verdict above is not edited)
Source: bench/agent-tasks/real_boot.py, rows in docs/sprint/w318/boot-manuk.tsv and boot-chrome.tsv; chrome_ver=145.0.7632.116; REAL = agisdk 0.3.5 v1 task set (112 tasks, 11 clones).
- Gate 1 (manuk boots >=10/11 clones): NOT MET. manuk boots 4/11 (dashdish, gomail, udriver, zilloft); 3 render a client-side "Application error" (opendining, staynb, topwork), 3 stay blank after 8 s (fly-unified, gocalendar, networkin), 1 is a Vercel DMCA takedown page (omnizon, dead for every backend). `--backend chrome` boots 10/11 under the same classifier.
- Gate 2 (restore-backed passes rise >=15% of failures) was not run: with 7 of 11 clones unusable on manuk, a 112-task x 3-run comparison would measure boot failures, not rollback. The 64 llm_boolean evals also need REAL's OpenAI-backed grader, and no OpenAI credential is configured here.
- Consequence for Bar 2: the engine has no REAL rollback evidence. Rollback stays unmeasured, not refuted. Follow-ups: W3.18.a (engine boot of the 6 failing clones), W3.18.b (grader key + omnizon).

## Addendum W3.18.a.a (REAL slice, deterministic subset; the v1.1 verdict and the W3.18 addendum above are not edited)
Source: bench/agent-tasks/real_run.py (one shim for every arm: REAL's /config + /finish setup, REAL's jmespath exact-match grading), report bench/agent-tasks/real_report.py → docs/sprint/w318/real-report.txt, rows docs/sprint/w318/real-runs.tsv (570 rows, all chrome_ver=145.0.7632.116); gate test scripts/real-gate2-test.sh. Haiku, 3 runs, arms run one at a time. Both Chrome arms ran the pinned CfT 145 with the sandbox ON (`--no-sandbox` removed by a wrapper; renderer Seccomp 2).
- **Deterministic subset:** 47 of REAL's 112 tasks. These are the tasks whose evals are all jmespath. 60 tasks hold an llm_boolean eval and are NOT SCORED on any arm, since REAL's grader needs an OpenAI key and none was provided. omnizon's 10 tasks are excluded on every arm (HTTP 451 DMCA). This subset is not comparable to REAL's published scores.
- **Pass counts (task passes iff ≥2/3 runs):** manuk 3/47 (per-run 3,3,3) · manuk+rollback.v2 3/47 (2,3,3) · cdp-layer 10/47 (5,10,10) · Playwright+Chrome 24/47 (25,23,24).
- **Gate 2 (rollback, Bar 2.2 rule): RED.** Of N=44 tasks manuk fails, 0 flip to pass on manuk+rollback.v2 (M=0, 0%; needs ≥15%), and the bands overlap (2-3 vs 3-3). The rollback arm loaded its tools and called checkpoint in 34/141 runs. It called restore in 1/141 (fly-unified-2 r2, which still failed 0/17 evals) and fork in 0.
- **What blocks the engine here is acting, not undoing.** manuk passes only gomail tasks. On opendining, Playwright passes 5/5 and manuk passes 0/5: agents report "the form rejects every phone number" and an unusable guest picker on staynb. The clone boots (state is read on 141/141 manuk runs), so this is a working shell, not a half-booted one, and its forms fail under manuk. Rollback cannot recover from a page that will not accept input.
- **Consequence for Bar 2:** REAL gives the engine no decisive rollback win. On this subset manuk is 21 tasks behind Playwright+Chrome (Bar 2.1 not met here), and cdp-layer, the manuk tool surface over Chrome, also trails Playwright (10 vs 24). The owner's post-verdict direction (the Chrome backend is the product) stands, and this is the last-shot result it asked for.
- **Harness notes, applied to every arm alike:** (1) opendining/staynb nest config.task_id one level down. The first 6 manuk r1 cells were rejected as invalid state before the shim learned this and were re-run; the report reads the latest row per cell. (2) Playwright staynb-9 r3 hit the 420 s timeout and counts as a fail. (3) The final Playwright cell overlapped the first re-run cell by about 1 min, so two browsers ran at once briefly.

## Addendum W4.A6 (re-bench after the layer fixes W4.A1/A4/A5; the verdict and addenda above are not edited)
Source: REAL det subset rows in docs/sprint/w4/A6-real.tsv (141 new rows, chrome_ver=145.0.7632.116, Haiku); own-suite rows appended to docs/sprint/SCOREBOARD.tsv (suite 69903c, 2026-10-31). Baseline = `real_report.py` on docs/sprint/w318/real-runs.tsv (run from the report, not from earlier prose).
- **Baseline correction:** the W3.18.a.a addendum above says manuk 3/47 (per-run 3,3,3). `real_report.py` on the same TSV gives manuk 6/47 (per-run 6,5,7, band 5-7). The report is the source; the old prose is not edited.
- **REAL, deterministic subset (47 tasks, pass = ≥2 of 3 runs):**

| Arm | Runs | Per-run | pass ≥2/3 | Band | Mean in+out tokens/task | Mean turns/task |
|---|---|---|---|---|---|---|
| manuk (engine) | 3 | 11, 11, 10 | 11/47 | 10-11 | ~6.2k | ~32 |
| cdp-layer | 3 | 15, 18, 16 | 15/47 | 15-18 | ~8.0k | ~43 |
| playwright | 1 (run 1 in band, no repeats) | 25 | n/a (single run) | 23-25 | ~5.1k | ~36 |

- **cdp-layer gate (≥4 tasks outside the band over the baseline):** MET. 15/47 against a baseline of 10/47 is +5, and all three runs (15-18) lie above the baseline's per-run max of 10. The +4 rule holds on the run-1 figure too (15 vs 5).
- **Own-suite gate (manuk within 39-40):** NOT MET. manuk own suite ×3 = 38, 39, 38 (prior 2026-10-29 run was 118/120 total). Task fails this run: F08 fails in all 3 runs ("answer missing 'Item 45: amber'"), L12 in 2 of 3 ("answer missing 'Marilyn Monroe'"). cdp-layer own suite = 40, 40, 40.
- **Engine vs layer:** manuk (10-11) stays behind cdp-layer (15-18) outside both bands on REAL. Any gain attributed to W4.A1/A4/A5 is a LAYER gain that cdp-layer also has; no engine win is claimed. Playwright (23-25) is still ahead of both.
- **Node status:** BLOCKED on the own-suite clause (manuk 38/39/38). The cdp-layer clause was met. Follow-up W4.A6.a investigates F08 and L12 on the manuk backend.
- **Harness notes:** playwright ran once only (its run 1 was inside its band, so the node's "3 if it moved" did not apply). Runs 2-3 for manuk and cdp-layer were run serially after run 1, interleaved per run. All rows: chrome_ver=145.0.7632.116.

## Addendum W4.B4 (token re-measure after W4.B1-B3; the verdict and addenda above are not edited)
Source: docs/sprint/w4/B4/{manuk,cdp_layer,playwright,playwright_preload}.tsv (+ `.turns.tsv` per-turn logs); report `bench/agent-tasks/b4_token_report.py` → docs/sprint/w4/B4/REPORT.md. Full suite = own 40 + external 14 (54 tasks), Haiku, 3 runs, 4 arms, chrome_ver=145.0.7632.116. Run 2026-10-31 by the W4.B4 agent. Transcripts are kept outside the repo (~/.cache/manuk-b4/transcripts).
Metrics are named, because the rankings differ by metric: **in+out** = input+output tokens (the metric used in the verdict above); **total** = in+out + cache_read + cache_create; **USD** = `cost_usd`.

| arm | pass per run (of 54) | tasks ≥2/3 | turns/task | in+out tok/task (3-run band) | total tok/task (band) | USD/task (band) |
|---|---|---|---|---|---|---|
| manuk (engine) | 48, 48, 48 | 48/54 | 5.38 | 727 [718, 733] | 126,540 [122,667, 132,230] | 0.0026 [0.0025, 0.0029] |
| cdp-layer | 50, 49, 51 | 51/54 | 4.58 | 604 [574, 637] | 108,586 [105,057, 110,983] | 0.0027 [0.0022, 0.0036] |
| playwright (no preload) | 49, 49, 48 | 50/54 | 6.27 | 696 [669, 721] | 122,686 [121,452, 124,331] | 0.0031 [0.0026, 0.0040] |
| playwright+preload | 51, 50, 48 | 51/54 | 5.35 | 517 [498, 542] | 169,018 [165,806, 175,135] | 0.0031 [0.0026, 0.0039] |

**Bar 2.2 tokens branch, read as SUCCESS.md writes it (manuk vs the best Playwright arm per metric, need ≥30% fewer at equal or better success, bands disjoint):**
- in+out: manuk 727 vs playwright+preload 517 → manuk uses **40.6% MORE**. NOT MET.
- total: manuk 126,540 vs playwright 122,686 → manuk uses 3.1% more. NOT MET.
- USD: manuk 0.0026 vs playwright 0.0031 → manuk is 14% cheaper; the bands overlap, and 14% is short of 30%. NOT MET.
- Success: manuk 48/54 per run vs Playwright band 48–51 → within 10 points (not the limiting condition).

**Verdict for the token branch: NOT MET on every metric.** Nothing is stated as a win: the only lower-than-Playwright number (USD, −14%) is inside the band overlap and below the 30% line. Turns are not the gap: manuk's 5.38 turns/task equals playwright+preload's 5.35, so the in+out deficit sits in tokens per turn (and in observation size, which shows up in total/USD), not in the turn count. cdp-layer is the cheapest manuk-surface arm (4.58 turns, 604 in+out), so the tool-layer gap to cdp-layer is larger than the engine-vs-Chrome gap.

**Harness checks (done on this run):** every row is at chrome_ver 145.0.7632.116 (the report refuses any other pin); 0 error rows in any arm (is_error column). Pass counts were not cross-checked against the transcripts. **Red check:** a halved-token mutant of the manuk arm flips the total metric to MET with disjoint bands, which shows the report can go green; a row at a wrong pin is refused (exit 1). Both mutations ran on copies in ~/.cache, not on the data.

**Caveat, not resolved here:** the per-turn `.turns.tsv` output_tokens do not sum to the run-level `output_tokens` (the per-turn average sums to far less than ~717 output tokens/task). Per-turn attribution is therefore not used for any claim in this addendum; follow-up W4.B4.a.
