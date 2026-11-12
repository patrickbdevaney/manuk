# W5.G1.b: hand labels vs the W5.G1 classifier (GATE RED)

chrome_ver=145.0.7632.116 (`$MANUK_CHROME_PIN`). Gate: agreement >= 9/10 with `causes.tsv`. **Result: 4/10 as drawn. Gate RED.**

## Draw
- Frame: the 41 failed (arm, task, run) rows of `G1/cdp-layer.tsv` and `G1/cdp-layer+pwfmt.tsv` (pass = 0), sorted, then
  `random.Random(20261112).sample(keys, 10)`. Drawn once, not redrawn.
- Labels were written to `G1/hand-labels.tsv` before `causes.tsv` or the classifier's output for these runs was read.
  Read before the labels: the G1 README, `G1.md`, the classifier rules in `bench/agent-tasks/w5g1_report.py`
  (precedence and the `act`/`target` definitions), and the per-run transcripts. Labels use the five classes with the
  definitions in `G1.md`. One class per run.

## Agreement

| arm | task | run | hand | causes.tsv (current classifier on current transcript) | transcript provenance | agree |
|---|---|---|---|---|---|---|
| cdp-layer | gomail-8 | 2 | target | tool-choice | OVERWRITTEN after the G1 run | no |
| cdp-layer+pwfmt | gomail-5 | 2 | target | tool-choice | verified | no |
| cdp-layer+pwfmt | staynb-4 | 3 | target | target | verified | yes |
| cdp-layer+pwfmt | dashdish-7 | 3 | target | target | verified | yes |
| cdp-layer | opendining-10 | 1 | target | act | OVERWRITTEN after the G1 run | no |
| cdp-layer | gomail-3 | 2 | target | act | OVERWRITTEN after the G1 run | no |
| cdp-layer+pwfmt | opendining-10 | 3 | perceive | act | verified | no |
| cdp-layer+pwfmt | fly-unified-6 | 3 | target | target | verified | yes |
| cdp-layer+pwfmt | dashdish-7 | 1 | act | act | verified | yes |
| cdp-layer | gomail-5 | 3 | target | tool-choice | OVERWRITTEN after the G1 run | no |

"Verified" = the transcript's final answer equals the `answer` column of the G1 row (first 80 chars).

- As drawn: **4/10**.
- Verified rows only: **4/6** (2 disagreements, both pwfmt).
- The 4 OVERWRITTEN rows cannot be scored. Their `target/real-runs` transcripts were rewritten after the G1 run (mtimes
  2026-11-07 17:50, 2026-11-08 03:13 and 14:54, 2026-11-12 05:50; `causes.tsv` was written 2026-11-07 02:01). The labels for those four were made on the
  newer transcripts, so they say nothing about the G1 failures.
- The gate is red regardless of how the four OVERWRITTEN rows resolve: the two verified disagreements cap the best
  possible total at 4 + 4 = 8 < 9.

## Disagreements, read as findings (not used to change labels)
- **gomail-5 pwfmt r2** (hand `target`, rule `tool-choice`): the agent made zero action calls. It handed off at once on a
  MODAL OVERLAY with no close control. The rule reads that as tool-choice; the hand label reads the overlay as the
  blocker. Both readings are defensible, so the class boundary is ambiguous here.
- **opendining-10 pwfmt r3** (hand `perceive`, rule `act`): the run completed the booking ("Booking Complete"). Its
  failure is a wrong ranking from the menu survey. The `act` rule fired on earlier, recovered no-effect or occluded
  results. Those were not the failure, so the rule over-triggers on recovered action errors.

## Classifier reproducibility (same root cause)
Re-running `w5g1_report.classify` on the current transcripts reproduces 28 of 41 `causes.tsv` rows. All 13 mismatches
are on transcripts with mtime after `causes.tsv` (13 of 21 newer transcripts differ). None of the 20 older transcripts
differs. So `causes.tsv`, the per-class counts in `G1.md`, and its top-2 class claim are not reproducible from the
transcripts now on disk. The gate only checks coverage (every failed run has a class), which passes, so it did not
catch this.

## Also found
- `ACTS` in `w5g1_report.py` omits `key`, `scroll`, `navigate`, `handoff`, and `drag`. A run whose only attempts were
  key presses scores `tool-choice` even though it acted.

## Follow-ups (graph rows)
- W5.G1.b.a: stop run-name reuse in `target/real-runs` so G1 transcripts stay tied to their rows.
- W5.G1.b.b: fix the `ACTS` set and the `act` over-trigger; re-derive `causes.tsv` only from verified transcripts.
- W5.G1.b.c: a new hand-label draw on the verified frame, after .a and .b, with its seed written into the node before the draw.
