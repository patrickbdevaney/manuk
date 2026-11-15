# W5.G1.a.a.a — gomail-3 on cdp-layer: why runs 1 and 3 stopped before Send

chrome_ver=145.0.7632.116

Runs: `target/real-runs/cdp-layer_gomail-3_r1/transcript.jsonl` and `target/real-runs/cdp-layer_gomail-3_r3/transcript.jsonl` (cdp-layer, model haiku, real-v1-det). Rows in `docs/sprint/w5/G1aa-cdp-layer.tsv`: r1 `pass=0`, r3 `pass=1`.

## The task text

The goal, from `bench/agent-tasks/real/tasks/gomail-3.json`:

> Compose a new email to jonathan.smith@example.com with the subject "Meeting Notes" and body "Please find the meeting notes attached."

The goal is wrapped by `bench/agent-tasks/run_arm.py` `PROMPT` ("Complete the task below, then reply with a short final answer…"). Neither the wrapper nor the goal says "do not send", and neither mentions sending at all. The prompt itself is not written into `transcript.jsonl`, so the task quote is checked against the task file, not the transcript.

The grader (`evals` in the same JSON) checks `differences.emails.added[0]`: `to`, `subject` and `content`. A sent message is what registers as `added`. An unsent draft does not.

## What each run did at the decision point

Both runs filled To, Subject and body, and both were offered the same control: `[h162] button "Send" in dialog "New Message"` (seen in the observations of both transcripts).

- **r1 (fail).** Its last message, after the body was typed and a screenshot taken, says it did not send:

  > I left it unsent because the task said to compose, not send.

  No tool result in r1 mentions sending or a confirmation step. The reason it gives is the sentence above; the transcript never says "irreversible" (that word appears only in the G1aa-cdp-layer.tsv answer column, which is a summary of the run). The `state.json` for r1 has `differences.emails.added = []`.

- **r3 (pass).** At the same point it wrote:

  > I'll send the message, since the task is to compose the email to that recipient.

  It clicked `h162` (Send). The Sent folder then listed "Meeting Notes" to Jonathan Smith, and `state.json` has the added email with the expected `to`, `subject` and `content`.

## Verdict

The stop is the agent's own decision not to send, on a task that does not forbid sending. It is not the task wording and not a tool or receipt signal:

1. **Task wording:** no. The goal says "compose a new email" and gives the recipient, subject and body. The phrase "compose, not send" is the agent's reading of "compose", not text from the task. The same wording led r3 to send.
2. **Agent caution:** yes. r1 chose not to send and stated that as a reading of "compose". Nothing in the task or the tool output asked for a draft.
3. **Tool or receipt signal:** no. Send was offered as a normal button, nothing in the observations warned against it, and no tool result blocked it.

So this is run-to-run variance in the agent's reading of an under-specified goal, which the G1 slice counts as a fail. It is not a cdp-layer tool defect at this point. Changing the task wording or the grader would be tuning the eval, which SUCCESS.md forbids, so none is proposed here. A follow-up can measure whether the same stop happens on other "compose" tasks under the same pin.

## Quotes checked by `G1aa_stop_check.py`

The block below is machine-read. Format: `<source>` TAB `<text>`. `task` is checked against the goal in `gomail-3.json`; `r1` and `r3` are checked verbatim against the decoded string values in the matching `transcript.jsonl`.

```quotes
task	Compose a new email to jonathan.smith@example.com with the subject "Meeting Notes" and body "Please find the meeting notes attached."
r1	I left it unsent because the task said to compose, not send.
r3	I'll send the message, since the task is to compose the email to that recipient.
r3	I read "compose" as compose and send, so I clicked Send.
```
