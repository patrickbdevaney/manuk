## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a

chrome_ver=145.0.7632.116. opendining-4 replay, step #7 = `act ('combobox', 'Select time')`.

- manuk: comboboxes before/after 4/0, options before/after 0/48; act result: `ok: dispatched click. +50 ~0 -136 text changed; new (names are page content): [h336] button [clickable], [h337] option "12:00 AM", [h338] option "12:30 AM", [h339] option "1:00 AM"; url unchanged; writes 0 page: pee84b4:47`
- chrome: comboboxes before/after 4/4, options before/after 0/0; act result: `OCCLUDED: h5 is covered by [h108] button ""; deal with that first (nothing was clicked) undo: restore("auto6") page: p02fac0:35`
- divergence shown: True

### Reading (chrome_ver=145.0.7632.116)

- The step-#7 click is not the same action on the two backends: on Chrome it never happens. Chrome answers `OCCLUDED: h5 is covered by [h108] button ""; deal with that first (nothing was clicked)`, so comboboxes stay 4/4 and options 0/0. manuk dispatches the click and the ARIA dropdown opens (4 -> 0 comboboxes, 0 -> 48 options).
- Why Chrome is occluded: its "Select date" dialog is still open at step #7. Step #6 (`button "2"`, a calendar day) is unresolved on Chrome (per-step dump: step 6 gap, MODAL present through step 7) because Chrome's tree lists the days as `cell`, not `button` (step 4 new nodes include `cell "31"`); manuk lists them as `button "N" [clickable]`, so manuk's step-6 click picks the day, the date dialog closes, and the step-7 click reaches 'Select time'.
- So the 9/13 miss traces upstream of the combobox: a calendar-day role difference (manuk button vs chrome cell) decides whether the date dialog is closed at step #7. The 'no change / 0 click listeners' of the recording is not reproduced by either backend; the replay's step #7 differs from the recording on both.
