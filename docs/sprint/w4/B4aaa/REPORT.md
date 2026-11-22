# W4.B4.a.a.a — in+out tokens/task, manuk (lean observe) vs playwright+preload (chrome_ver=145.0.7632.116)

Own suite (40 tasks) x3 runs, Haiku, run_arm.py, both arms run concurrently on one box; binary target/release/manuk-mcp (02:10 build).
in+out = input_tokens + output_tokens (cache tokens excluded, as in the B4 VERDICT metric).

| arm | pass per run (of 40) | in+out/task per run | mean | out/task mean |
|---|---|---|---|---|
| manuk | 39, 38, 39 | 576, 492, 521 | 529 | 522 |
| playwright+preload | 39, 39, 38 | 459, 473, 467 | 466 | 459 |

Gap manuk vs playwright+preload: +13.6% in+out (B4 reference was +40%); bands do not overlap-below (manuk 492-576, playwright+preload 459-473), so manuk is still above Playwright+preload; Bar 2.2 tokens (>=30% fewer) is not met on this metric. Pass bands equal (38-39/40 both).
Almost all of in+out is output tokens (input_tokens excludes cache). Not measured here: cache-inclusive total and USD per task (follow-up).
Gate bench/agent-tasks/b4aaa_gate.py: exit 0 (VALID); `--mutate swap` exit 1 (arm mismatch). See GATE.txt.
