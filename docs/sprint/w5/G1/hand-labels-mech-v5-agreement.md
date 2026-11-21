# W5.G1.b.d: blind check of the mechanical classes on a fresh batch (GATE RED, 8/10)

chrome_ver=145.0.7632.116. Batch `cdp-layer+pwfmt-v5.tsv`: 30 runs (10 tasks x 3), 30/30 tied to transcripts (w5g1_transcript_check rc 0), 15 failed rows.
Seed 20261121 was written to the GRAPH row before the draw; `random.sample` of the 15 failed rows sorted by (task, run), 10 drawn.
Labels (`hand-labels-mech-v5.tsv`) were written by a separate subagent from transcripts only, forbidden from reading causes files or the classifier, before the class column was read (`causes-pwfmt-v5.tsv`).
Result: `w5g1_agree_check.py hand-labels-mech-v5.tsv --causes causes-pwfmt-v5.tsv` = **8/10, rc 1. Gate RED. Not redrawn.** Control: labels set equal to the classifier give rc 0.

Misses: dashdish-7 r3 and staynb-2 r3 (hand refusal-seen, classifier stop-by-choice): the hand reader counted a refusal/no-effect result near the end that the classifier's "after the last successful action" window did not. The labeller had no task text (transcripts omit the prompt), so it could not assign grader-issue; the classifier's gomail-8 grader-issue is a fixed rule, not a per-run call.
So the refusal-seen vs stop-by-choice boundary is not reproducible at 9/10 even for the mechanical classes; counts of refusal-seen must not be quoted as validated.
