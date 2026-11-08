# RG.0 REAL gap ledger

chrome_ver=145.0.7632.116, W5.B1 (3 runs x 102, Haiku). GAP = playwright passes >=2 of 3 and manuk <=1 of 3: 23 tasks.
Mechanism = class of the FIRST failure signal in a failed manuk run (majority over its failed runs); rules in `bench/agent-tasks/rg0_ledger.py`.

## Mechanisms ranked by tasks explained

| mechanism | tasks | ENGINE | SHARED |
|---|---|---|---|
| engine-render | 10 | 4 | 6 |
| action | 8 | 2 | 6 |
| observation | 3 | 0 | 3 |
| agent | 2 | 2 | 0 |

## ENGINE cohort

| task | mechanism | signal | evidence |
|---|---|---|---|
| dashdish-6 | action | stale-handle | manuk r1 step 19 (extract): STALE page handle pc23959:18: that page is at seq 19; use pc23959:19 |
| fly-unified-11 | engine-render | answer-reports-overlay | manuk r1 step 20 (final-answer): I did not book the flight. The search never reached results, so there's no flight to confirm. What I got done on evals-fly-unified.vercel.app: - Set trip type to One-way, |
| gocalendar-2 | agent | same-page-state-different-decision | manuk r2 step 8 (-): In Month view for July 2024, all the events are on July 15–19. No other July dates show any events. Month view collapses each day to one visible event plus a "+N more" bu |
| gocalendar-3 | engine-render | answer-reports-overlay | manuk r1 step 10 (final-answer): I did not delete the event. "Breakfast Meeting with Client" (10am–11am, Friday July 19, 2024) is visible in the week view, but clicking it brings up a modal overlay with  |
| gocalendar-4 | action | stale-handle | manuk r1 step 10 (key): bad page handle "pec4606": expected "<id>:<seq>" as returned by the last call |
| staynb-2 | engine-render | answer-reports-overlay | manuk r1 step 8 (final-answer): I did not finish the booking. A modal overlay on the stay page blocks everything behind it, and I couldn't get past it. - **Stay chosen:** "Cabin in the woods" (Asheville |
| staynb-4 | engine-render | occluded-hit-test | manuk r1 step 23 (act): OCCLUDED: h9 is covered by [h8] textbox "Who"; deal with that first (nothing was clicked) undo: restore("auto13") page: p150d6e:22 |
| udriver-7 | agent | same-page-state-different-decision | manuk r1 step 17 (-): You took **5 trips in June**, all in June 2024 according to the My Trips page (`/trips`): 1. The Landing San Francisco Apartments, 6/10/2024, $28.50 2. Phat Thai, 6/12/20 |

## SHARED cohort

| task | mechanism | signal | evidence |
|---|---|---|---|
| dashdish-10 | engine-render | occluded-hit-test | manuk r1 step 18 (act): OCCLUDED: h121 is covered by [h173] image "Image GINTO Express"; deal with that first (nothing was clicked) undo: restore("auto12") page: p70a6e2:19 |
| dashdish-11 | action | stale-handle | manuk r1 step 32 (key): STALE page handle p5cc7eb:31: that page is at seq 32; use p5cc7eb:32 |
| dashdish-2 | action | stale-handle | manuk r1 step 4 (act): h6 is unknown (handles are per-page; call observe to list them) undo: restore("auto5") page: p28dda3:6 |
| dashdish-7 | action | stale-handle | manuk r1 step 2 (act): h4 is unknown (handles are per-page; call observe to list them) undo: restore("auto3") page: pdc68c2:4 |
| fly-unified-1 | engine-render | answer-reports-overlay | manuk r1 step 30 (final-answer): I couldn't get the flight results, so I don't have an answer to the earliest Savannah → Albuquerque flight on December 1, 2024. What happened: - I set the search to one-w |
| fly-unified-6 | action | stale-handle | manuk r1 step 20 (act): STALE: h37 is no longer on the page (call observe) undo: restore("auto15") page: p3eaad7:21 |
| gocalendar-1 | action | stale-handle | manuk r2 step 19 (act): STALE: h151 is no longer on the page (call observe) undo: restore("auto9") page: pd56f17:20 |
| gomail-3 | observation | target-absent-from-observe | manuk r1 step 6 (find): no match for "Compose" page: p45b981:7 |
| gomail-5 | observation | target-absent-from-observe | manuk r1 step 7 (find): no match for "Compose" page: pcfaf61:7 |
| opendining-10 | action | stale-handle | manuk r1 step 28 (observe): bad page handle "pa37a4c": expected "<id>:<seq>" as returned by the last call |
| opendining-4 | engine-render | occluded-hit-test | manuk r1 step 15 (act): OCCLUDED: h282 is covered by [h315] generic ""; deal with that first (nothing was clicked) undo: restore("auto8") page: p7d442c:13 |
| opendining-5 | observation | target-absent-from-observe | manuk r1 step 2 (find): no match for "Ocean Breeze" page: p3aac60:4 |
| udriver-8 | engine-render | occluded-hit-test | manuk r1 step 2 (fill_form): STOPPED at step 2 of 2: it failed, so steps 3..2 did not run. step 1/2 (type h13) done step 2/2 (type h15) FAILED: OCCLUDED: h15 is covered by [h19] option "333 Fremont A |
| zilloft-2 | engine-render | occluded-hit-test | manuk r2 step 13 (act): OCCLUDED: h417 is covered by [h426] generic ""; deal with that first (nothing was clicked) undo: restore("auto11") page: pc9a7b1:15 |
| zilloft-5 | engine-render | answer-reports-overlay | manuk r1 step 7 (final-answer): I could not get the answer. I did not find any listings, so I don't have a result count. What happened: - The Zilloft homepage opens with a modal overlay that has no clos |

## Caveats
- Classification is a first-failure-signal heuristic over transcripts, not a root-cause proof. `answer-reports-overlay` rows rest on the agent's own final answer (weaker than an OCCLUDED tool result). `stale-handle` covers STALE, unknown and malformed page handles (action layer, per W5.G1). `agent` = no signal fired.
- The ENGINE-cohort `--dump-dom` replay against Chrome is NOT done here; it is follow-up RG.0.a.
- Gate: `python3 -I bench/agent-tasks/rg0_ledger.py` exits 0; `--mutate drop-task` exits 1 (22 tasks).
