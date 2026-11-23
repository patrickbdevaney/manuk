# W6.C2.a: cdp-layer REAL-fast x3 with evaluate / act force / act at exposed

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Job: bench-queue/specs/w6c2a-cdp-layer.json, result bench-queue/results/w6c2a-cdp-layer-4f615639fd983f62/band.tsv (job wall 7349 s, 0 interventions, 1 auto_retry). Binary: manuk-mcp built from 499e5fba (the W6.C2 commit). Verdict produced by bench/agent-tasks/w6c2a_tally.py (self-test green), not hand-computed.

```
| run | evaluate | act_force | act_at | tool_calls | passes/17 | mean turns |
|---|---|---|---|---|---|---|
| r1 | 52 | 10 | 20 | 464 | 10 | 28.47 |
| r2 | 39 | 9 | 16 | 472 | 4 | 29.00 |
| r3 | 64 | 1 | 23 | 469 | 9 | 28.94 |

new: task_pass_2of3=9/17  per-run=[10, 4, 9]  mean turns=28.80
W6.B0 same 17 tasks: task_pass_2of3=8/17  per-run=[9, 9, 8]  mean turns=25.18
evaluate used in 3/3 runs
pass band (8, 9) in=True  turn band (23.06, 26.53) in=False
falsifier does not fire
```

Falsifier (pre-registered, GRAPH.tsv W6.C2.a): fires only when evaluate is used in <10% of runs AND task pass and mean turns both sit inside the W6.B0 band. evaluate is used in 3/3 runs (52, 39, 64 calls), so it does not fire.

Not in the verdict, recorded for honesty: mean turns 28.80 is above the W6.B0 same-task band (23.06-26.53), and pass per run 10/4/9 is wide. The job-level band.tsv reports in_band=1 on task_pass_2of3, which is a 0-17 band and says little.
