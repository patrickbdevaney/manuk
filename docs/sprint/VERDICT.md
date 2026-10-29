# VERDICT — Bar 1 / Bar 2, read against SUCCESS.md as written

chrome_ver=145.0.7632.116 (all rows). Driving model Haiku. SUCCESS.md is unedited (sha256 32bef687…3c5). The injection rule is
reported under both columns per SUCCESS-AMEND.md (execution = headline, substring = pre-registered secondary).
SCOREBOARD rows have no id column; a cell's source is the (arm, suite_hash, task, run) key set named in the Source column,
reproduced by the command shown. suite hashes: own-only v1 `69903c7d…`, full own+external v1.1 `061ee22b…`.

## Table

| Item | Result | Numbers (3-run band) | Source |
|---|---|---|---|
| Bar 1.1 one-line install | NOT MEASURED on a clean checkout | 9 tools listed in a fresh session after `claude mcp add manuk -- manuk-mcp`; the release build was incremental, clean-checkout proof is open (R1.7.a.a TODO) | docs/sprint/R1.7.a-install.txt, JOURNAL R1.7.a |
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

Bar 1 holds on items 2 and 3, item 4 is DONE in the graph, and item 1 lacks the clean-checkout proof, so Bar 1 is MET provisionally pending R1.7.a.a.

Applicable SUCCESS.md row: **Bar 1 only** — "A useful agent tool, but the engine is not the reason."
Then: ship the MCP layer with a Chrome backend (`manuk-mcp --backend chrome`, R4.1); manuk becomes the deterministic/forkable research backend.

## Caveats (stated, not argued away)
- The rollback branch is unmeasured rather than refuted: the model never needed or chose to restore (open question for the owner, not a row).
- The injection slice has no discriminating case: Playwright also scores ~0% under the execution rule, so that branch cannot be won on this slice.
- Thresholds are unedited; no task was removed or tuned.
