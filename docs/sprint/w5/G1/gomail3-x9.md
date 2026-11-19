# W5.G1.b.a.a.a.a.a.b: gomail-3, nine more runs per tree (pre-77b97d40 vs HEAD)

chrome_ver=145.0.7632.116 (all rows). Suite real-v1-det, Haiku, `cdp-layer` arm, task gomail-3 only, one run per
`real_run.py run --arm cdp-layer --runs 1 --run-base <r-1> --tasks gomail-3 --jobs 1 --timeout 420` invocation, alternating
pre-77 and HEAD per run number (run 1 pre-77, run 1 HEAD, run 2 pre-77, ...), 2026-11-19 06:27 to 06:53 EST. Driver script
kept outside the repo (`/tmp/x9-run.sh`).

## Builds

- pre-77: `/home/patrickd/manuk-pre77/target/release/manuk-mcp`, sha256 `f978c551…1cad`, marker `drop-form-containers` count 0.
  Same binary as in `pre77-build.tsv` and `pre77-build-repeat.tsv`.
- HEAD: **not the repeat binary.** The repeat's HEAD binary (`82900822…`) was overwritten and cannot be re-hashed. This node
  rebuilt the same source tree (`a45160e5`) in a worktree at `/home/patrickd/manuk-a45160` (`MOZJS_FROM_SOURCE=1`, release).
  The result is sha256 `d28e8369…c4c1`, marker count 1. The bytes differ from the repeat's HEAD binary, so this HEAD side is
  the same source tree but not the same binary. Build identity: `pre77-build-x9.tsv`. The original `pre77-build-repeat.tsv`
  still records the lost `82900822` binary.
- The working tree's current `target/release/manuk-mcp` (HEAD `81ebe129`, 49 commits and 191 files past `a45160e5`) was
  not used. Measuring it would have been a different tree.

## Results (gomail-3, 9 runs per side)

| run | pre-77 pass | pre-77 secs | HEAD pass | HEAD secs |
|---|---|---|---|---|
| 1 | 0/3 | 123.3 | 0/3 | 34.0 |
| 2 | 0/3 | 75.9 | **3/3** | 42.0 |
| 3 | 0/3 | 95.7 | **3/3** | 47.0 |
| 4 | 0/3 | 96.8 | 0/3 | 40.3 |
| 5 | 0/3 | 279.2 | **3/3** | 38.1 |
| 6 | 0/3 | 182.3 | 0/3 | 36.7 |
| 7 | 0/3 | 90.3 | 0/3 | 36.9 |
| 8 | 0/3 | 97.7 | 0/3 | 30.8 |
| 9 | 0/3 | 103.9 | 0/3 | 34.4 |
| **x9 total** | **0/9** | median 97.7 s | **3/9** | median 36.9 s |

Each row is a single run with `evals_n` 3. Pass means all three checks held. Rows are in
`cdp-layer-gomail3-x9-pre77.tsv` and `cdp-layer-gomail3-x9-head.tsv`. All 18 rows are tied to their transcripts
(`w5g1_transcript_check.py`: 18/18, CHECK PASS).

## Gate (pre-registered)

`w5g1_v3_compare.py --dir docs/sprint/w5/G1 --v2 cdp-layer-gomail3-x9-pre77.tsv --v3 cdp-layer-gomail3-x9-head.tsv --runs-per-task 9
--builds docs/sprint/w5/G1/pre77-build-x9.tsv --min-gap 3 --gap-task gomail-3` → **exit 0**. Gap gomail-3 = |0 − 3| = 3 runs (min 3).
Structure: 9 rows per set, one pin, distinct run numbers, two different binaries, marker only in HEAD.

Mutants, each RED for its own reason (all exit 1; the real files exit 0):
- dropped row → `gomail-3: v3 has 8 rows, need 9`
- pin mismatch → `chrome_ver differs`
- equal shas → `pre-77 manuk-mcp sha equals HEAD's`
- `--min-gap 4` (gap is 3) → `per-task gap 3 is below --min-gap 4`
- transcript answer tampered → `answer differs` (checker exit 1)

## Pooled reading (the node's stated verdict basis)

The node says the gap verdict uses all four gomail-3 sets. `w5g1_v3_compare.py` does not pool; it reads only the two
files it is given. The pooling below was done by hand from the TSVs.

| side | sets | passes |
|---|---|---|
| pre-77 | 0/3 (`cdp-layer-pre77.tsv`), 0/3 (repeat), 0/9 (x9) | **0/15** |
| HEAD | 3/3 (v2, unhashed build), 2/3 (v3, unhashed build), 0/3 (repeat, `82900822`), 3/9 (x9, `d28e8369`) | **8/18** |

One-sided Fisher exact on the pooled counts: p = 0.003. On the x9 sets alone: 0/9 vs 3/9, p = 0.10.

Caveats that limit the pooled number:
- The HEAD pool mixes four binaries. Two of them (v2/v3) have no hash on record, and the repeat binary is gone. The HEAD cell
  moved 3/3 → 2/3 → 0/3 → 3/9 across builds and days. Pooling is only as good as the claim that these are one tree.
- The pre-registered gap clause (min-gap 3 on x9) is met exactly at its threshold, and it is not significant alone.

## What the failures look like (from the answers, not from a trace)

- pre-77: 8 of 9 answers say the body text would not go in ("couldn't enter the body", "body editor not exposed", "body still
  empty"). The email never reaches Send.
- HEAD: 0 of 9 answers report body-entry trouble. The 3 passing runs sent the email. Of the 6 failing HEAD runs, 5 say in
  their answer that the email was not sent (composed, then stopped before Send); the 6th does not say either way.
- The grader (`bench/agent-tasks/real/tasks/gomail-3.json`) checks `differences.emails.added[0]`, which only exists after a
  send. The goal says "Compose a new email to …". So a HEAD run that composes and stops fails the grader while doing what
  the words say. That is the same kind of goal/grader mismatch as gomail-8 (W5.G1.b.b.a.a.a.a). It is a finding, not a fix.
  SUCCESS.md forbids tuning the task after results, so the task is unchanged here.

Reading: the pre-77 → HEAD difference on gomail-3 is most plausibly the body-entry step that 77b97d40 changed (pre-77 cannot
enter the body in 8 of 9 runs; HEAD can). Whether HEAD's extra passes count depends on the send-vs-compose mismatch above.
That is a measured difference in where the agent stops, not a clean pass/fail effect of the tree.

## Not settled

- Whether HEAD's gomail-3 rate is stable. The x9 HEAD set (3/9) is near the pre-registered noise band of the earlier sets.
- The exact binary identity of the pooled HEAD sets (v2/v3 have no hash).
- Whether the grader or the goal should change. That is an owner decision (follow-up row).
- The compare script prints "every task has 3 rows" and "Same 10 tasks x 3 runs" with a 9-run gate. The check itself uses
  `--runs-per-task`, so the verdict is right; only the message text is stale. Not fixed here.
