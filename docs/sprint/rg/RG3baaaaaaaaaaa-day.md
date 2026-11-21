## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a

chrome_ver=145.0.7632.116. Why opendining's calendar day '2' was `button` on manuk but `cell` on the Chrome backend.

- Cause: the in-process session (`mcp/src/session.rs` `promote`, W4.A4) turns ANY non-interactive element the page makes clickable into `button [clickable]`. The Chrome backend (`mcp/src/cdp.rs`) only recorded a clickable hint when the role was `Generic`; the day is `role=gridcell`, which the backend maps to `Role::Cell`, so it stayed an unactionable `cell "2"` and the replay's `button "2"` (step #6) was unresolved there.
- Fix: `mcp/src/cdp.rs` records the hint for `Role::Generic | Role::Cell`. Probe `bench/agent-tasks/rg3baaaaaaaaaaa_day.py` (MUI-shape grid of `<button role=gridcell onclick>` plus a `<td role=gridcell onclick>`): before the fix chrome printed no day handles (exit 1, `roles chrome: []`); after, both backends print `button 1/2/7` (exit 0).
- Live page, opendining-4 at step 5 (`find "2"`): manuk `[h303] button "2" [clickable]`; chrome before `cell "2"`, after `[h125] button "2" [clickable]`. Step #6 now resolves and clicks on Chrome ("selected date is Nov 2, 2024").
- NOT fixed by this: the step-#7 click on Chrome is still `OCCLUDED ... covered by button ""`. After the day click Chrome's date dialog ("Select date", with Cancel/OK, MODAL OVERLAY) stays open; manuk's picker ("November 2024", no OK button) closes on the day click. The two engines are served different picker variants (desktop vs modal) — follow-up node.
