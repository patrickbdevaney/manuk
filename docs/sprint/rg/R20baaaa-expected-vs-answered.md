# RG.2.0.b.a.a.a.a: gocalendar-2 expected vs answered (chrome_ver=145.0.7632.116)

Source: docs/sprint/rg/R20baaa-real-manuk.tsv (3 manuk runs, all judged 0), compared with passing runs B2-real-manuk.tsv / B2-judge-fallback-dropped-manuk.tsv (manuk, 1) and failing B-real-playwright.tsv / B2-real-playwright.tsv (Chrome arm, 0).
Eval: one llm_boolean rubric naming 15 events (3 per day Mon-Fri: first three of each day).

| day | rubric names | all 3 failing manuk runs answered | extra vs rubric |
|---|---|---|---|
| Mon Jul 15 | Team Standup Meeting, Client Call, Team Brainstorming Session | those 3 + Coffee with Mentor, Dinner and Movie | +2 |
| Tue Jul 16 | Morning Walk, Team Workshop, Lunch with Boss | those 3 + Creative Session | +1 |
| Wed Jul 17 | Project Sync, Customer Feedback Session, Team Collaboration Work(shop) | those 3 + Follow-up Meeting, Dinner with Friends, Reading Time | +3 |
| Thu Jul 18 | Morning Coffee with sister, Team Check-In, Lunch with Marketing Team | those 3 + Workout Session | +1 |
| Fri Jul 19 | Breakfast Meeting with Client, Team Progress Check, Client Presentation | those 3 | 0 |

Missing from answers: 0 of 15. Misdated: 0 (identical per-day lists in all 3 runs, and identical to the passing runs). Engine is not the miss.

## Failing clause (inferred, NOT judge-confirmed)
The answer content is identical between passing and failing runs, so the content clause ("lists all events ... including <15>") is not what differs. What differs is the closing state sentence:
- failing manuk runs 1-3 each end "the final state is Day view for Fri Jul 19" / "currently showing the Day view" (run 3 of the earlier set also admits Tue-Fri were inferred);
- passing runs say "Month view is on".
The task goal is "Switch to the Month view", so the likely failing clause is the unmet Month-view end state (agent drilled into Day view via "+N more", which RG.2.0.b.a.b.a showed is a drill-down in Chrome too, and did not return). Chrome arm failures (B/B2 playwright) end the same way (Day view / unverified Month).

## Not done
Re-judging with and without the closing sentence, and a fresh Chrome-arm x1 with transcripts, were attempted but the judge endpoint hung (no reply to a trivial prompt within 100 s; 12-call run killed at 10 min). Hypothesis untested.

## Correction (RG.2.0.b.a.a.a.a.a)
Run 3 does not end in Day view; it ends "I did not open each of those days individually to confirm the rest." The closing-sentence test is in docs/sprint/rg/R20baaaaa-verdict.md: not confirmed on the fallback judge, primary judge rate-limited. The rubric has no view clause.
