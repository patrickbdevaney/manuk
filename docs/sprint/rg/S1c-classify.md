# RG.S1.c.a: networkin-3 classification (chrome_ver=145.0.7632.116, judge openai/gpt-oss-120b)

Task networkin-3 has three deterministic jmespath evals (no llm_boolean): a post was added; its text contains 'event'/'Event'; its text contains 'network'/'Network'. Substring match is case-sensitive on the stored post text.

| source | rows (manuk) | outcome | cause class |
|---|---|---|---|
| S1c-real-manuk.tsv (RG.S1.c) | r1 unpublished, r2 unpublished, r3 published | 0/3 | agent behaviour: publish hesitation (answer says "publishing is outward-facing"); composer filled, Post button enabled, never clicked; 6-7 turns vs 8 |
| E1a-real-manuk.tsv | r1 3/3, r2 3/3, r3 unpublished | 2/3 | publish hesitation (r3) |
| R3a-real-manuk.tsv | r1 2/3, r2 unpublished, r3 3/3 | 1/3 | hesitation (r2) + wording miss (r1) |
| E1-real, E1r2a, E2c-ahead, E1ba | 3/3 published in each; scores 3/3 except E1-real r1 2/3 | pass | none (E1-real r1 wording miss) |
| S1ca-real-manuk.tsv (this node, HEAD ff7e4260) | r1 3/3, r2 2/3, r3 2/3 | 1/3 pass | wording miss x2: all three runs PUBLISHED; the stored text of r2 ("a networking evening", "Purpose", "lightning talks") and r3 ("Networking Night", "Light refreshments") contains 'network' but never the literal word 'event', so eval 2 fails |

Playwright arm (B-real, B2-real) published in both rows examined and scored 2/3 (the same 'event' eval), so the same grader miss hits the reference arm.

## Classification
- Engine: none found. In every row where the agent clicked Post, the typed text reached app state intact (transcripts: the full typed body appears in the feed snapshot and in feedPostsDiff; add-post count eval passes 3/3 in S1ca). Fills, click on Post and state_ok=1 all work.
- Agent behaviour, two modes, both haiku variance: (a) publish hesitation (0 of 3 published in S1c, 1 of 3 in E1a and R3a, 0 of 3 in S1ca); (b) omitting the literal word "event" from a post written for a networking *event* (S1ca r2, r3).
- No engine fixture is warranted (no engine cause), so no RED/GREEN is claimed. The task is not edited to fit results.

## Gate
networkin-3 x3 at HEAD ff7e4260: pass 1 of 3 (docs/sprint/rg/S1ca-real-manuk.tsv). Gate "pass>=2 of 3 or next blocker named" -> blocker named: haiku wording variance on the 'event' substring eval (agent behaviour, not engine). Earlier rounds of the same task at E1ba/E2c/E1r2a were 3/3, so the task is a noisy 1/3..3/3 band, not a regression.
