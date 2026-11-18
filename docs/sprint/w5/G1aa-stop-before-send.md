# W5.G1.a.a.a — gomail-3 on cdp-layer: why runs 1 and 3 stopped before Send

chrome_ver=145.0.7632.116

## Which run directories this doc quotes

The live directories `target/real-runs/cdp-layer_gomail-3_r<N>/` are overwritten by later reruns, so this
doc names timestamped archives only. Every run and pass claim is in the `claims` block below and is checked
by `G1aa-doc-provenance.py` against its archive's `state.json`, and against the TSV row where one exists.

- The three rows of `docs/sprint/w5/G1aa-cdp-layer.tsv` (the original G1aa measurement, run 20:58 to 21:00
  on 2026-11-14) are archived as `cdp-layer_gomail-3_r1.20261114T205846`, `..._r2.20261114T205927` and
  `..._r3.20261114T210000`. Row 1 is a draft, row 2 is the sent run that passed, row 3 is a draft.
- Later runs of the same task, from the same pin, are in `cdp-layer_gomail-3_r1.20261114T211043`,
  `..._r1.20261114T231413` (draft), `..._r3.20261114T213951` (sent) and `..._r3.20261114T234243` (sent).
  They are not TSV rows, so their outcome is read from the archive state only.

## The task text

The goal, from `bench/agent-tasks/real/tasks/gomail-3.json`:

> Compose a new email to jonathan.smith@example.com with the subject "Meeting Notes" and body "Please find the meeting notes attached."

The goal is wrapped by `bench/agent-tasks/run_arm.py` `PROMPT` ("Complete the task below, then reply with a short final answer…"). Neither the wrapper nor the goal says "do not send", and neither mentions sending at all. The prompt itself is not written into `transcript.jsonl`, so the task quote is checked against the task file, not the transcript.

The grader checks `differences.emails.added[0]`: `to`, `subject` and `content`. A sent message is what registers as `added`. An unsent draft does not.

## What the stopped run and the sent run did at the decision point

Both runs filled To, Subject and body, and both were offered the same control: `[h162] button "Send" in dialog "New Message"`.

- **Stopped run (archive `cdp-layer_gomail-3_r1.20261114T231413`, no email added).** Its last message, after the body was typed and a screenshot taken, says it did not send:

  > I left it unsent because the task said to compose, not send.

  No tool result in that transcript mentions sending or a confirmation step. The reason it gives is the sentence above.

- **Sent run (archive `cdp-layer_gomail-3_r3.20261114T234243`, one email added).** At the same point it wrote:

  > I'll send the message, since the task is to compose the email to that recipient.

  It then clicked `h162` (Send). Its `state.json` has the added email with the expected `to`, `subject` and `content`.

## Verdict

The stop is the agent's own decision not to send, on a task that does not forbid sending. It is not the task wording and not a tool or receipt signal:

1. **Task wording:** no. The goal says "compose a new email" and gives the recipient, subject and body. The phrase "compose, not send" is the agent's reading of "compose", not text from the task. The same wording led the sent run to send.
2. **Agent caution:** yes. The stopped run chose not to send and stated that as a reading of "compose". Nothing in the task or the tool output asked for a draft.
3. **Tool or receipt signal:** no. Send was offered as a normal button, nothing in the observations warned against it, and no tool result blocked it.

So this is run-to-run variance in the agent's reading of an under-specified goal, which the G1 slice counts as a fail. It is not a cdp-layer tool defect at this point. Changing the task wording or the grader would be tuning the eval, which SUCCESS.md forbids, so none is proposed here. The measured split is in `G1aa2-send-rate.txt`.

## Claims (checked by `G1aa-doc-provenance.py`)

Format: `<run N or ->` TAB `<archive dir>` TAB `<outcome as 0 or 1>`. A run number other than `-` also requires a matching TSV row with the same outcome and timestamp.

```claims
1	cdp-layer_gomail-3_r1.20261114T205846	pass=0
2	cdp-layer_gomail-3_r2.20261114T205927	pass=1
3	cdp-layer_gomail-3_r3.20261114T210000	pass=0
-	cdp-layer_gomail-3_r1.20261114T231413	pass=0
-	cdp-layer_gomail-3_r1.20261114T211043	pass=1
-	cdp-layer_gomail-3_r3.20261114T213951	pass=1
-	cdp-layer_gomail-3_r3.20261114T234243	pass=1
```

## Quotes (checked by `G1aa-doc-provenance.py`)

Format: `<archive dir or task>` TAB `<text>`. `task` is checked against the goal in `gomail-3.json`; archive quotes are checked verbatim against the decoded string values in that archive's `transcript.jsonl`.

```quotes
task	Compose a new email to jonathan.smith@example.com with the subject "Meeting Notes" and body "Please find the meeting notes attached."
cdp-layer_gomail-3_r1.20261114T231413	I left it unsent because the task said to compose, not send.
cdp-layer_gomail-3_r3.20261114T234243	I'll send the message, since the task is to compose the email to that recipient.
cdp-layer_gomail-3_r3.20261114T234243	I read "compose" as compose and send, so I clicked Send.
```
