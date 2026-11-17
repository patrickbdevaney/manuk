# W5.G1.b.c.a.a.a.a.a.a: fifth hand-label draw, unexplained-aware classifier (GATE RED)

chrome_ver=145.0.7632.116. Gate: agreement >= 9/10 with `causes-pwfmt-v3.tsv`. **Result: 6/10. Gate RED. Not redrawn.**
Checker: `python3 -I bench/agent-tasks/w5g1_agree_check.py G1/hand-labels-v5.tsv --causes G1/causes-pwfmt-v3.tsv` exits 1 (6/10); exits 0 (10/10) with labels set equal to the classifier.

## Draw
Frame: a NEW 30-run cdp-layer+pwfmt batch (`cdp-layer+pwfmt-v3.tsv`, 29/30 rows tied to their transcript; opendining-10 r3 has no result event and is excluded), 13 classed failed rows
in `causes-pwfmt-v3.tsv` (arm `cdp-layer+pwfmt-v3`), disjoint from hand-labels v1-v4 (different runs). Seed 20261117:
`random.Random(20261117).sample(sorted (task,run) keys, 10)`. Labels were written by a separate subagent that was told not to read any causes file or run the classifier
(it read the gomail-8 task's eval spec). The earlier class `none` is now called `unexplained`, matching the classifier.

| task | run | hand | classifier | agree |
|---|---|---|---|---|
| fly-unified-6 | 3 | unexplained | target | no |
| fly-unified-6 | 1 | perceive | act | no |
| fly-unified-6 | 2 | act | unexplained | no |
| gomail-3 | 1 | unexplained | unexplained | yes |
| opendining-4 | 2 | unexplained | unexplained | yes |
| opendining-4 | 1 | target | unexplained | no |
| gomail-8 | 2 | grader-issue | grader-issue | yes |
| gomail-8 | 3 | grader-issue | grader-issue | yes |
| staynb-4 | 3 | unexplained | unexplained | yes |
| opendining-10 | 2 | perceive | perceive | yes |

## Findings (labels not changed)
- The fix holds where it was aimed: all 4 hand `unexplained` rows with a stop-by-choice agree with the classifier except fly-unified-6 r3 (classifier: target); the perceive fallback no longer inflates perceive (3 of 13 vs 14 of 18).
- All 4 disagreements are on fly-unified-6 (3) and opendining-4 r1 (1): the date-picker tasks. The classifier reads act/target/unexplained from keywords in the transcript where the hand reading
  follows what the agent said it could not do (year dropdown not exposed = perceive; refused focus = act; wrong year picked = target).
- Hand labels on fly-unified-6 are themselves judgement calls (r3 could be perceive); 6/10 with 3 of 4 misses on one task means the classifier is not validated and no cause may be claimed from `causes-pwfmt-v3.tsv`.
