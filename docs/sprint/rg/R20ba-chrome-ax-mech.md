# RG.2.0.b.a.b.a — mechanism behind the dropped Month cells after "+N more"

chrome_ver=145.0.7632.116 (pin `MANUK_CHROME_PIN`). Probe: `R20ba-chrome-ax-mech.js`; output `R20ba-chrome-ax-mech.json`; gate `R20ba-chrome-ax-mech-gate.py`.

**Mechanism: not hiding at all. The "+N more" click drills down to a day view and react-big-calendar unmounts the Month view.**

| | before click | after click ("+2 more") |
|---|---|---|
| Month-grid day cell (`.rbc-month-view .rbc-button-link`) | attached, in AX, not ignored, 0 hidden ancestors | **detached from DOM** (0 cells; `DOM.querySelector` finds none) |
| mounted rbc view | `rbc-month-view` | `rbc-time-view` |
| dialog / aria-modal / popover / `.rbc-overlay` | 0 | 0 |
| kept control (a non-grid button) | attached, in AX, 0 hidden ancestors | same: attached, in AX, 0 hidden ancestors |

Document buttons fall 77 -> 42. No `inert`, `aria-hidden`, `aria-modal` or `hidden` on any ancestor of the kept control, which is why RG.2.0.b.a.b saw 0 of 45 `aria-hidden` siblings. The earlier "dropped 33 of 41" counted nodes by role|name; the Month cells really are gone, and the sidebar mini-calendar's MUI cells reappear as `gridcell|1` (role changed from button), so they are also not matched.

**Consequence.** gocalendar-2's "+N more" is a navigation, not an open layer. Chrome's AX tree being "without" those cells says nothing about a popover. A manuk "hidden behind an open layer" refusal would be wrong here; the right model is stale handles after a view change (handles to unmounted nodes). It also corrects the RG.2.0.b.a.b reading for this one state (the view-popover and zilloft rows stand).

**Gate.** Real probe passes 6/6 checks; `MUTANT=1` (probe reads an aria-hidden-free chain as hidden) fails 3 of 6.
