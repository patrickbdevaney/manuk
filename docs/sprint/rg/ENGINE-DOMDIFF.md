# RG.0.a ENGINE cohort DOM diff

chrome_ver=145.0.7632.116. Replays the load-time page of the failing step (not in-page actions: handles are per-page). Node tree = observe(full), ids stripped.

## dashdish-6
- failing signal: stale-handle (manuk r1 step 19 (extract): STALE page handle pc23959:18: that page is at seq 19; use pc23959:19)
- replayed page: https://evals-dashdish.vercel.app/store/mashaallah-halal-food
- nodes: manuk 4, chrome 3
- verdict: first differing node at index 3 (similarity 0.857)

manuk:
```
#### Item Not Found
- We couldn't find the item you're looking for. It might have been removed or does…
button "Go Back"
button "Dashdish" [clickable] ↓offscreen
```
chrome:
```
#### Item Not Found
- We couldn't find the item you're looking for. It might have been removed or does…
button "Go Back"
```

## fly-unified-11
- failing signal: answer-reports-overlay (manuk r1 step 20 (final-answer): I did not book the flight. The search never reached results, so there's no flight to confirm. What I got do)
- replayed page: https://evals-fly-unified.vercel.app
- nodes: manuk 77, chrome 75
- verdict: first differing node at index 0 (similarity 0.474)

manuk:
```
link "Airlines logo"
- image "Airlines logo"
button "Book"
button "My trips"
```
chrome:
```
- generic "🚀 Version 2.0 launches soon! Stay tuned. This website is a non-commercial clone …"
button "close"
- image "Image 1 of 5"
- tablist "booking tabs"
```

## gocalendar-2
- failing signal: same-page-state-different-decision (manuk r2 step 8 (-): In Month view for July 2024, all the events are on July 15–19. No other July dates show any events. Month view collapse)
- replayed page: https://evals-gocalendar.vercel.app
- nodes: manuk 177, chrome 204
- verdict: first differing node at index 0 (similarity 0.415)

manuk:
```
button "Create event menu" [collapsed]
- image "create event"
- Create
- generic "Select date"
```
chrome:
```
- generic "🚀 Version 2.0 launches soon! Stay tuned. This website is a non-commercial clone …"
button "close"
button "Create event menu" [collapsed]
- image "create event"
```

## gocalendar-3
- failing signal: answer-reports-overlay (manuk r1 step 10 (final-answer): I did not delete the event. "Breakfast Meeting with Client" (10am–11am, Friday July 19, 2024) is visible in)
- replayed page: https://evals-gocalendar.vercel.app/calendar
- nodes: manuk 177, chrome 204
- verdict: first differing node at index 0 (similarity 0.415)

manuk:
```
button "Create event menu" [collapsed]
- image "create event"
- Create
- generic "Select date"
```
chrome:
```
- generic "🚀 Version 2.0 launches soon! Stay tuned. This website is a non-commercial clone …"
button "close"
button "Create event menu" [collapsed]
- image "create event"
```

## gocalendar-4
- failing signal: stale-handle (manuk r1 step 10 (key): bad page handle "pec4606": expected "<id>:<seq>" as returned by the last call)
- replayed page: https://evals-gocalendar.vercel.app
- nodes: manuk 177, chrome 204
- verdict: first differing node at index 0 (similarity 0.415)

manuk:
```
button "Create event menu" [collapsed]
- image "create event"
- Create
- generic "Select date"
```
chrome:
```
- generic "🚀 Version 2.0 launches soon! Stay tuned. This website is a non-commercial clone …"
button "close"
button "Create event menu" [collapsed]
- image "create event"
```

## staynb-2
- failing signal: answer-reports-overlay (manuk r1 step 8 (final-answer): I did not finish the booking. A modal overlay on the stay page blocks everything behind it, and I couldn't g)
- replayed page: https://evals-staynb.vercel.app
- nodes: manuk 753, chrome 759
- verdict: first differing node at index 0 (similarity 0.730)

manuk:
```
- generic "Return home"
- image "logo"
button "Stays"
button "Experiences"
```
chrome:
```
- generic "🚀 Version 2.0 launches soon! Stay tuned. This website is a non-commercial clone …"
button "close"
- generic "Return home"
- image "logo"
```

## staynb-4
- failing signal: occluded-hit-test (manuk r1 step 23 (act): OCCLUDED: h9 is covered by [h8] textbox "Who"; deal with that first (nothing was clicked) undo: restore("auto13") pa)
- replayed page: https://evals-staynb.vercel.app
- nodes: manuk 753, chrome 759
- verdict: first differing node at index 0 (similarity 0.730)

manuk:
```
- generic "Return home"
- image "logo"
button "Stays"
button "Experiences"
```
chrome:
```
- generic "🚀 Version 2.0 launches soon! Stay tuned. This website is a non-commercial clone …"
button "close"
- generic "Return home"
- image "logo"
```

## udriver-7
- failing signal: same-page-state-different-decision (manuk r1 step 17 (-): You took **5 trips in June**, all in June 2024 according to the My Trips page (`/trips`): 1. The Landing San Francisco)
- replayed page: https://evals-udriver.vercel.app/trips
- nodes: manuk 48, chrome 82
- verdict: first differing node at index 0 (similarity 0.046)

manuk:
```
link "Udriver" ↓offscreen
link "Ride" ↓offscreen
link "Package" ↓offscreen
link "My trips" ↓offscreen
```
chrome:
```
- listitem "Udriver"
link "Udriver"
- listitem "Ride"
link "Ride"
```
