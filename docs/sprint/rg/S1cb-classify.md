# RG.S1.c.b: opendining-8 x3 under manuk at HEAD (chrome_ver=145.0.7632.116, judge openai/gpt-oss-120b)

Raw rows: docs/sprint/rg/S1cb-real-manuk.tsv (3 rows, every row chrome_ver and judge_model pinned).
Tally: **1 of 3 pass** (run 2 pass 3/3; runs 1 and 3 pass 2/3). Gate "pass>=2 of 3" NOT MET.
Reference: W5.B1 pass; RG.E1.b.a 2/3 (docs/sprint/rg/E1ba-real-manuk.tsv).

| run | pass | booked | how the restaurant was chosen |
|---|---|---|---|
| 1 | 2/3 | The Royal Dine (4.02) | read only the home page's five cards; never clicked "Let's go"; said it "did not check for more" |
| 2 | 3/3 | Bar Central (3.74) | clicked "Let's go" (routes to /s?...), scrolled the results to the end, found Bar Central |
| 3 | 2/3 | The Royal Dine (4.02) | same as run 1: five home-page cards only, no search |

## Classification
- The failing mechanism is the agent's choice of page. The home page shows five restaurant cards (Royal Dine 4.02, River View Café 4.18, Vintage Bites 4.20, Evening Delight 4.20, Fancy Lights Bistro 4.33). The full list is on the results page reached by "Let's go" (h12). Runs 1 and 3 never left the home page, so the list they saw was incomplete.
- Engine: not shown. The extract on the home page is faithful to the page it read (five cards with ratings and review counts; the RG.E1.b fix holds here), and run 2 shows the results page extracts Bar Central with its rating. Whether Chrome's home page also shows only five cards has NOT been checked yet; that is the next node (RG.S1.c.b.a), because the home page reports 1003 offscreen hidden text fragments that the extract withholds by default.
- Judge: the answers in runs 1 and 3 name a restaurant that the page does show, so the verdict is not judge variance. The three evals are jmespath (no llm_boolean), so no judge call decided any pass/fail here.

## Gate
- Tally (one row per run, pinned chrome_ver and judge_model): `python3 -I docs/sprint/rg/S1cb-gate.py docs/sprint/rg/S1cb-real-manuk.tsv 145.0.7632.116` exits 1 on the real rows (count 1 of 3). Mutants: a judge_model changed on every row goes red; a wrong chrome_ver goes red.
- The node reports the count, not a fix: no engine change is claimed, and the task is not edited to fit the result.
