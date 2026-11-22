# W5.G1.b.d.b.a: blind label of all 35 failed rows of cdp-layer+pwfmt-v7r (GATE NOT MET, 24/35)

chrome_ver=145.0.7632.116. Batch `cdp-layer+pwfmt-v7r.tsv` (60 runs, 35 failed). Labels `hand-labels-mech-v7r.tsv` (sha256 36021fab...7c17,
written 06:37, after `causes-pwfmt-v7r.tsv` existed at 05:33, but the labeller was forbidden to read it and any classifier code) came from a
separate subagent given only the transcripts and the G1.md rule text. It labelled with a keyword script over the first 300 chars of each
result, not a full read of each transcript; that is a limit of this check.

Result: `w5g1_agree_check.py hand-labels-mech-v7r.tsv --causes causes-pwfmt-v7r.tsv --thresh 32` = **24/35, rc 1 (needs 32).**
Classifier: 22 stop-by-choice / 2 refusal-seen (fly-unified-5 r1, fly-unified-9 r1). Labeller: 22 stop-by-choice / 13 refusal-seen.
Confusion: stop-by-choice/stop-by-choice 22; refusal-seen/refusal-seen 2; classifier stop-by-choice vs labeller refusal-seen 11; the reverse 0.

Refusal-seen positives: the classifier's 2 are both hand-labelled refusal-seen (2/2), but the labeller found 11 more that the classifier calls
stop-by-choice, so refusal-seen is NOT validated: agreement on rows either side calls refusal-seen is 2/13.

Why they disagree: the stated rule says "a refusal/no-effect result after the last successful action". The classifier additionally needs
`no change` >=2 (or STALE >=2) after that point, and its vocabulary omits `UNREACHABLE`, `OCCLUDED`, `read-only` and `is unknown` on non-handle
tools. The 11 disagreements are exactly single `no change`, `UNREACHABLE`, `OCCLUDED`, `read-only`, `is unknown` results after the last success.
The stated rule is under-specified (count threshold and result vocabulary are in code, not in the rule), so a reader cannot reproduce the classifier from it.
Rules were not changed here (pre-registered). Gate can go red: it did.
