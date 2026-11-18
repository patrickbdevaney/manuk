# W5.G1.b.c.a.a.a.a.a.a.a: date-picker rules + fresh blind draw v6 (GATE RED on the fresh draw)

chrome_ver=145.0.7632.116. Classifier change (w5g1_report.py): the agent's own "not exposed to my tools" statement -> perceive; a "refused focus" result -> act; the overlay/dialog `target` path needs an unrecovered failed action.
- v5 rows (same rows, regenerated causes): `w5g1_agree_check.py hand-labels-v5.tsv` 9/10, rc 0 (was 6/10). Remaining miss: opendining-4 r1 (wrong year picked, hand = target). Same-row agreement is not validation.
- Fresh batch: `cdp-layer+pwfmt-v4.tsv`, 30 runs, 30/30 tied to transcripts (w5g1_transcript_check rc 0), 14 failed rows in `causes-pwfmt-v4.tsv`. Seed 20261118 drew 10; labels by a separate subagent that was told not to read causes files or run the classifier.
- Result: **5/10, rc 1. Gate RED. Not redrawn, rules not re-tuned on these rows.**

| task | run | hand | classifier |
|---|---|---|---|
| opendining-4 | 1 | unexplained | unexplained |
| gomail-3 | 3 | grader-issue | unexplained |
| gomail-5 | 2 | perceive | perceive |
| fly-unified-6 | 2 | perceive | unexplained |
| fly-unified-6 | 1 | perceive | act |
| opendining-10 | 3 | perceive | perceive |
| opendining-4 | 3 | act | unexplained |
| opendining-10 | 1 | perceive | perceive |
| fly-unified-6 | 3 | perceive | unexplained |
| gomail-8 | 1 | grader-issue | grader-issue |

Misses: fly-unified-6 x3 (hand perceive: picker year control unreachable; classifier needs an explicit exposure phrase), gomail-3 r3 (declined to send: hand reads it as a grader mismatch, classifier unexplained), opendining-4 r3 (country option not scrollable: hand act). Hand labels on these are themselves judgement calls (v5 hand-labelled fly r3 unexplained, v6 perceive), so the labelled-agreement gate is not reliably reachable by keyword rules; no cause may be claimed from causes-pwfmt-v4.tsv.
