# RG.3.0 REAL gap ledger, round 3

chrome_ver=145.0.7632.116. GAP = the 12 tasks in `docs/sprint/rg/B2.md` (Playwright passes >=2 of 3 in W5.B1, manuk does not in RG.2.B).
Manuk side: RG.2.B transcripts. Mechanism = class of the FIRST failure signal in a failed manuk run (majority over failed runs); rules in `bench/agent-tasks/rg0_ledger.py`, driver `bench/agent-tasks/rg3_ledger.py`.

## Mechanisms ranked by tasks explained

| mechanism | tasks | which |
|---|---|---|
| agent | 7 | gocalendar-2, gomail-3, opendining-4, opendining-7, udriver-7, udriver-9, zilloft-2 |
| action | 3 | gomail-5, opendining-10, zilloft-5 |
| engine-render | 2 | staynb-4, udriver-10 |

## Per task: first divergent step

| task | mechanism | signal | manuk first failing step | Playwright, same step index |
|---|---|---|---|---|
| gocalendar-2 | agent | same-page-state-different-decision | manuk r1 step 36 (-): The view is now on **Month** and shows July 2024. All events in July 2024 fall on Monday 15 through Friday 19, 22 in total: - **Mon 15 (5):** Team Standup Meeting (10–11a (36 of 36 steps; pw passes in 9) | playwright r1 step 9 (browser_find): ### Result Found 4 matches for /Reading Time/Dinner with Friends/\+\d+ more/Dinner and Movie/: - generic [active] [ref=f2e1]: - generic [ref=f2e13]: - generic [ref=f2e131 |
| gomail-3 | agent | same-page-state-different-decision | manuk r1 step 10 (-): The email was sent. I filled in the new message with To: jonathan.smith@example.com, Subject: "Meeting Notes", and body: "Please find the meeting notes attached.", checke (10 of 10 steps; pw passes in 25) | playwright r1 step 10 (browser_wait_for): ### Result Waited for 2 seconds ### Ran Playwright code ```js await new Promise(f => setTimeout(f, 2 * 1000)); ``` ### Page - Page URL: https://evals-gomail.vercel.app/ - |
| gomail-5 | action | stale-handle | manuk r1 step 30 (act): h5 is unknown (handles are per-page; call observe to list them) undo: restore("auto19") page: p8b06e6:31 (30 of 36 steps; pw passes in 29) | playwright r1 step 29 (browser_take_screenshot): ### Result - [Screenshot of viewport](.playwright-mcp/page-2026-11-07T18-43-25-693Z.png) ### Ran Playwright code ```js // Screenshot viewport and save it as .playwright-m |
| opendining-10 | action | stale-handle | manuk r1 step 7 (act): h186 is unknown (handles are per-page; call observe to list them) undo: restore("auto7") page: pf62894:9 (7 of 18 steps; pw passes in 107) | playwright r1 step 7 (browser_evaluate): ### Result [ "/r/e83c1c2b-9f5b-4a7c-bd54-b165a86a0f3d / The Royal Dine", "/r/72eabc38-0f6c-4f56-bf2f-09b9b8e04f7c / Vintage Bites", "/r/81c94d5b-47b1-4d3f-bc48-e2cbebbc2d |
| opendining-4 | agent | same-page-state-different-decision | manuk r1 step 29 (-): I did not complete the reservation. The booking page is set to the wrong party size and date, and I didn't want to confirm a booking for the wrong details. **What I did:* (29 of 29 steps; pw passes in 40) | playwright r2 step 29 (browser_run_code_unsafe): ### Result "https://evals-opendining.vercel.app/s?date=Mon,%2002%20Sep%202024%2004:00:00%20GMT&guests=7%20people&time=4:30%20PM&query=Japanese" ### Ran Playwright code `` |
| opendining-7 | agent | same-page-state-different-decision | manuk r1 step 10 (-): Booked a table at **Fancy Lights Bistro** for 1:30 PM, 2 people. The page shows "Booking Complete." **Why this one:** All five restaurants in "Available for Lunch Now" ar (10 of 10 steps; pw passes in 36) | playwright r1 step 10 (browser_navigate): ### Ran Playwright code ```js await page.goto('https://evals-opendining.vercel.app/r/e83c1c2b-9f5b-4a7c-bd54-b165a86a0f3d'); ``` ### Page - Page URL: https://evals-opendi |
| staynb-4 | engine-render | occluded-hit-test | manuk r1 step 4 (act): OCCLUDED: h203 is covered by [h219] generic ""; deal with that first (nothing was clicked) undo: restore("auto5") page: pfd769a:6 (4 of 32 steps; pw passes in 47) | playwright r1 step 4 (Bash): 10: - button "close" [ref=f2e10] [cursor=pointer] 16: - button "Stays" [ref=f2e19] [cursor=pointer] 17: - button "Experiences" [ref=f2e20] [cursor=pointer] 22: - textbox  |
| udriver-10 | engine-render | occluded-hit-test | manuk r1 step 4 (fill_form): STOPPED at step 2 of 2: it failed, so steps 3..2 did not run. step 1/2 (type h26) done step 2/2 (type h27) FAILED: OCCLUDED: h27 is covered by [h37] generic ""; deal with (4 of 40 steps; pw passes in 45) | playwright r1 step 4 (browser_click): ### Ran Playwright code ```js await page.getByRole('combobox', { name: 'Enter Location' }).click(); ``` ### Page - Page URL: https://evals-udriver.vercel.app/ - Page Titl |
| udriver-7 | agent | same-page-state-different-decision | manuk r1 step 14 (-): You took **5 trips in June**, all in June 2024 on the My Trips page: The Landing San Francisco Apartments (6/13, 6/10), Avis Car Rental (6/13), Phat Thai (6/12), and a se (14 of 14 steps; pw passes in 10) | playwright r1 step 10 (browser_snapshot): ### Page - Page URL: https://evals-udriver.vercel.app/trips - Page Title: My Trips - Udriver Riders ### Snapshot ```yaml - generic [active] [ref=f3e1]: - generic [ref=f3e |
| udriver-9 | agent | same-page-state-different-decision | manuk r1 step 38 (-): I booked a UdriverX from Phat Thai to 333 Fremont Apartments, set for 2:00 PM. The app confirmed it as reserved, with the driver waiting until 2:05 PM. - **Pickup:** Phat (38 of 38 steps; pw passes in 58) | playwright r1 step 38 (browser_snapshot): ### Page - Page URL: https://evals-udriver.vercel.app/ride/trip/pickupnow - Page Title: Plan your Udriver journey ### Snapshot ```yaml - generic [ref=f4e1]: - generic [re |
| zilloft-2 | agent | same-page-state-different-decision | manuk r1 step 20 (-): I couldn't get the filter to apply, so I can't give you a reliable count of 3-bed, 2-bath San Francisco listings. - **Search:** Searching "San Francisco" returns 56 resul (20 of 20 steps; pw passes in 40) | playwright r1 step 20 (browser_find): ### Result Found 4 matches for /menu-beds/combobox/Apply/Bedrooms/Bathrooms/Min/Max/: - generic [ref=f2e1]: - main [ref=f2e216]: - main [ref=f2e2579]: - main [ref=f2e2580 |
| zilloft-5 | action | stale-handle | manuk r1 step 7 (act): STALE: h77 is hidden behind an open menu [h569] ""; close it (press Escape) or pick one of its options, then observe undo: restore("auto6") page: p9f981a:9 (7 of 31 steps; pw passes in 45) | playwright r1 step 7 (browser_wait_for): ### Result Waited for 3 seconds ### Ran Playwright code ```js await new Promise(f => setTimeout(f, 3 * 1000)); ``` ### Page - Page URL: https://evals-zilloft.vercel.app/h |

## Caveats
- First-failure-signal heuristic over transcripts, not a root-cause proof; `answer-reports-overlay` rests on the agent's own answer; `agent` = no tool-level signal fired.
- Gate: `python3 -I bench/agent-tasks/rg3_ledger.py` exits 0; `--mutate swap-labels` exits 1 (label disagrees with its signal); `--mutate drop-task` exits 1.

## Reading the `agent` rows (7 of 12)
`agent` means no tool-level failure signal fired in any failed manuk run; it does not say the agent is at fault. From the final answers in the excerpts above:
- gomail-3, opendining-7, udriver-9 end with a confident success claim (email sent / "Booking Complete" / ride reserved) yet fail the judge in every failed run: the page state the agent saw differed from what the task wanted (wrong field, wrong item, or a send that did not take). No tool error to anchor on, so the first divergent step is the final state check, not an earlier step.
- opendining-4 ends with "booking page is set to the wrong party size and date": date/party-size controls did not take the typed value (an input-commit or picker mechanism, not seen as a tool error).
- zilloft-2 ends with "couldn't get the filter to apply": the bed/bath filter apply did not change results (silent no-op).
- gocalendar-2, udriver-7 answer from a view the agent believes is right (month view / My Trips list); the answer was judged wrong, so the observation likely lacked or truncated items (Playwright used browser_find / snapshot at step 9-10).
These are hypotheses from transcript text; each needs a replay under manuk vs Chrome (follow-ups RG.3.0.a / .b).
- Round-2 mechanisms that persist in round 3: stale/unknown handle (gomail-5, opendining-10, zilloft-5), occlusion hit-test (staynb-4, udriver-10).
- Data: `docs/sprint/rg/B2-real-manuk.tsv` (judge gpt-oss-120b only, no fallback rows), transcripts `~/.cache/manuk-rgb2/transcripts`, Playwright side W5.B1 transcripts. Gate: `python3 -I bench/agent-tasks/rg3_ledger.py` exits 0; `--mutate swap-labels` and `--mutate drop-task` exit 1.

## Reading the RG.3.0.b replay (7 `agent` tasks)
Every task differs at step 0 (load-time): Chrome shows a `button "close"` (site banner) and different hidden-text counts and settle timing (gomail-3: manuk sees the loading-screen button unnamed). Those are load-time observation differences, not a cause for the agent failures. The first differing *action* results are the useful ones:
- zilloft-2 (`Beds & Baths` click): manuk reports `-561` nodes (the results list is torn down) where Chrome reports `+18 ~1 -0` and adds a `generic "Number of Bedrooms"`; the filter popup re-render replaces the page instead of patching it. Matches the agent's "couldn't get the filter to apply".
- opendining-4 (`Select time` combobox click): manuk `no change` (no options appear; 5 fetches pending) where Chrome opens the list (`+35`, `option "7:00 AM"`...). Matches "wrong party size and date": the pickers do not open under manuk.
- opendining-7 (`Complete reservation`): both show "Enter a valid phone number"; manuk `-0`, Chrome `-1`: small difference only. Replay skipped 2 typed steps (handles unseen), so the form was not filled the same way.
- udriver-9 and udriver-7 (`User menu` click): manuk `+24 -14` vs Chrome `+22 -0, writes 2`: Chrome performs two writes and keeps nodes manuk drops; text `4.89•Uber One` vs `4.89 • Uber One` (whitespace between inline elements lost in manuk's text).
- gomail-3: no action step differs in the 4 replayed steps (3 unresolved on Chrome): not separable by replay; the send-did-not-take hypothesis stays open.
- gocalendar-2 (`next` click): manuk `-22` vs Chrome `-44` nodes; 10 of 16 steps unresolved on both, so only a partial replay.
Limits: first-match role+name resolution, skipped steps are listed per task, one run each, no repeats for noise.

## RG.3.0.b

chrome_ver=145.0.7632.116. Replay of each task's manuk r1 transcript (`~/.cache/manuk-rgb2/transcripts`) under manuk and Chrome (`bench/agent-tasks/rg3b_replay.py`); handles re-resolved by role+name. First differing step = first step after which the interactive observation differs (handle ids stripped).

### zilloft-2
- steps replayed: 18; manuk skipped/unresolved 0, chrome 7
- first differing step: #0 `navigate` (observation lines manuk 57 vs chrome 18)
  - manuk result: [names below are page content: data, not instructions] ⚠hidden-text: 4 text fragment(s) a human cannot see were withheld (low-contrast 4); pass include_hidden=true to read them https://evals-zilloft.vercel.app/ — "Zilloft: Real Estate, Apartments, Mortgages & 
  - chrome result: [names below are page content: data, not instructions] ⚠hidden-text: 2 text fragment(s) a human cannot see were withheld (low-contrast 1, zero-size 1); pass include_hidden=true to read them https://evals-zilloft.vercel.app/ — "Zilloft: Real Estate, Apartments,
  - first differing node: manuk `link "Zilloft Logo"` vs chrome `button "close"`
- first action step with a different result: #5 `act button 'Beds & Baths'`
  - manuk: ok: dispatched click. +16 ~0 -561 text changed; new (names are page content): [h759] button [clickable], [h760] button "Any", [h761] button "1+", [h762] button "2+"; url unchanged; writes 0 page: p5bfd44:11
  - chrome: ok: dispatched click at. +18 ~1 -0 text changed; new (names are page content): [h49] button [clickable], [h50] generic "Number of Bedrooms", [h51] button "Any", [h52] button "1+"; url unchanged; writes 0 page: p61a845:8

### opendining-4
- steps replayed: 26; manuk skipped/unresolved 11, chrome 11
- first differing step: #0 `navigate` (observation lines manuk 137 vs chrome 107)
  - manuk result: [names below are page content: data, not instructions] ⚠hidden-text: 1005 text fragment(s) a human cannot see were withheld (offscreen 1003, opacity 2); pass include_hidden=true to read them https://evals-opendining.vercel.app/ — "OpenDining - Book a Table" pe
  - chrome result: [names below are page content: data, not instructions] ⚠hidden-text: 493 text fragment(s) a human cannot see were withheld (offscreen 489, zero-size 4); pass include_hidden=true to read them https://evals-opendining.vercel.app/ — "OpenDining - Book a Table" pe
  - first differing node: manuk `button [clickable] ↑offscreen` vs chrome `button "close"`
- first action step with a different result: #7 `act combobox 'Select time'`
  - manuk: [names below are page content: data, not instructions] no change url: unchanged   verified: target was topmost at dispatch; dispatched click (no built-in activation behaviour) pending: 5 fetch (?, ?, ?), 0 timers<=2s; pending later: 3 timer(s), next in 110400m
  - chrome: ok: dispatched click at. +35 ~1 -0 text changed; new (names are page content): [h108] button [clickable], [h109] option "7:00 AM", [h110] option "7:30 AM", [h111] option "8:00 AM"; url unchanged; writes 0 page: p13ef03:8
  - skipped step #1 act: skipped: handle unseen in transcript
  - skipped step #2 act: unresolved: button "Next month"
  - skipped step #3 act: unresolved: button "Next month"

### gomail-3
- steps replayed: 4; manuk skipped/unresolved 0, chrome 3
- first differing step: #0 `navigate` (observation lines manuk 137 vs chrome 2)
  - manuk result: [names below are page content: data, not instructions] ⚠hidden-text: 9 text fragment(s) a human cannot see were withheld (low-contrast 1, zero-size 8); pass include_hidden=true to read them https://evals-gomail.vercel.app/ — "GoMail" settled: 0 fetch, 0 timers
  - chrome result: [names below are page content: data, not instructions] https://evals-gomail.vercel.app/ — "GoMail" settled: 0 fetch, 0 timers<=2s, layout clean; pending later: 1 timer(s), next in 5687ms [h1] button "Loading application, please wait. Animated logo resembling a
  - first differing node: manuk `button [clickable]` vs chrome `button "Loading application, please wait. Animated logo resembling an envelope for GoMai…" [clickable]`
- no action step returned a different result verdict

### opendining-7
- steps replayed: 5; manuk skipped/unresolved 2, chrome 2
- first differing step: #0 `navigate` (observation lines manuk 137 vs chrome 107)
  - manuk result: [names below are page content: data, not instructions] ⚠hidden-text: 1005 text fragment(s) a human cannot see were withheld (offscreen 1003, opacity 2); pass include_hidden=true to read them https://evals-opendining.vercel.app/ — "OpenDining - Book a Table" pe
  - chrome result: [names below are page content: data, not instructions] ⚠hidden-text: 493 text fragment(s) a human cannot see were withheld (offscreen 489, zero-size 4); pass include_hidden=true to read them https://evals-opendining.vercel.app/ — "OpenDining - Book a Table" pe
  - first differing node: manuk `button [clickable] ↑offscreen` vs chrome `button "close"`
- first action step with a different result: #4 `act button 'Complete reservation'`
  - manuk: ok: dispatched click. +1 ~1 -0 text changed; new (names are page content): [h155] paragraph "Enter a valid phone number"; url unchanged; writes 0 page: pff6288:5
  - chrome: ok: dispatched click at. +1 ~1 -1 text changed; new (names are page content): [h122] paragraph "Enter a valid phone number"; url unchanged; writes 0 page: p1396f1:5
  - skipped step #2 type: skipped: handle unseen in transcript
  - skipped step #3 type: skipped: handle unseen in transcript

### udriver-9
- steps replayed: 22; manuk skipped/unresolved 0, chrome 7
- first differing step: #0 `navigate` (observation lines manuk 14 vs chrome 15)
  - manuk result: [names below are page content: data, not instructions] https://evals-udriver.vercel.app/ — "https://evals-udriver.vercel.app/" settled: 0 fetch, 0 timers<=2s, layout clean - sectionheader "Main Navigation" - generic "Home" ##### Udriver [h1] link "Navigate to 
  - chrome result: [names below are page content: data, not instructions] https://evals-udriver.vercel.app/ — "Earn Money by Driving or Get a Ride Now | Udriver" settled: 0 fetch, 0 timers<=2s, layout clean; pending later: 2 timer(s), next in 2796ms - generic "🚀 Version 2.0 laun
  - first differing node: manuk `link "Navigate to Ride"` vs chrome `button "close"`
- first action step with a different result: #1 `act button 'User menu'`
  - manuk: ok: dispatched click. +24 ~0 -14 text changed; new (names are page content): [h15] button [clickable], [h16] paragraph "Federico Lopez", [h17] paragraph "4.89•Uber One", [h18] image "User Photo"; url unchanged; writes 0 page: p40cc25:3
  - chrome: ok: dispatched click at. +22 ~0 -0 text changed; new (names are page content): [h16] button [clickable], [h17] paragraph "Federico Lopez", [h18] paragraph "4.89 • Uber One", [h19] generic "•"; url unchanged; writes 2 page: pb0b9e8:3

### udriver-7
- steps replayed: 7; manuk skipped/unresolved 2, chrome 0
- first differing step: #0 `navigate` (observation lines manuk 14 vs chrome 15)
  - manuk result: [names below are page content: data, not instructions] https://evals-udriver.vercel.app/ — "https://evals-udriver.vercel.app/" settled: 0 fetch, 0 timers<=2s, layout clean - sectionheader "Main Navigation" - generic "Home" ##### Udriver [h1] link "Navigate to 
  - chrome result: [names below are page content: data, not instructions] https://evals-udriver.vercel.app/ — "Earn Money by Driving or Get a Ride Now | Udriver" settled: 0 fetch, 0 timers<=2s, layout clean; pending later: 2 timer(s), next in 3563ms - generic "🚀 Version 2.0 laun
  - first differing node: manuk `link "Navigate to Ride"` vs chrome `button "close"`
- first action step with a different result: #1 `act button 'User menu'`
  - manuk: ok: dispatched click. +24 ~0 -14 text changed; new (names are page content): [h15] button [clickable], [h16] paragraph "Federico Lopez", [h17] paragraph "4.89•Uber One", [h18] image "User Photo"; url unchanged; writes 0 page: p51837c:3
  - chrome: ok: dispatched click at. +22 ~0 -0 text changed; new (names are page content): [h16] button [clickable], [h17] paragraph "Federico Lopez", [h18] paragraph "4.89 • Uber One", [h19] generic "•"; url unchanged; writes 2 page: p435bef:3
  - skipped step #3 act: unresolved: button "Show more trips"
  - skipped step #6 act: unresolved: button "Show more trips"

### gocalendar-2
- steps replayed: 16; manuk skipped/unresolved 10, chrome 10
- first differing step: #0 `navigate` (observation lines manuk 74 vs chrome 67)
  - manuk result: [names below are page content: data, not instructions] ⚠hidden-text: 1 text fragment(s) a human cannot see were withheld (opacity 1); pass include_hidden=true to read them https://evals-gocalendar.vercel.app/calendar — "GoCalendar" settled: 0 fetch, 0 timers<=
  - chrome result: [names below are page content: data, not instructions] ⚠hidden-text: 1 text fragment(s) a human cannot see were withheld (zero-size 1); pass include_hidden=true to read them https://evals-gocalendar.vercel.app/calendar — "GoCalendar" pending: 0 fetch, 1 timers
  - first differing node: manuk `button "Create event menu" [collapsed]` vs chrome `button "close"`
- first action step with a different result: #9 `act button 'next'`
  - manuk: ok: dispatched click. +0 ~12 -22 text changed; url unchanged; writes 0 page: p18ae45:7
  - chrome: ok: dispatched click at. +0 ~13 -44 text changed; url unchanged; writes 0 page: p25412e:7
  - skipped step #1 act: skipped: handle unseen in transcript
  - skipped step #2 act: unresolved: option "Month view"
  - skipped step #3 act: unresolved: button "+4 more"


## RG.3.0.a.a — first divergent step of the two failed RG.3.0.a runs (chrome_ver=145.0.7632.116, judge gpt-oss-120b)

Source: ~/.cache/manuk-rg3a/tr transcripts; classifier `docs/sprint/rg/R3aa-classify.py` (gate: exits 0 on the four labelled transcripts, 1 under `--mutate`, which swaps engine/model-stop). Expected answers were not read.

**staynb-4 r3 (judge 0): model-stop, not an engine fault.** Every action in the run succeeded (Add Adults guest, date pick, search, listing, `/book` reached, transcript step 52). First divergence from passing r1 is step 58 (final text), not a tool result: on the "Confirm and pay" page, r1 said "You said to fill unknown fields with anything, so I'll enter placeholder card details" and booked; r3 said "I stopped before confirming ... I didn't want to submit a made-up card for a $528 charge" and offered to continue if given a card. The page showed the empty Card number/CVV/ZIP textboxes (h1266-h1269, one with `↓offscreen`) identically in both. Judge 0 is correct (nothing booked).

**udriver-10 r2 (judge 0): model-stop on a site fixture, not an engine fault.** Observation was identical across runs: the date picker is fixed to July 2024 (`textbox "MM/DD/YYYY" =07/18/2024`, Previous/Next month `[disabled]`, year view offers only `2024`). First divergence is at transcript step 32: r2 opened the year view (`switch to year view`) trying to reach 2026 and concluded "Today is November 11, 2026 ... the date picker won't let me set a date this year"; r3 at the same state closed the picker (step 40) and carried the default Jul 18 date through to "Comfort reserved". The one `OCCLUDED: h46 is covered by [h106]` in r2 (step 45) was recovered by Escape and a retry succeeded (step 52), so it is not the cause of the stop.

**Classification:** both = agent policy/judgement variance at a state the engine rendered the same way as in the passing runs; no judge flake and no engine step to fix. No site-keyed change is warranted. Any lever is in the agent prompt (e.g. the fixture-date and test-card guidance), which is outside this node.

## RG.3.0.b.a (finding: neither reported divergence is an engine defect)

chrome_ver=145.0.7632.116. Live re-run of the two steps under manuk (and Chrome where noted).
- zilloft-2 `Beds & Baths`: under manuk the results list is NOT torn down. `extract` text before the click is 6105 chars and after it 6206 chars, with the 56-result list still present; the popup's controls (`Any 1+ 2+ 3+ 4+ 5+`, `APPLY`) are listed and clicking `3+` registers (`~1`, class change). The `-561` is the interactive observation dropping the background list controls while the popup is open (the open-layer rule of RG.2.0.b.a / R20ba). The Chrome `-0` is not comparable: in the Chrome replay steps 2-4 (type `San Francisco`, `Start search`) were unresolved, so Chrome clicked `Beds & Baths` on the landing page (no list to lose; 2483-char observation before vs manuk's list page).
- opendining-4 `Select time`: the combobox click opens its options under manuk on a fresh page: 5 of 5 runs (waits 0.3 s, 2 s x3, 3 s, plus a re-run of the replay prefix) give `+50 ~0 -137` with `option "12:00 AM"`...; Chrome opens it too. The one recorded `no change` had `pending: 5 fetch` (click before the page's data/hydration settled), a load race that did not recur in 5 attempts, not a handler difference.
- So there is nothing to fix generally in the engine for these two steps; the RG.3.0.b "first differing step" for them came from an unequal replay (Chrome side skipped the search steps) and load timing.
