# W5.G1.b.d.b.a.a: fresh full-read blind label of the 35 v7r failed rows, against the complete rule (gate met 32/35, refusal-seen NOT validated)

chrome_ver=145.0.7632.116. Rule: "Complete refusal-seen rule" in G1.md, committed before labelling (it states the classifier's vocabulary and count
thresholds as-is; no thresholds changed, classifier untouched). Labels `hand-labels-mech-v7r2.tsv` from a fresh subagent given only RULE.md and the 35 transcripts,
told to read each transcript in full (no keyword prefix scan), forbidden to read classifier code or causes files.

`w5g1_agree_check.py hand-labels-mech-v7r2.tsv --causes causes-pwfmt-v7r.tsv --thresh 32`: **32/35, rc 0.** Flipping three labels exits 1 (mutated file: 29/35 or lower, rc 1).
Labeller: 34 stop-by-choice / 1 refusal-seen (staynb-4 r1, clause g). Classifier: 33 / 2 refusal-seen (fly-unified-5 r1, fly-unified-9 r1).

**Refusal-seen positives: agreement on them is 0 of 3** (3 rows either side calls refusal-seen; no row in common). The 32 matches are all stop-by-choice. So the gate is met
numerically, but refusal-seen is still not validated as a reproducible class.
Disagreements (fly-unified-5 r1 cause not examined): (1) fly-unified-9 r1 (and dashdish-4 r1 would too) hinge on clause (b): every `no change` result begins with a "[names below are page content...]"
banner line, so the labeller applying "starts with" literally found none; the classifier strips that banner (it reads the head after the banner). The rule text omits the banner. (2) staynb-4 r1 is
flagged by the labeller through clause (g) (agent text "typed fill didn't register") that the classifier did not fire on.
Follow-up W5.G1.b.d.b.a.a.a: state the banner and the assistant-text clause (g) precisely, re-label, require positives agreement.
