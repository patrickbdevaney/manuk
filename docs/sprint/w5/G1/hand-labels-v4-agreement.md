# W5.G1.b.c.a.a.a.a: fourth hand-label draw on the fresh frame (GATE RED)

chrome_ver=145.0.7632.116. Gate: agreement >= 9/10 with `causes-pwfmt-v2.tsv`. **Result: 5/10. Gate RED. Not redrawn.**
Checker: `python3 -I bench/agent-tasks/w5g1_agree_check.py G1/hand-labels-v4.tsv --causes G1/causes-pwfmt-v2.tsv` exits 1 (5/10); exits 0 (10/10) on the same rows with labels set equal to the classifier.

## Draw
Seed 20261116. Frame: the 18 classed rows of `causes-pwfmt-v2.tsv` (arm `cdp-layer+pwfmt-v2`, transcripts under `cdp-layer+pwfmt_*`), disjoint from hand-labels v1-v3
(those are the earlier 30-run set). `random.Random(20261116).sample(sorted keys, 10)`. Labels were written from transcripts and committed
(hand-labels-v4.tsv) before the class column was read. `none` = the run's stop was none of the classes.

| task | run | hand | classifier | agree |
|---|---|---|---|---|
| gomail-5 | 1 | perceive | perceive | yes |
| staynb-4 | 2 | grader-issue | perceive | no |
| opendining-10 | 3 | perceive | perceive | yes |
| opendining-4 | 1 | grader-issue | perceive | no |
| gomail-3 | 2 | none | perceive | no |
| gomail-8 | 1 | grader-issue | grader-issue | yes |
| opendining-10 | 1 | perceive | target | no |
| opendining-4 | 2 | none | perceive | no |
| gomail-8 | 2 | grader-issue | grader-issue | yes |
| dashdish-7 | 1 | perceive | perceive | yes |

## Findings (labels not changed)
- Every perceive-labelled row (4) agrees except opendining-10 r1, which the classifier calls `target` although no overlay stopped it (same
  overlay-text + failed-action over-trigger seen in v3).
- Two disagreements are runs that completed the task as asked (staynb-4 r2 shows "3 Guests" on confirmation, opendining-4 r1 shows Booking
  Complete); eval 0 with no failure in the transcript. The classifier's perceive fallback labels them
  perceive; only the hard-coded GRADER_ISSUE set (gomail-8) catches grader issues. Not explained by the transcripts; grader not examined.
- Two `none` rows: gomail-3 r2 stopped at the draft by choice, opendining-4 r2 refused to continue on a booking page that opened with default
  date/size. The perceive fallback absorbs runs that are not perception failures, as in the a-v2 draw.
- Consequence: the perceive count in causes-pwfmt-v2.tsv (14 of 18) overstates perception failures; no cause may be claimed from it.
