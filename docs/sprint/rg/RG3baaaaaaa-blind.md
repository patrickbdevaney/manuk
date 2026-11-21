## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b.b.a.a.a.a.a.a

chrome_ver=145.0.7632.116. opendining-4, replay settle 2 s, wait 2 s. Transcript steps #13/#14 act on combobox ordinal 0/1 ([h8]/[h10], both `[collapsed]`). Observation each was resolved against, per backend:

### manuk

- step #13: unresolved=True; comboboxes in observation: 0; role counts: {'button': 2, 'option': 48}; interactive lines: 50
  - preceding steps: ["#10 act {'handle': 'h11', 'action': 'click'}", "#11 act {'handle': 'h7', 'action': 'focus'}", "#12 act {'handle': 'h7', 'action': 'click'}"]
  - first lines: `['[h336] button [clickable] ↓offscreen', '[h337] option "12:00 AM"', '[h338] option "12:30 AM"']`
- step #14: unresolved=True; comboboxes in observation: 0; role counts: {'button': 2, 'option': 48}; interactive lines: 50
  - preceding steps: ["#11 act {'handle': 'h7', 'action': 'focus'}", "#12 act {'handle': 'h7', 'action': 'click'}", "#13 act {'handle': 'h8', 'action': 'select', 'value': '4:30 PM'}"]
  - first lines: `['[h336] button [clickable] ↓offscreen', '[h337] option "12:00 AM"', '[h338] option "12:30 AM"']`

### chrome

- step #13: unresolved=True; comboboxes in observation: 0; role counts: {'option': 48}; interactive lines: 48
  - preceding steps: ["#10 act {'handle': 'h11', 'action': 'click'}", "#11 act {'handle': 'h7', 'action': 'focus'}", "#12 act {'handle': 'h7', 'action': 'click'}"]
  - first lines: `['[h151] option "12:00 AM" ↑offscreen', '[h152] option "12:30 AM" ↑offscreen', '[h153] option "1:00 AM" ↑offscreen']`
- step #14: unresolved=True; comboboxes in observation: 0; role counts: {'option': 48}; interactive lines: 48
  - preceding steps: ["#11 act {'handle': 'h7', 'action': 'focus'}", "#12 act {'handle': 'h7', 'action': 'click'}", "#13 act {'handle': 'h8', 'action': 'select', 'value': '4:30 PM'}"]
  - first lines: `['[h151] option "12:00 AM" ↑offscreen', '[h152] option "12:30 AM" ↑offscreen', '[h153] option "1:00 AM" ↑offscreen']`


Reason: steps #11/#12 focus+click the Select-time combobox [h7]; the dropdown opens and the interactive observation then lists only the listbox (48 options, plus 2 buttons on manuk) and no comboboxes, on both backends. The transcript's [h8]/[h10] (collapsed comboboxes) are therefore absent from the observation the ordinal resolves against, so #13/#14 cannot be scored on either side; this is a shared blind spot of the replay, not a manuk-only divergence (see RG3baaaaaaa-match.md for the 11/13 resolved sets).

