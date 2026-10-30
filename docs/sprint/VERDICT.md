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
