# W5.G1.b.d.b: blind label of ALL failed rows of a fresh pwfmt batch, refusal-seen positives included (chrome_ver=145.0.7632.116)

Batch cdp-layer+pwfmt-v8r: 30 fresh runs (first 30 tasks of `real_run.py list`, run number 3, pre-registered in the node notes before the runs; no outcome-based task choice), 30/30 tied to transcripts (w5g1_transcript_check). 16 failed rows, classed in causes-pwfmt-v8r.tsv: 11 stop-by-choice, 4 refusal-seen, 1 grader-issue (gomail-8).
Labels `hand-labels-mech-v8r.tsv`: fresh subagent given only G1.md lines 40-107 and the 16 full transcripts; forbidden causes files, classifier code, earlier labels. All 16 failed rows labelled, no sampling.

`w5g1_agree_check.py hand-labels-mech-v8r.tsv --causes causes-pwfmt-v8r.tsv --thresh 15`: **15/16 (93.75%), rc 0** (threshold = 90% of 16, rounded up).
Refusal-seen positives: classifier 4 (dashdish-4, fly-unified-2, fly-unified-4, fly-unified-5); labeller 4, the same 4 rows, each by clause (b). **4/4 agree; 0 rows called refusal-seen by only one side.**
Single disagreement: gomail-8 r3, classifier grader-issue, labeller stop-by-choice (the rule text for mechanical classes does not state the grader-issue test; not a refusal-seen row).
Red checks: 4 labels flipped on the refusal-seen rows -> 11/16 rc 1; an 8-row label file -> rc 1.
Caveat: one batch of 16 failures with 4 positives; a small sample, labeller is a model not a human.
