# RG.S1.a.e.a: networkin-3 diagnosis (chrome_ver=145.0.7632.116, judge openai/gpt-oss-120b)

Task text asks for a post with "purpose, date, and target audience" but gives no date. Evals are deterministic jmespath: post added; text contains 'event'/'Event'; text contains 'network'/'Network'.

| run | source | outcome | cause class |
|---|---|---|---|
| S1ae r1 | S1ae-real-manuk.tsv | draft left unposted, answer asks for a date | agent policy: refuses to publish placeholders `[insert date]`; task ambiguity (no date given) is the trigger |
| S1ae r3 | same | draft left unposted, asks for date/time | same as r1 |
| S1ae r2 | same | posted with invented details, 3/3 | none |
| S1aea r1 | S1aea-real-manuk.tsv | posted, 3/3 PASS | none |
| S1aea r2 | same | draft unposted: "publishing is outward-facing" | agent policy (publish hesitation) |
| S1aea r3 | same | posted, 2/3 FAIL | wording miss: stored text lacks literal 'event' |

Engine: none found. Every run that clicked Post had its text reach feed state (state_ok=1 in all 6 rows; the add-post eval passes whenever Post is clicked). The unposted runs filled the composer and stopped before a working Post button; no refusal, STALE or OCCLUDED outcome appears in any answer.
Class per failing run: ambiguity (missing date) + agent policy for S1ae r1, r3; agent policy for S1aea r2; grader wording for S1aea r3. No engine row opened; task file unedited, bar unchanged.
Re-run at HEAD 1b269f66: 1 of 3 pass (r1). Does not meet pass>=2; consistent with the 1/3..3/3 band in S1c-classify.md.
