# RG.2.0 REAL gap ledger, round 2

chrome_ver=145.0.7632.116. GAP = the 11 tasks in `docs/sprint/rg/B.md` (Playwright passes >=2 of 3 in W5.B1, manuk does not in RG.B).
Manuk side: RG.B transcripts. Mechanism = class of the FIRST failure signal in a failed manuk run (majority over failed runs); rules in `bench/agent-tasks/rg0_ledger.py`, driver `bench/agent-tasks/rg2_ledger.py`.

## Mechanisms ranked by tasks explained

| mechanism | tasks | which |
|---|---|---|
| action | 4 | gocalendar-1, gocalendar-2, udriver-7, zilloft-5 |
| agent | 3 | fly-unified-1, fly-unified-6, opendining-10 |
| engine-render | 2 | dashdish-11, zilloft-2 |
| observation | 2 | gomail-3, gomail-5 |

## Per task: first divergent step

| task | mechanism | signal | manuk first failing step | Playwright, same step index |
|---|---|---|---|---|
| dashdish-11 | engine-render | occluded-hit-test | manuk r1 step 17 (act): OCCLUDED: h127 is covered by [h138] generic ""; deal with that first (nothing was clicked) undo: restore("auto10") page: p8b7bf4:19 (17 of 65 steps; pw passes in 37) | playwright r1 step 17 (browser_navigate): ### Ran Playwright code ```js await page.goto('https://evals-dashdish.vercel.app/cart'); ``` ### Page - Page URL: https://evals-dashdish.vercel.app/cart - Page Title: 404 |
| fly-unified-1 | agent | same-page-state-different-decision | manuk r2 step 30 (-): I could not find the earliest Savannah → Albuquerque flight for December 1, 2024. The site never displayed any results. What I did: - Set the search to one-way, Savannah  (30 of 30 steps; pw passes in 25) | playwright r1 step 25 (browser_snapshot): ### Page - Page URL: https://evals-fly-unified.vercel.app/choose-flight?search=%7B%22tripType%22%3A%22oneway%22%2C%22bookWithMiles%22%3Afalse%2C%22flexibleDates%22%3Afals |
| fly-unified-6 | agent | same-page-state-different-decision | manuk r1 step 35 (-): I couldn't reserve the seat. The flight search doesn't return any results, so there's no flight to book and no reservation was made. **What I did** - Set the search to on (35 of 35 steps; pw passes in 61) | playwright r1 step 35 (browser_snapshot): ### Page - Page URL: https://evals-fly-unified.vercel.app/choose-flight?cartId=90e858ac-9f70-42f9-8e42-703a327e1a62 - Page Title: Airlines Airways ### Snapshot ```yaml -  |
| gocalendar-1 | action | stale-handle | manuk r1 step 76 (act): h1435 is unknown (handles are per-page; call observe to list them) undo: restore("auto30") page: p0c6108:63 (76 of 160 steps; pw passes in 69) | playwright r1 step 69 (browser_take_screenshot): ### Result - [Screenshot of viewport](.playwright-mcp/page-2026-11-07T18-05-44-489Z.png) ### Ran Playwright code ```js // Screenshot viewport and save it as .playwright-m |
| gocalendar-2 | action | stale-handle | manuk r2 step 10 (act): STALE: h199 is no longer on the page (call observe) undo: restore("auto6") page: p0db112:11 (10 of 33 steps; pw passes in 9) | playwright r1 step 9 (browser_find): ### Result Found 4 matches for /Reading Time/Dinner with Friends/\+\d+ more/Dinner and Movie/: - generic [active] [ref=f2e1]: - generic [ref=f2e13]: - generic [ref=f2e131 |
| gomail-3 | observation | target-absent-from-observe | manuk r2 step 6 (find): no match for "Compose" page: p16c1a6:7 (6 of 19 steps; pw passes in 25) | playwright r1 step 6 (browser_wait_for): ### Result Waited for 10 seconds ### Ran Playwright code ```js await new Promise(f => setTimeout(f, 10 * 1000)); ``` ### Page - Page URL: https://evals-gomail.vercel.app/ |
| gomail-5 | observation | target-absent-from-observe | manuk r1 step 8 (find): no match for "Compose" page: p889f24:9 (8 of 33 steps; pw passes in 29) | playwright r1 step 8 (browser_take_screenshot): ### Result - [Screenshot of viewport](.playwright-mcp/page-2026-11-07T18-42-47-699Z.png) ### Ran Playwright code ```js // Screenshot viewport and save it as .playwright-m |
| opendining-10 | agent | same-page-state-different-decision | manuk r2 step 82 (-): I did not complete the reservation. I checked all 20 menus and picked The Vegan Table, but the booking form rejected my phone number and I ran out of budget before I coul (82 of 82 steps; pw passes in 107) | playwright r1 step 82 (browser_evaluate): ### Result { "val": "", "active": true } ### Ran Playwright code ```js await page.evaluate('async () => { const i = document.querySelector(\'input[id=":r8:"]\'); return { |
| udriver-7 | action | stale-handle | manuk r1 step 11 (act): STALE: h84 is no longer on the page (call observe) undo: restore("auto7") page: pc45e08:13 (11 of 41 steps; pw passes in 10) | playwright r1 step 10 (browser_snapshot): ### Page - Page URL: https://evals-udriver.vercel.app/trips - Page Title: My Trips - Udriver Riders ### Snapshot ```yaml - generic [active] [ref=f3e1]: - generic [ref=f3e |
| zilloft-2 | engine-render | occluded-hit-test | manuk r1 step 10 (act): STOPPED at step 1 of 3: it failed, so steps 2..3 did not run. step 1/3 (act h269) FAILED: OCCLUDED: h269 is covered by [h281] generic ""; deal with that first (nothing wa (10 of 50 steps; pw passes in 40) | playwright r1 step 10 (browser_find): ### Result Found 1 match for /bed/bath/filter/i: - generic [active] [ref=f2e1]: - main [ref=f2e216]: - navigation "Main navigation menu" [ref=f2e219]: ... - generic [ref= |
| zilloft-5 | action | stale-handle | manuk r1 step 7 (act): STALE: h39 is no longer on the page (call observe) undo: restore("auto6") page: pf605b6:9 (7 of 70 steps; pw passes in 45) | playwright r1 step 7 (browser_wait_for): ### Result Waited for 3 seconds ### Ran Playwright code ```js await new Promise(f => setTimeout(f, 3 * 1000)); ``` ### Page - Page URL: https://evals-zilloft.vercel.app/h |

## Caveats
- First-failure-signal heuristic over transcripts, not a root-cause proof; `answer-reports-overlay` rests on the agent's own answer; `agent` = no tool-level signal fired.
- fly-unified-1 and fly-unified-6 are labelled `agent` because no tool-level signal fired, but both final answers say the flight search never returned results while Playwright reached `/choose-flight`; that is likely an engine or navigation/settle cause, not a decision difference (follow-up RG.2.0.a).
- Gate: `python3 -I bench/agent-tasks/rg2_ledger.py` exits 0; `--mutate swap-labels` exits 1 (label disagrees with its signal); `--mutate drop-task` exits 1.
