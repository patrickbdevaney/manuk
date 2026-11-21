# RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.c — is chrome's ordinal #13 miss a replay regression?

chrome_ver=145.0.7632.116, 2026-11-21. Gate: `bench/agent-tasks/rg3baaaaaaaaaaaaaaaac_verdict.py` (raw output: `RG3baaaaaaaaaaaaaaaac-verdict.txt`).

## Verdict: not a replay regression. The transcript is stale at #13; the match rule scores a correct resolution as a miss.

The explanation holds, so the gate is green. The node's red condition (chrome #13 live line not `[expanded]` in any run) did not fire.

### Evidence

**1. The transcript recorded #13 before the dropdown opened.** Replay step #7 is the 0-based index of the flattened replay, which is the click on `Select time` (`act h7 click`). In the recorded transcript (`~/.cache/manuk-rgb2/transcripts/manuk_opendining-4_r1.jsonl`, raw step 9) that click reported:

> `no change ... dispatched click (no built-in activation behaviour) ... NO EFFECT (step 5): 0 click listeners`

The dropdown never opened, so the time combobox stayed collapsed. Ordinal #13 (`act h8 select "4:30 PM"`, a nameless combobox, ordinal 1) was recorded against `[h8] combobox [collapsed]`.

**2. On Chrome, step #7 now opens the dropdown, and #13 resolves to the expanded combobox.** Three runs at the pinned chrome_ver:

| run | chrome #13 transcript line | chrome #13 live line | chrome step #7 result (excerpt) |
|---|---|---|---|
| run1 | `[h8] combobox [collapsed]` | `[h5] combobox [expanded]` | `ok: dispatched click at. +35 ~1 -0 ...` `option "7:00 AM"` |
| run2 | `[h8] combobox [collapsed]` | `[h5] combobox [expanded]` | same |
| run3 | `[h8] combobox [collapsed]` | `[h5] combobox [expanded]` | same |

The `~1` in step #7 is the single changed line: the combobox's state flipping from collapsed to expanded. The new options are the 7:00 AM onward list. Step #7 is resolved and not occluded in all three runs (`docs/sprint/rg/RG3baaaaaaaaaaaaaa/step7-run{1,2,3}.json`). Before the pointer:fine fix it was OCCLUDED (commit `408e4efe`).

**3. The #13 miss is therefore a state change, not a different element.** The live `[expanded]` combobox is the one the replay then selects `4:30 PM` on. Chrome's #14 (`[h7] combobox [collapsed]`, transcript `[h10] combobox [collapsed]`) matches in all three runs.

### Counts (unchanged, no bar edited)

- chrome 11/13 match: the two chrome misses are opendining-4 #13 (explained above) and gocalendar-2 #6 (`button [clickable] ↑offscreen` in the transcript vs a named calendar cell live). #6 is not covered by this verdict. On manuk, #6 also misses, and #13/#14 are unresolved (the 136 removed nodes; tracked by RG...a.a.a.a.a.a.a.a.b, not this node).
- `rg3baaaaaaa_spread.py --dir docs/sprint/rg/RG3baaaaaaaaaaaaaa` remains **GATE FAIL**: `run1..3: chrome 11/13 < 12`. MIN_CHROME=12 is pre-registered and was not edited. The red bar is recorded as is.
- Consequence to note: if the #13 miss were scored as a match, chrome would be 12/13 and the spread bar would pass. That is the reason to treat the match rule's stale reference as the open issue (see follow-ups), not to change the threshold or the rule here.

### Mutation proof

- Unmutated gate: exit 0 (`GATE PASS`).
- `--mutate collapsed` (in-memory live line set to `combobox [collapsed]`): exit 1, all three runs reported.
- Transcript path pointed at `/dev/null`: exit 1.

## Follow-ups (rows added to docs/sprint/GRAPH.tsv)

- `.a` decide and pre-register how the match rule treats a transcript recorded before a state-changing step; no threshold edit.
- `.b` explain gocalendar-2 #6 on both backends (offscreen clickable transcript node vs a named live cell).
