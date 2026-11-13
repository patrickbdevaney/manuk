# RG.2.0.b.a.a.a.a.a: gocalendar-2 closing-state re-judge (chrome_ver=145.0.7632.116)

Question: is the failing clause the closing Day-view sentence, as RG.2.0.b.a.a.a.a inferred?
Script: `docs/sprint/rg/R20baaaaa-rejudge.py` (judge sees rubric + answer + the run's saved final state, nothing else).
Inputs: the 3 answers in `R20baaa-real-manuk.tsv`; final state from `target/real-runs/manuk_gocalendar-2_r{1,2,3}/state.json`
(all three are an empty eventsDiff, so the state is identical across runs).

## Judge status
- Primary judge (`openai/gpt-oss-120b`): HTTP 429 with Retry-After 307 s on the 33-call batch; no verdict in ~40 min. Not usable this invocation.
- Fallback (`claude -p` Haiku, the harness's documented W5.J1 path, `judge_model` column = `claude-haiku (claude -p)`):
  used for every number below. It is noisy on this rubric: the **positive control** (a hand-written answer naming exactly the 15 rubric events, no view claims, no extras) passed 2 of 3, and the negative control passed 0 of 3.

## Verdicts (fallback judge, 3 reps per cell; file `R20baaaaa-rejudge-haiku.tsv`)

| answer | F full | S view sentences stripped | SI S + inference disclaimers stripped |
|---|---|---|---|
| run1 | 1/3 | 3/3 | 2/3 |
| run2 | 1/3 | 1/3 | 2/3 |
| run3 | 0/3 | 0/3 | 0/3 |
| POS control | 2/3 | | |
| NEG control (3 events dropped) | 0/3 | | |

Stripped text: `R20baaaaa-stripped-haiku.txt`. What was removed: sentences after the last rubric event (Client Presentation) that say view, page or switch; for SI also the "inferred / did not open" sentences.

## What the failing reasons name
In every F rep that fails, the reason is one of:
1. **Extra events:** the answer lists 22 events, including 7 not in the rubric (Coffee with Mentor, Dinner and Movie, Creative Session, Follow-up Meeting, Dinner with Friends, Reading Time, Workout Session). The judge reads "accurately list all events ... including [15]" as an exact set.
2. **Name variant:** "Team Collaboration Workshop" vs the rubric's "Team Collaboration Work".
3. **Ungroundable state:** the judge says the listed events are "not verifiable from the state, which shows no calendar data". The saved state is an empty diff, so this is a judge-prompt effect, not an answer effect.

No failing reason mentions the Month/Day view. The rubric has no view clause. The judge is never shown the goal, so the view can only affect the verdict through the answer text.

## Cross-arm check (existing recorded rows, no new run)
All 7 "extra" events appear in every recorded gocalendar-2 answer, for manuk (B, B2, R20b, R20ba, R20baaa) and for the Chrome arm (`B-real-playwright.tsv`, `B2-real-playwright.tsv`). So the extras are not manuk-specific. The one primary-judge pass (`B2-real-manuk.tsv` run 2) also lists the 7 extras, and its answer opens with "Month view is on" and has no Day-view claim. That is consistent with the view hypothesis, but it is one sample.

## Conclusion
- **Not confirmed:** the closing-state hypothesis. Stripping the view sentences moves run1 from 1/3 to 3/3 and leaves run2 and run3 flat or within noise. The fallback judge cannot resolve that difference: its positive control fails once in three.
- **Named instead:** the judge's exact-set reading of the rubric against the 22-event answer, plus a name-variant clause and an ungroundable-state clause. The rubric and task are frozen (SUCCESS.md: no tuning after results), so any change to them is an owner decision and restarts the comparison.
- **Correction to RG.2.0.b.a.a.a.a:** it said all three failing manuk runs end in Day view. Run 3 does not. It ends "I did not open each of those days individually to confirm the rest" and has no closing view claim.
- **Chrome-arm x1 not re-run:** the judge was unusable and the Chrome-arm answers already in the tree show the same extras.

## Gate
`R20baaaaa-rejudge.py` exits 0 only if the stripper changes every answer and the primary judge passes POS and fails NEG on every rep. This run: the stripper check passed; the POS control failed (2/3), so the gate is red on the fallback judge. The red branch is shown by `--mutate nostrip` (`R20baaaaa-rejudge-haiku-mutant-nostrip.tsv`): the gate prints `stripper changed all 3 answers=False`.

A first fallback pass had a join bug that dropped run 3's "Monday list" sentence and the Client Presentation time from S and SI. It was fixed before the table above was produced, and no numbers from it are used.

Follow-up nodes: `RG.2.0.b.a.a.a.a.a.a` (re-run on the primary judge when it returns), `RG.2.0.b.a.a.a.a.a.b` (owner decision on the rubric's set reading).
