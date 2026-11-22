# W5.G1.b.d (retry): blind check of the mechanical classes on a fresh batch (GATE MET, 10/10)

chrome_ver=145.0.7632.116. Batch `cdp-layer+pwfmt-v6m.tsv`: 30 runs (10 tasks x 3, haiku), pinned binary copy sha256 500ac3a0...3cd3
(target/release/manuk-mcp at 2026-11-22 02:42, MANUK_MCP_BIN). 30/30 tied to transcripts (`w5g1_transcript_check.py --tsv ...v6m.tsv`, CHECK PASS).
15 passed, 15 failed (pass per run 5/5/5).
Seed 20261122 was written to the GRAPH row and committed (ad50b8ac) before the batch ran; `random.seed(20261122); random.sample` of the
15 failed rows sorted by (task, run), 10 drawn.
Labels (`hand-labels-mech-v6m.tsv`, written 03:33:43, sha256 bb9f124b...0882) were written by a separate subagent from transcripts only, forbidden from
reading causes files, the classifier, G1.md and earlier labels, before `causes-pwfmt-v6m.tsv` was generated. Difference from the v5 draw (8/10):
this labeller was given the rule text that W5.G1.b.d.a published (G1.md lines 43-58, 64-68, 73-74) plus a prompt summary of it; the summary was
written by the node agent, who had read the FAIL list in `w5g1_report.py`, so the refusal vocabulary in the prompt mirrors the code.

Result: `w5g1_agree_check.py hand-labels-mech-v6m.tsv --causes causes-pwfmt-v6m.tsv` = **10/10, rc 0.**
Drawn labels: 9 stop-by-choice, 1 grader-issue (gomail-8 r2, [1, 1, 0]), 0 refusal-seen.
Mutants: dashdish-7 r1 + staynb-4 r2 set to refusal-seen (the reading the labeller called "the spirit of the rule") -> 8/10 rc 1;
labels file cut to 8 rows -> rc 1. One flipped row -> 9/10 rc 0 (threshold is 9).

Limits (what this does NOT show):
- No drawn row is classed refusal-seen; the classifier has 2 refusal-seen rows in this batch, neither drawn. So agreement on refusal-seen
  positives is untested. The boundary was exercised only by two near-misses: dashdish-7 r1 (one `UNREACHABLE ... no clickable area` after the
  last success) and staynb-4 r2 (one `no change | url: unchanged` after the last success), both stop-by-choice by the stated rule and both flagged
  ambiguous by the labeller.
- It shows a reader given the stated rule reproduces the classifier, not that the classes say why the agent stopped.
