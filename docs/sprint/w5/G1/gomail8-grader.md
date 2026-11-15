# gomail-8 grader check (W5.G1.b.b.a.a.a.a)

Question: why are cdp-layer v2 gomail-8 r1 and r2 failed rows, when the transcripts show four GitHub emails
moved to Trash?

Source: `docs/sprint/w5/G1/cdp-layer-v2.tsv` (chrome_ver=145.0.7632.116), task `bench/agent-tasks/real/tasks/gomail-8.json`,
archived grader state `target/real-runs/cdp-layer_gomail-8_r<run>.<ts>/state.json`. Reproduce with:

    python3 -I bench/agent-tasks/w5g1_gomail8_grader.py

## Task checkpoints (frozen task JSON, read as-is)

| index | description | expected |
|---|---|---|
| 0 | specific GitHub email (id 3) moved to trash | `differences.emails.updated[0].id == "3"` |
| 1 | email is in trash | `differences.emails.updated[0].trash == true` |
| 2 | exactly one email updated | `length(differences.emails.updated) == 1` |

The goal text is "Clear all emails from \"GitHub\" in the inbox to trash."

## Per-row result

| row | pass | evals_vec | failed checkpoint | archived state: updated emails (id, trash) | class |
|---|---|---|---|---|---|
| cdp-layer gomail-8 r1 (2026-11-14T21:13:57) | 0 | [1, 1, 0] | 2 (exactly one email updated) | 4: (3, true), (16, true), (37, true), (50, true) | grader/task contradicts the goal |
| cdp-layer gomail-8 r2 (2026-11-14T21:28:02) | 0 | [1, 1, 0] | 2 (exactly one email updated) | 4: (3, true), (16, true), (37, true), (50, true) | grader/task contradicts the goal |
| cdp-layer gomail-8 r3 (2026-11-14T21:43:16) | 0 | [1, 1, 0] | 2 (exactly one email updated) | 4: (3, true), (16, true), (37, true), (50, true) | grader/task contradicts the goal |

r3 is not named in the node, but it has the same failure and is listed for completeness.

## Finding

- The agent did what the goal asks. Checkpoints 0 and 1 pass in all three runs, and the archived state shows four
  emails moved to Trash, all with id and trash as checked. The agent's own answers say the four were the GitHub
  emails (search "GitHub" returned four results, each sender at github.com). That claim is from the transcripts,
  not checked against the seed data.
- Checkpoint 2 requires exactly one updated email. The goal asks for all of them. For a correct run the check
  cannot pass. The same checkpoint is the only one failed in every run, so this is a task/grader issue, not an agent
  error.
- Earlier notes already said the same: `docs/sprint/JOURNAL.md` W4.P0.e ("the gomail-8 false-pass finding is
  withdrawn (task evals require exactly one email)") and `bench/agent-tasks/engine_compass.py` line 50, which records
  "manuk trashed 1 of 4 GitHub mails and passes 3/3".
- Consequence for G1: `causes-v2.tsv` counts these three rows as `perceive`, which is the fallback class. Under the
  (a) counts, 3 of the 14 perceive rows are grader failures. This is not an (a) cause claim.

## Not done here

- The task and SUCCESS.md are unchanged (their thresholds are frozen; the task is not edited to fit the result).
- Reclassifying these rows in `causes-v2.tsv` is a follow-up node, not part of this one.
