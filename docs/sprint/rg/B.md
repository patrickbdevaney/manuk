# RG.B REAL re-bench at HEAD (c47a2bd0 + RG.B tooling), chrome_ver=145.0.7632.116

Harness: `bench/agent-tasks/real_run.py` (the W5.B1 harness), Haiku agent, judge `openai/gpt-oss-120b` for every
REAL row, 102 judged tasks. Driver: `docs/sprint/rg/B-run.sh`. Tally: `bench/agent-tasks/rgb_tally.py`.

## Result

| arm | run 1 | run 2 | run 3 | band | tasks pass >=2 of 3 |
|---|---|---|---|---|---|
| manuk (RG.B, HEAD) | 43 | 38 | 41 | 38-43 | 42 / 102 |
| playwright (W5.B1 3 runs + RG.B drift control) | 48, 44, 50 (B1) | 49 (drift control, RG.B) | | 44-50 | n/a (control is 1 run) |
| manuk (W5.B1, for reference) | 28 | 28 | 29 | 28-29 | |

**Verdict: GAP.** The manuk band (38-43) and the Playwright band (44-50) do not overlap. Manuk is still below
Playwright on REAL, though W5.B1 manuk was 28-29 per run and now is 38-43.

Drift control: the Playwright re-run (49) is inside its W5.B1 band (44-50), so the harness did not drift.

## Task lists (from `rgb_tally.py`)

GAP tasks (Playwright passes >=2 of 3 in W5.B1, manuk does not pass >=2 of 3 now), 11 tasks:
dashdish-11, fly-unified-1, fly-unified-6, gocalendar-1, gocalendar-2, gomail-3, gomail-5, opendining-10,
udriver-7, zilloft-2, zilloft-5.

AHEAD tasks (manuk passes >=2 of 3, Playwright W5.B1 does not), 6 tasks:
dashdish-3, gomail-8, networkin-3, opendining-2, opendining-8, topwork-7.

## Judge consistency (one judge per task)

The W5.B1.f fallback judge (Haiku via `claude -p`) judged 14 manuk rows (9 tasks) and 4 playwright rows during the
run, because the primary judge was rate-limited. The node requires one judge per task, so those rows were moved to
`docs/sprint/rg/B-judge-fallback-dropped-{manuk,playwright}.tsv` (kept, not deleted) and re-run with
`docs/sprint/rg/B-rejudge.sh`. Re-running used the same (arm, task, run) keys and the same harness. Three rows fell
back again and were re-run once more (manuk staynb-7 run 2; playwright gocalendar-9 run 1, twice). Final state:
all 306 manuk rows and all 102 playwright rows have `judge_model=openai/gpt-oss-120b`; `excluded_rows: 0`.

## Gate evidence

- The tally refuses any row whose judge is not gpt-oss-120b or whose chrome_ver differs from
  `$MANUK_CHROME_PIN` (checked before the first verdict; it raised on the fallback rows before they were moved).
- `python3 -I bench/agent-tasks/rgb_tally.py --self-test` GREEN: a mutant forcing one failing task to pass moves the
  count 42 -> 43 (exit 0). A tally that ignored the mutant would exit 1.
- Playwright band uses the B1 rows (`docs/sprint/w5/B1-real-playwright.tsv`) plus the RG.B drift run.

## Next round

Per the node rule (not WIN): RG.2.0 (ledger on the 11 GAP tasks) and RG.2.B (bench) are appended to GRAPH.tsv.
