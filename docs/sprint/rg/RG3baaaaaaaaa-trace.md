## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a

chrome_ver=145.0.7632.116. Per ordinal step: wanted (role, n) vs live count of that role, as the replay saw it before the step.

- opendining-4 manuk #13: tool=act wanted combobox n=1 (transcript `[h8] combobox [collapsed]`); live combobox count after step #12: 0; at step: 0 gap=True; all roles at step: button=2, option=48
- opendining-4 manuk #14: tool=act wanted combobox n=3 (transcript `[h10] combobox [collapsed]`); live combobox count after step #13: 0; at step: 0 gap=True; all roles at step: button=2, option=48
- opendining-4 chrome #13: tool=act wanted combobox n=1 (transcript `[h8] combobox [collapsed]`); live combobox count after step #12: 4; at step: 4 gap=False; all roles at step: button=58, combobox=4, link=48, textbox=2
- opendining-4 chrome #14: tool=act wanted combobox n=3 (transcript `[h10] combobox [collapsed]`); live combobox count after step #13: 4; at step: 4 gap=False; all roles at step: button=58, combobox=4, link=48, textbox=2
- gocalendar-2 manuk #6: tool=act wanted button n=47 (transcript `[h76] button [clickable] ↑offscreen`); live button count after step #5: 51; at step: 8 gap=False; all roles at step: button=8, link=1
- gocalendar-2 manuk #7: tool=act wanted combobox n=0 (transcript `[h47] combobox [collapsed]`); live combobox count after step #6: 0; at step: 0 gap=True; all roles at step: button=8, link=1
- gocalendar-2 chrome #6: tool=act wanted button n=47 (transcript `[h76] button [clickable] ↑offscreen`); live button count after step #5: 92; at step: 99 gap=False; all roles at step: button=99, combobox=1, link=1
- gocalendar-2 chrome #7: tool=act wanted combobox n=0 (transcript `[h47] combobox [collapsed]`); live combobox count after step #6: 1; at step: 1 gap=False; all roles at step: button=99, combobox=1, link=1

### Reading (chrome_ver=145.0.7632.116; from the per-step dump of opendining-4 manuk, steps 0-13, and the transcript)

- opendining-4 #13 #14 (manuk unresolved): manuk's comboboxes are all present through step #6 (4 comboboxes, 0 options). Replay step #7 is a `click` on the named combobox "Select time" (`act h7 click` in the transcript). On manuk that click opens the page's ARIA dropdown: the observation after it has 0 comboboxes, 2 buttons and 48 `option`s, so nameless-combobox ordinals 1 and 3 have nothing to resolve to. On Chrome the same click leaves the page unchanged (4 comboboxes, 58 buttons, 0 options). In the original recorded run the same click returned "no change ... 0 click listeners" and the page kept its comboboxes. So the miss is not an ordinal-counting error: manuk's page state after the step-#7 click differs from both Chrome's and the recording's.
- gocalendar-2 #6: both backends resolve ordinal button n=47 to a different button (manuk: 51 buttons before the step, chrome: 92; transcript line `button [clickable] ↑offscreen`). Chrome also mismatches here (12/13), so a button ordinal in a calendar grid is not comparable across backends; not a manuk-only miss.
- gocalendar-2 #7 (manuk unresolved): follows #6. The manuk #6 click landed on an event button, after which the tree has 8 buttons, 1 link, 0 comboboxes (an event view replaced the grid); Chrome's #6 click landed elsewhere and kept its combobox. A consequence of the #6 mismatch, not an independent miss.
- Gate (manuk >= 11/13) NOT met: 9/13 stays. Reaching it needs the opendining-4 click-opens-dropdown difference explained first (follow-up rows).
