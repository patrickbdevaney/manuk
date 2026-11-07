# W5.T0.a: why manuk's Haiku thinks more (chrome_ver=145.0.7632.116)

Source: the W4.B4 runs (own 40 + external 14 = 54 tasks x 3 runs, arms manuk and playwright+preload), 324 stream-json
transcripts in ~/.cache/manuk-b4/transcripts, joined to docs/sprint/w4/B4/<arm>.tsv (pass, chrome_ver) and
<arm>.tsv.turns.tsv (per-turn result_bytes). Script: `python3 -I bench/agent-tasks/w5t0a_thinking.py
~/.cache/manuk-b4/transcripts docs/sprint/w4/B4 docs/sprint/w5/T0a-thinking` (writes T0a-thinking.tsv, one row per task).

## Measurement
- `thinking_tokens` is the exact per-run count `usage.output_tokens_details.thinking_tokens` from the result line. The
  thinking text itself is empty in every transcript (redacted: 0 of 648 runs carry thinking characters), so the usage
  count is the only available measure. All 648 runs have the field; 62 playwright+preload runs and 13 manuk runs have 0.
- `obs_prev` = the mean size, in bytes of text, of the tool result the model read before each turn after the first.
  turns.tsv `result_bytes` on turn i are the results turn i issued, so the observation read before turn i+1 is turn i's
  value. The join is checked: the turns.tsv bytes summed per run must equal the tool-result text bytes recomputed from
  that run's transcript. The 162+162 checked runs all pass.
- Eligible tasks: both arms pass at least 2 of 3 runs. 48 of 54 tasks.

## Answers (n = 48 eligible tasks unless stated)
1. **Concentrated or spread.** Spread, with a heavy head. Mean per-task gap is +92.5 thinking tokens per run. 38 of 48
   tasks have a positive gap. The top 10 tasks hold 61% of the positive gap (3,400 of 5,603). The gap is not confined to
   10 tasks. Top tasks: X05 +519, L03 +457, X06 +455, L07 +386, L09 +295, L17 +289, L13 +265, X08 +258, L10 +257, F11 +219.
2. **Does it track observation bytes?** Weakly. Across tasks, Pearson(thinking gap, obs gap) = 0.06 and Spearman = 0.30.
   Within manuk alone, Pearson(thinking_manuk, obs_prev_manuk) = 0.14 and Spearman = 0.42. At run level for manuk
   (n = 162), Pearson(obs_prev, thinking per turn) = 0.19 and Spearman = 0.41. The rank association is moderate and the
   linear one is small. Observation size is not the main driver of the gap. Task identity and the number of turns are
   confounded with it and are not controlled here.
3. **Turns.** Per-task turn counts differ between arms (for example X05: 13.3 manuk vs 4.3 pw+preload). Thinking is
   reported per run, not per turn, so per-task thinking mixes more turns with more thinking per turn.

## Gate evidence
- Script on real data: 48 eligible tasks, RED 0, exit 0.
- Red mutant `--shuffle-join` (each task's turn log is taken from the next task): RED 324 runs, exit 1 (for example
  `RED join playwright+preload X14 r3: turns bytes 795 vs transcript 1312`).

## Caveats
- One chrome_ver, one model (Haiku), 3 runs per task-arm. Per-task means of 3 runs are noisy.
- Correlations pool tasks and are not adjusted for task or turn count. They are descriptive, not causal.
- Causal test (not run here): truncate manuk's navigate observation to the playwright-snapshot size and re-measure
  thinking. That is follow-up W5.T0.a.a.
