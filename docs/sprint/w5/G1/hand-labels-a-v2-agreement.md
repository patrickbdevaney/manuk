# W5.G1.b.b.a.a.a: hand-label test of the (a) perceive fallback on the v2 re-run (GATE RED)

chrome_ver=145.0.7632.116. Gate: agreement >= 9/10 with `causes-v2.tsv`. **Result: 5/10. Gate RED. Not redrawn.**
Checker: `bench/agent-tasks/w5g1_agree_check.py G1/hand-labels-a-v2.tsv --causes G1/causes-v2.tsv` exits 1 (5/10); exits 0 (10/10) on the same file with labels set equal to the classifier.

## Draw
Seed 20261116 (recorded in the GRAPH row before the draw). Frame: the 14 `perceive` rows of arm cdp-layer in `causes-v2.tsv`;
`random.Random(20261116).sample(sorted (task,run), 10)`. Transcripts resolved via the archived `<dir>.<ts>` (v2). Labels were written from
transcripts before the classifier column was compared. Labels use the five classes, or `none` when the run ended for a reason that is none of them.

| task | run | hand | classifier | agree |
|---|---|---|---|---|
| gomail-5 | 1 | perceive | perceive | yes |
| staynb-2 | 3 | none | perceive | no |
| opendining-10 | 3 | perceive | perceive | yes |
| gomail-8 | 2 | none | perceive | no |
| gomail-8 | 1 | none | perceive | no |
| opendining-10 | 1 | perceive | perceive | yes |
| opendining-10 | 2 | perceive | perceive | yes |
| gomail-5 | 3 | perceive | perceive | yes |
| dashdish-10 | 2 | target | perceive | no |
| opendining-4 | 1 | none | perceive | no |

## Findings (labels not changed)
- The 5 agreeing rows are real perceive failures (opendining menu sections render empty; gomail date/time picker options not found).
- 4 disagreements are `none`: the run's own stop was not a failure of the five classes (staynb-2 r3 stopped on a payment-safety decision after
  reaching the booking page; opendining-4 r1 stopped on a site bug; gomail-8 r1/r2 show four emails verified in Trash and no blocker, so why they
  count as failed is not explained by the transcript, grader not examined).
- 1 disagreement is a real miss: dashdish-10 r2 is `target` (modal overlay, no close control, stopped), classified perceive.
- Consequence: the 14 (a) perceive runs are not explained by the classifier; no (a) cause may be claimed from it. The five-class scheme has no
  bucket for "run stopped by choice / site bug / possibly graded wrong".
