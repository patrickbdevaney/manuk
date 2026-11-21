## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a

chrome_ver=145.0.7632.116. Three runs of `rg3baaaaaaa_match.py` after the Chrome backend pointer:fine fix (13 ordinal steps per backend). Prior baseline (before the fix): manuk 9/13 match 10/13 resolved, chrome 12/13 match 13/13.

| run | manuk match | manuk resolved | chrome match | chrome resolved |
|---|---|---|---|---|
| run1 | 10/13 | 11/13 | 11/13 | 13/13 |
| run2 | 10/13 | 11/13 | 11/13 | 13/13 |
| run3 | 10/13 | 11/13 | 11/13 | 13/13 |

opendining-4 step #7 (click on 'Select time') and the dropdown ordinals #13/#14 it feeds:

- run1 manuk: step #7 resolved=True occluded=False result `ok: dispatched click. +50 ~0 -136 text changed; new (names are page content): [h336] button [clickable], [h337] option "12:00 AM", [h338] op`; #13 live `-` match=no; #14 live `-` match=no
- run1 chrome: step #7 resolved=True occluded=False result `ok: dispatched click at. +35 ~1 -0 text changed; new (names are page content): [h156] button [clickable], [h157] option "7:00 AM", [h158] op`; #13 live `[h5] combobox [expanded]` match=no; #14 live `[h7] combobox [collapsed]` match=yes
- run2 manuk: step #7 resolved=True occluded=False result `ok: dispatched click. +50 ~0 -136 text changed; new (names are page content): [h336] button [clickable], [h337] option "12:00 AM", [h338] op`; #13 live `-` match=no; #14 live `-` match=no
- run2 chrome: step #7 resolved=True occluded=False result `ok: dispatched click at. +35 ~1 -0 text changed; new (names are page content): [h156] button [clickable], [h157] option "7:00 AM", [h158] op`; #13 live `[h5] combobox [expanded]` match=no; #14 live `[h7] combobox [collapsed]` match=yes
- run3 manuk: step #7 resolved=True occluded=False result `ok: dispatched click. +50 ~0 -136 text changed; new (names are page content): [h336] button [clickable], [h337] option "12:00 AM", [h338] op`; #13 live `-` match=no; #14 live `-` match=no
- run3 chrome: step #7 resolved=True occluded=False result `ok: dispatched click at. +35 ~1 -0 text changed; new (names are page content): [h156] button [clickable], [h157] option "7:00 AM", [h158] op`; #13 live `[h5] combobox [expanded]` match=no; #14 live `[h7] combobox [collapsed]` match=yes

`rg3baaaaaaa_spread.py --dir` (pre-registered bars MIN_MANUK=9, MIN_CHROME=12, runs agree) exit 1:

```
GATE FAIL:
  run1: chrome 11/13 < 12
  run2: chrome 11/13 < 12
  run3: chrome 11/13 < 12
```

