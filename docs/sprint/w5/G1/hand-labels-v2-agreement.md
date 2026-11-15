# W5.G1.b.c: second hand-label draw on the verified frame (GATE RED)

chrome_ver=145.0.7632.116. Gate: agreement >= 9/10 with `causes.tsv`. **Result: 5/10. Gate RED. Not redrawn.**

## Draw
- Seed 20261114, recorded in the GRAPH.tsv row before the draw.
- Frame: rows of `G1/causes.tsv` (regenerated in W5.G1.b.b from the 30 pwfmt rows that pass `w5g1_transcript_check`) with a class
  (gomail-5 r1 has `None`), minus the 6 W5.G1.b dev-set rows (so the dev set is not reused as validation). 13 rows; sorted by
  (arm, task, run); `random.Random(20261114).sample(keys, 10)`. All 10 are cdp-layer+pwfmt; the cdp-layer arm has no verified rows.
- Labels in `G1/hand-labels-v2.tsv` were written from the transcripts before `causes.tsv` was read for these rows. Class
  definitions: the `w5g1_report.py` docstring (`act` = browser-side refusal/no-effect the agent saw in a result; the rest is the model's choice).

## Agreement
| task | run | hand | classifier | agree |
|---|---|---|---|---|
| dashdish-10 | 3 | target | target | yes |
| dashdish-7 | 2 | target | target | yes |
| fly-unified-6 | 1 | target | target | yes |
| fly-unified-6 | 2 | target | target | yes |
| gomail-3 | 3 | act | target | no |
| gomail-5 | 3 | act | target | no |
| opendining-10 | 1 | perceive | target | no |
| opendining-10 | 2 | perceive | target | no |
| staynb-2 | 1 | target | target | yes |
| staynb-4 | 1 | perceive | target | no |

All 5 disagreements are the classifier saying `target`; it said `target` for all 10 draws (and for 19 of 20 verified rows).

## Findings (not used to change labels)
- `target` is the fall-through for any run whose transcript mentions an overlay/occlusion/dialog text (`f["occl"]`, `f["dialog_text"]`),
  so runs that merely saw an overlay (opendining-10 r1 completed its booking; staynb-4 r1 stopped at payment by choice; opendining-10 r2
  could not read menus) are labelled `target`.
- gomail-3 r3 and gomail-5 r3: every type/focus/fill_form call was STALE and never landed, but the `act` rule did not fire. The
  "after the last successful action" window likely resets on a later successful click or open; the rule needs a check against these two.
- The classifier labels 19 of 20 verified failures `target`, so it carries little discriminating power on this frame; the G1 top-class claim rests on it.
