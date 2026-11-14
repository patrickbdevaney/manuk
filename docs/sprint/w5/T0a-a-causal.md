# W5.T0.a.a: does a smaller navigate observation cut manuk's thinking? (chrome_ver=145.0.7632.116)

Design: the manuk arm on the 12 largest-thinking-gap tasks from W5.T0.a (L12 X05 L03 X06 X02 L07 X01 L09 L17 X03 L13 X08),
3 runs each, model Haiku, `--suite full`. Two arms interleaved within each run number:
- nocap: navigate observation unchanged.
- cap: `MANUK_NAV_CAP_BYTES` = round(obs_prev_pw) for the task (the Playwright observation size from T0a-thinking.tsv).
  The cap cuts on a char boundary and appends `[truncated: navigate observation capped at N bytes]`.

Driver: `bench/agent-tasks/w5t0aa_run.sh`. Gate and summary: `bench/agent-tasks/w5t0aa_causal.py`.
Rows: `T0a-a-{nocap,cap}.tsv` (+ `.turns.tsv`), per-task table `T0a-a-causal.tsv`. Transcripts stay in ~/.cache/manuk-t0aa.

## Result
- Truncation applied: navigate results over the cap, nocap 33 runs, cap 0. Truncation notes in the cap arm: 42.
- Paired thinking difference (cap − nocap), mean per task: **−11.3 tokens**, 95% bootstrap band **[−81.8, +49.6]** (seed 20261114, 10k resamples over tasks).
- Tasks with lower thinking under cap: 5 of 12.
- Pass (of 36 runs per arm): nocap 24, cap 26.

## Reading
The band spans zero and the sign is mixed, so a smaller navigate observation does not measurably cut thinking on these 12 tasks.
The T0a correlation was therefore not a sign that observation size drives the thinking gap. The gap is more likely
task identity or turn count, which this design does not separate. Pass counts are within noise (24 vs 26 of 36).

## Gate evidence
- Real data: GATE GREEN, exit 0.
- `--mutant` (nocap data judged as the cap arm): GATE RED, 90 violations, exit 1 (e.g. `RED cap X06 r3: navigate result 3135 bytes > cap 1730+120`).

## Caveats
- One chrome_ver, one model, 3 runs per task-arm; per-task means of 3 runs are noisy.
- The cap is a fixed head-truncation of the navigate text, so the cap arm also loses content; pass counts do not show a penalty here, but this is not a semantics-preserving snapshot.
