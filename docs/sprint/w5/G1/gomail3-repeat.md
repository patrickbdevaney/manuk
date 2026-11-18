# W5.G1.b.a.a.a.a.a: is the gomail-3 gap (pre-77b97d40 vs HEAD) reproducible? (no, RED on the stated clause)

chrome_ver=145.0.7632.116 (all rows). Suite real-v1-det, Haiku, `cdp-layer` arm, task gomail-3 only,
`real_run.py run --arm cdp-layer --runs 3 --tasks gomail-3 --jobs 1 --timeout 420`, run back to back on 2026-11-18.

## Setup

- Pre-77 side: `MANUK_MCP_BIN=/home/patrickd/manuk-pre77/target/release/manuk-mcp` (the `0d02a367` build; sha256 `f978c551…1cad`,
  0 marker strings). Rows in `cdp-layer-gomail3-repeat-pre77.tsv`.
- HEAD side: `MANUK_MCP_BIN=/home/patrickd/manuk/target/release/manuk-mcp`, rebuilt 2026-11-18 05:58 from the engine tree of
  `a45160e5` (no `engine/`, `crates/` or `tests/` change since, checked with `git diff --stat`). sha256 `82900822…aa0`, 1 marker
  string. Rows in `cdp-layer-gomail3-repeat-head.tsv`. Build identity: `pre77-build-repeat.tsv`.
- Transcripts: `~/.cache/manuk-real/transcripts/cdp-layer_gomail-3_r{1,2,3}.jsonl`; the prior transcripts were archived by the
  run's own guard, not overwritten. `w5g1_transcript_check.py --tsv` ties 3/3 rows in each set (CHECK PASS).

## Result

| set | build | runs | passes | run seconds | turns |
|---|---|---|---|---|---|
| pre-77, 2026-11-15 (`cdp-layer-pre77.tsv`) | pre-77 | 3 | 0/3 | (see TSV) | (see TSV) |
| pre-77 repeat, 2026-11-18 | pre-77 | 3 | 0/3 | 96.5, 174.7, 91.3 | 31, 83, 39 |
| HEAD v2, 2026-11-14 (`cdp-layer-v2.tsv`) | 20:57 binary, unhashed | 3 | 3/3 | (see TSV) | (see TSV) |
| HEAD v3, 2026-11-14 (`cdp-layer-v3.tsv`) | same as v2 | 3 | 2/3 | (see TSV) | (see TSV) |
| HEAD repeat, 2026-11-18 | 05:58 binary, `82900822…` | 3 | 0/3 | 34.7, 34.4, 35.1 | 13, 14, 12 |

Pooled over sets: pre-77 0/6; HEAD 5/9 (3 + 2 + 0). The HEAD cell moved 3/3 → 2/3 → 0/3 across builds and days, so the cell
is not stable at one tree.

## Verdict against the pre-registered clause

- The gate `w5g1_v3_compare.py --dir docs/sprint/w5/G1 --v2 cdp-layer-gomail3-repeat-pre77.tsv --v3 cdp-layer-gomail3-repeat-head.tsv
  --band-pair cdp-layer-v2.tsv,cdp-layer-v3.tsv --builds docs/sprint/w5/G1/pre77-build-repeat.tsv` exits 0 (3 rows per set, same pin,
  distinct run numbers, builds differ, marker in the right binary only).
- The node's clause "RED if the per-task gap in the repeat is below 2 runs" is **met**: the repeat gap is 0 - 0 = 0 runs. The
  gomail-3 pre-77 vs HEAD difference (0/3 vs 3/3 in v2, 0/3 vs 2/3 in v3) is **not reproduced** by the repeat. Reading: the
  earlier gap is inside the run-to-run noise of this cell, and there is no measured effect of 77b97d40 on gomail-3.
- Note: the compare script does not itself implement the "gap below 2 runs" rule; the verdict above is read by hand from the table.
  Its exit 0 covers structure (row counts, pin, shas, markers), not the effect.

## Why the cell fails (from the final answers and the grader)

- gomail-3's grader requires `differences.emails.added[0]` with the exact body `<p>Please find the meeting notes attached.</p>`.
  The goal says "Compose". HEAD (repeat): all 3 runs wrote the body and stopped before Send (their answers say so); most likely that is why the
  added-email check did not hold. Pre-77 (repeat): the runs reported failing to get the body text in (`body empty`, `the key tool
  dropped the period`, `body editor not exposed`).
- So the two builds fail the cell at different steps in the repeat (inferred from the answers and the grader's expected
  `added[0]` check, not from a transcript-level trace). That is a difference in where the agent stops, not a measured
  pass/fail effect of the tree. No task or grader was changed (SUCCESS.md: no tuning after results).

## Checks

- Mutants (in a temp dir, each must go RED for its own reason): dropped repeat row → `gomail-3: v3 has 2 rows, need 3`; pin
  mismatch → `chrome_ver differs`; equal pre-77 and HEAD shas → `sha equals HEAD's`; marker string in the pre-77 build →
  `carries the 77b97d40 marker`; duplicate run number → `v2 repeats a run number`. All five exit 1. Real files: exit 0.

## What this does not settle

- Whether HEAD's gomail-3 is itself a regression (3/3 → 0/3 at HEAD across builds) or noise: three runs per set is too few.
  Follow-up row W5.G1.b.a.a.a.a.a.a measures it with more runs.
- The 2026-11-07 set (9/30) and the tree effect of 77b97d40 on the 10-task total: unchanged by this node.
