chrome_ver=145.0.7632.116; W4.B4.q: turns per task on the agent bench after W4.B3 (navigate(url, query) + extract(query)).
Suite = full (own 40 + external 14, 3 runs); Haiku; source docs/sprint/w4/B4q/*.tsv (run_arm.py, MANUK_MCP_BIN = HEAD release build).
Baseline = W4.B1 own-suite manuk arm (SCOREBOARD, suite 69903c7d). Mutant = same HEAD binary with MANUK_QUERY_MUTATE=ignore.
Turns = assistant turns per run (tool turns + the final answer turn). Report script: bench/agent-tasks/b4q_turns_report.py.

baseline (W4.B1, own only): 40 tasks x 3 runs (120 own rows, 0 external rows)
  own turns/task 4.97  band [4.72, 5.17]
  pass per run [39, 39, 38] of 40; tasks passing >=2/3: 39/40
  L-tasks (25): median of task medians 3.0; task medians <=2.5: 5/25

manuk (HEAD, query honoured): 54 tasks x 3 runs (120 own rows, 42 external rows)
  own turns/task 3.83  band [3.70, 3.90]
  pass per run [50, 50, 50] of 54; tasks passing >=2/3: 50/54
  L-tasks (25): median of task medians 3.0; task medians <=2.5: 9/25
  external turns/task 6.74 (no pre-W4.B3 external arm exists; compare with noquery)

manuk-noquery (mutant): 54 tasks x 3 runs (120 own rows, 42 external rows)
  own turns/task 4.05  band [4.03, 4.08]
  pass per run [51, 53, 51] of 54; tasks passing >=2/3: 52/54
  L-tasks (25): median of task medians 3.0; task medians <=2.5: 7/25
  external turns/task 6.19 (no pre-W4.B3 external arm exists; compare with noquery)

own turns: manuk 3.83 vs noquery 4.05 (mutant should be higher)
baseline own turns 4.97 (the W4.B1 journal says 4.97)
