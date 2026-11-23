# W5.G1.b.d.b.a.a.a: fresh full-read blind label of the 35 v7r failed rows, banner and clause (g) stated precisely (gate met 33/35; refusal-seen positives 1/3, stated NOT validated)

chrome_ver=145.0.7632.116. Rule: "Precise banner and clause (g) text" in G1.md, committed (before labelling) on top of the complete rule. No thresholds, no classifier change.
Labels `hand-labels-mech-v7r3.tsv` from a fresh subagent given only the rule text and the 35 transcripts, told to read each in full, forbidden classifier code, causes files and earlier labels.

`w5g1_agree_check.py hand-labels-mech-v7r3.tsv --causes causes-pwfmt-v7r.tsv --thresh 32`: **33/35, rc 0.** Mutated file (4 labels flipped): 30/35, rc 1.
Labeller 33 stop-by-choice / 2 refusal-seen; classifier 33 / 2 (v7r2 labeller: 34 / 1).

**Refusal-seen positives: 1 of 3 agree** (3 rows either side calls refusal-seen; below the >=2/3 bar, so stated, not met):
- fly-unified-9 r1: both refusal-seen (clause b).
- dashdish-4 r1: labeller refusal-seen (3 `no change` results after last success), classifier stop-by-choice. Cause: the actual result text is `no change` NEWLINE `url: unchanged`,
  and `w5g1_report.FAIL` only matches the ` | ` form, so the classifier counts those results as SUCCESSES (they move the last-success point past them), while `real_classify` RX["nochange"] would
  accept the newline form. The G1.md text ("starts with `no change | url: unchanged`") matches the FAIL regex literally but a human reads the newline form as the same result. Two regexes in one classifier disagree; the rule never says which.
- fly-unified-5 r1: classifier refusal-seen by clause (g) (agent text with a typed-fail phrase plus a type call); labeller found no clause fired (missed (g) despite the stated text).
The banner fix worked (fly-unified-9 r1 now agrees; staynb-4 r1 no longer a disagreement). refusal-seen remains an unvalidated class.
Follow-up W5.G1.b.d.b.a.a.a.a: decide the newline `no change` form (state it as failure in the rule and align FAIL, or state it as success), re-label.
