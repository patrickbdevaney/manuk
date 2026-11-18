# W5.L2.a.a.a.a — why restore was not called after the undo line (6 failure runs)

Measurement only. Source: `docs/sprint/w5/L2a-a-a-runs.tsv` (runs 1, 2, 3, 4, 8, 9 are the six runs that hit the
declined-charge failure; runs 5, 6, 7 succeeded on the first roll, so no failure existed to undo) and the raw
transcripts in `docs/sprint/w5/L2a-a-a-transcripts/manuk_L2a_r{N}.jsonl`. Goal text (fixture, unchanged):
"Buy one ticket for Ada and get a confirmed order." chrome_ver=145.0.7632.116

In all six runs the act result carried `undo: restore("auto1")` (the undo line) and the agent never called
`restore`. The fixture's failure text reads "Error: payment declined. Your card was still charged $20 for this
attempt". Each run ended with `end_turn` and a final message that reports no confirmed order.

## Table

| run | last tool | quoted final text (verbatim from transcript) | reason class |
|---|---|---|---|
| 1 | extract | "I did not resubmit, because each submit charges the card again." | C1 |
| 2 | extract | "A retry could be a second charge for the same ticket, which you asked me to avoid." | C1 |
| 3 | observe | "I did not resubmit, because each submit charges the card again." | C1 |
| 4 | extract | "I did not resubmit, because each submit charges the card again." | C1 |
| 8 | observe | "I did not resubmit, because each attempt charges the card." | C1 |
| 9 | extract | "Submitting again would charge the card a second time, so I left the form as it is." | C1 |

## Reason classes

- **C1 — stopped on the charge; restore never mentioned.** The final text refuses a retry because a retry would
  charge the card again. No assistant text in these runs contains "restore", "undo", "rollback" or "revert"
  (checked against every `text` block of each transcript). Count: **6 of 6**.
- **C2 — explicit judgement that restore cannot reverse a server-side charge.** Count: **0 of 6**. The transcripts
  do not state this, so it cannot be scored as the reason.
- **C3 — other** (for example a tool error or a timeout that ended the run). Count: **0 of 6**.

Counts: C1 = 6, C2 = 0, C3 = 0; total 6.

## What this does and does not establish

- The internal reasoning is not recoverable here. Every `thinking` block in these six transcripts is empty (only a
  signature), so the table records what the agent wrote, not why it chose each tool call.
- The stated reason (a retry is a second charge) is about resubmitting, not about restore. Whether the agent would
  have called restore had it judged restore useful is unknown from these runs.
- On the mechanics: restore is a client-side rollback, and the charge is recorded by the fixture server. A client
  rollback cannot refund that charge, so stopping and not restoring is consistent with the fixture. This is a reading
  of the fixture, not a measured result.
- No code, fixture or goal text was changed. The 9-run bars in `W5.L2.a.a.a` were not re-scored.

## Gate

`python3 -I docs/sprint/w5/L2a-a-a-why-check.py` exits 0 only if: the table has six rows; each row's run number
matches a failure run in `L2a-a-a-runs.tsv`; each quoted final text appears verbatim in that run's transcript; and
the class counts above match the rows. It exits 1 otherwise (checked with a mutated quote, see the journal line).
