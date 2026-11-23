# W5.G1.b.d.b.a.a.a.a: newline-form `no change` stated as a failure and aligned in w5g1_report.FAIL; fresh blind label of the 35 v7r failed rows agrees 34/35 (refusal-seen positives 2/3, bar met)

chrome_ver=145.0.7632.116. Reading chosen: `no change | url: unchanged` and `no change` NEWLINE `url: unchanged` are one result shape, both failures (G1.md, "Newline form" bullet, committed with the code before the label). FAIL regex now `^no change[ \t]*(?:\||\n)[ \t]*url: unchanged`; test_newline_form_nochange_is_failure added (12 tests pass; red with the old regex). No thresholds changed.
Classifier rerun on the same 60 v7r runs -> `causes-pwfmt-v7r4.tsv`: 32 stop-by-choice / 3 refusal-seen (v7r: 33 / 2); the only change is dashdish-4 r1 -> refusal-seen.
Labels `hand-labels-mech-v7r4.tsv`: fresh subagent given only the rule text and the 35 transcripts; forbidden classifier code, causes files, earlier labels. Caveat: it read assistant texts in full but truncated long tool results (~220 chars) and scanned them by phrase.

`w5g1_agree_check.py hand-labels-mech-v7r4.tsv --causes causes-pwfmt-v7r4.tsv --thresh 32`: **34/35, rc 0.** Mutated file (4 labels flipped): 30/35, rc 1.
Refusal-seen positives (3 rows either side calls it): **2 of 3 agree** (>= 2/3 bar met): dashdish-4 r1 and fly-unified-9 r1 both refusal-seen (clause b).
Remaining disagreement: fly-unified-5 r1, classifier refusal-seen via clause (g), labeller stop-by-choice (labeller noted the agent wrote "didn't register" but used bare `fill_form`; the classifier's clause (g) fired on a type-family call it found). Not resolved here.
