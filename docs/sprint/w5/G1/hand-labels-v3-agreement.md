# W5.G1.b.c.a.a: third hand-label draw on the revised classifier (GATE RED)

chrome_ver=145.0.7632.116. Gate: agreement >= 9/10 with `causes.tsv` (classifier of W5.G1.b.c.a). **Result: 7/10. Gate RED. Not redrawn.**
Checker: `bench/agent-tasks/w5g1_agree_check.py G1/hand-labels-v3.tsv` exits 1 (7/10); exits 0 on a labels file set equal to causes.tsv (10/10).

## Draw
- Seed 20261115 (from the GRAPH row, before the draw). Frame: `causes.tsv` rows with a class, minus the dev rows of `hand-labels.tsv`
  = 14 rows, all cdp-layer+pwfmt. Fewer than 10 remained after also excluding the 10 v2 rows (4), so per the row's fallback the v2 rows
  stay in the frame. `random.Random(20261115).sample(sorted keys, 10)`. 6 of the 10 were also in the v2 draw; the classifier was changed
  after seeing v2 (W5.G1.b.c.a), so this is not an independent validation for those 6.
- Labels written from transcripts and committed (hand-labels-v3.tsv) before `causes.tsv` was read for these rows.

## Agreement
| task | run | hand | classifier | agree |
|---|---|---|---|---|
| gomail-3 | 1 | target | target | yes |
| dashdish-10 | 3 | target | target | yes |
| gomail-5 | 1 | act | act | yes |
| opendining-10 | 2 | perceive | target | no |
| opendining-10 | 1 | perceive | target | no |
| fly-unified-6 | 1 | target | target | yes |
| gomail-3 | 2 | act | act | yes |
| dashdish-7 | 2 | target | perceive | no |
| opendining-4 | 1 | perceive | perceive | yes |
| fly-unified-6 | 2 | target | target | yes |

## Findings (labels not changed)
- The two `act` fixes hold (gomail-3 r2, gomail-5 r1 now agree).
- opendining-10 r1 and r2 are still `target`: they saw overlay text and some action result failed (STALE/OCCLUDED on unrelated steps), so the
  "overlay text + any failed action" rule fires although the overlay was not what stopped the run.
- dashdish-7 r2 is now `perceive` but the run stopped on an overlay that Escape could not clear (8 turns, no other blocker): the new rule
  needs an action failure after the overlay, and the Escape press returned no failure text.
Follow-up needed: rule changes must be validated on rows not in v1/v2/v3, so the verified frame needs enlarging.
