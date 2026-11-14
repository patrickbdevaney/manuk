# W5.T0.a.b: per-turn thinking tokens in the harness (chrome_ver=145.0.7632.116)

What changed: `bench/agent-tasks/run_arm.py` writes a `thinking_tokens` column to every `<out>.turns.tsv` row.
Stream-json redacts thinking text (the block is present, its text is empty), so per-turn thinking is apportioned
from the run result's `output_tokens_details.thinking_tokens` over the turns that carry a thinking block, weighted
by each turn's output tokens. Turns without a thinking block get 0. Column sum equals the run total exactly.

Gate: `bench/agent-tasks/w5t0ab_turn_thinking.py`
- offline (stored transcript `docs/sprint/w5/L2a-transcripts/manuk_L2a_r1.jsonl`, 9 turns): sum 149 vs result 149, GREEN;
  `--mutant` (column dropped): sum 0 vs 149, RED.
- real smoke (one task L01, one run per arm; `docs/sprint/w5/t0ab-smoke/`):
  - playwright: turn sum 112 vs result 112 (turns 1 and 4 carry thinking: 34 and 78).
  - manuk: turn sum 0 vs result 0 (result key present with value 0; no thinking in this run).
  - GREEN. `--mutant` (column dropped from the read rows): RED, "no thinking_tokens column", "no runs checked".

Run: `python3 -I bench/agent-tasks/w5t0ab_turn_thinking.py [RUNDIR] [--mutant]`.
Earlier turn logs (no thinking column) are unchanged; consumers read columns by name.
