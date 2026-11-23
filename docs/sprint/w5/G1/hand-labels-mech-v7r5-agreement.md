# W5.G1.b.d.b.a (re-run): fresh blind label of the 35 v7r failed rows under the complete stated rule: 35/35 with the classifier at HEAD, refusal-seen positives 2/2

chrome_ver=145.0.7632.116. This node was BLOCKED at 24/35 (`hand-labels-mech-v7r-agreement.md`) because the rule was under-stated; its
follow-ups (.a ... .a.a.a.a.a.a) completed the rule in G1.md and aligned the code. This is the node's own gate re-run on the same 35 rows.

- **Classifier at HEAD.** `w5g1_report.py --b-tsv cdp-layer+pwfmt-v7r.tsv --b-label cdp-layer+pwfmt-v7r` reproduces `causes-pwfmt-v7r4.tsv`
  byte-for-byte on its 35 rows: 33 stop-by-choice / 2 refusal-seen (dashdish-4 r1, fly-unified-9 r1). All 35 transcripts tie to their rows (answer match).
- **Labeller inputs.** `blind-rule-v7r5.md` = G1.md rule text (steps 1-5, banner, clause (g), newline form, line-wise matching, final answer excluded)
  with row-specific notes removed; transcripts rendered by `bench/agent-tasks/w5g1_render_blind.py` (full results, no truncation, results paired to calls
  by tool_use_id). Three fresh subagents (12/12/11 rows), forbidden the repo, classifier code, causes files and earlier labels. Each used its own phrase
  script plus an eye-read of every result after the last success and every non-final assistant text, and reports the two agreed.
- **Discarded first round.** A first labelling round was run on a RULE file I staged wrongly (it cut off the end of clause (g) and dropped the
  newline-form and line-wise bullets) and on a renderer that numbered parallel results wrongly. Its labels were thrown away before any comparison
  with the classifier. In that round one labeller called staynb-4 r1 refusal-seen via (g) on a bare `fill_form`, which the full text excludes.
- **Gate.** `w5g1_agree_check.py hand-labels-mech-v7r5.tsv --causes causes-pwfmt-v7r4.tsv --thresh 32`: **35/35, rc 0**.
  Against the original `causes-pwfmt-v7r.tsv` (older code, as the gate row names it): 33/35, rc 0; the 2 differences are the two rows the
  follow-ups changed in code (dashdish-4 r1 newline-form no-change; fly-unified-5 r1 final-answer clause (g)).
  Mutant (first 4 labels flipped): 31/35, rc 1.
- **Refusal-seen positives.** Classifier 2 (dashdish-4 r1, fly-unified-9 r1); both hand-labelled refusal-seen (clause (b): 3 and 2 no-change results
  after the last success). The labeller added none. So refusal-seen agrees 2/2, but 2 positives are below the >=3 the parent asked for; the
  v8r batch (4/4, W5.G1.b.d.b) is the larger positive check. Labellers are models, not humans, and they read the classifier's rule, so this shows
  the rule is reproducible by a reader, not that refusal-seen explains why the agent stopped.
- `test_w5g1_report.py` passes.
