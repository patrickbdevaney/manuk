# RG.3.0.b.a.a.b.a: the DOM node behind Chrome's `button "close"`

chrome_ver=145.0.7632.116. DOM dump 3,12 s after navigate on each backend (`bench/agent-tasks/rg3ba_close_dom.py`, JS `rg3ba_close.js`; manuk = `dom-eval`, in-engine JS). Columns: the close button (tag, class, aria-label, computed style, rect), its parent, page facts, number of matches.

| site | backend | close button | parent | page | hits |
|---|---|---|---|---|---|
| zilloft @3s | chrome | `<button class="MuiButtonBase-root MuiIconButton-root Mu" aria-label="close">` display=flex visibility=visible opacity=1 rect=[973, 8, 28, 28] | `<div class="MuiAlert-action css-1mzcepu" aria-label="">` display=flex visibility=visible opacity=1 rect=[841, 184, 16, 4] | 923 elements; banner text in body: True; role=alert nodes: 2 | 1 |
| zilloft @3s | manuk | NO close button in DOM (911 elements; banner text in body: False; role=alert nodes: 1) | - | - | - |
| zilloft @12s | chrome | NO close button in DOM (911 elements; banner text in body: False; role=alert nodes: 1) | - | - | - |
| zilloft @12s | manuk | NO close button in DOM (911 elements; banner text in body: False; role=alert nodes: 1) | - | - | - |
| udriver @3s | chrome | `<button class="MuiButtonBase-root MuiIconButton-root Mu" aria-label="close">` display=flex visibility=visible opacity=1 rect=[973, 8, 28, 28] | `<div class="MuiAlert-action css-1mzcepu" aria-label="">` display=flex visibility=visible opacity=1 rect=[841, 184, 16, 4] | 138 elements; banner text in body: True; role=alert nodes: 2 | 1 |
| udriver @3s | manuk | NO close button in DOM (125 elements; banner text in body: False; role=alert nodes: 1) | - | - | - |
| udriver @12s | chrome | NO close button in DOM (129 elements; banner text in body: False; role=alert nodes: 1) | - | - | - |
| udriver @12s | manuk | NO close button in DOM (125 elements; banner text in body: False; role=alert nodes: 1) | - | - | - |
| gocalendar @3s | chrome | `<button class="MuiButtonBase-root MuiIconButton-root Mu" aria-label="close">` display=flex visibility=visible opacity=1 rect=[973, 8, 28, 28] | `<div class="MuiAlert-action css-1xfslal" aria-label="">` display=flex visibility=visible opacity=1 rect=[841, 184, 16, 4] | 1029 elements; banner text in body: True; role=alert nodes: 1 | 1 |
| gocalendar @3s | manuk | NO close button in DOM (1024 elements; banner text in body: False; role=alert nodes: 0) | - | - | - |
| gocalendar @12s | chrome | NO close button in DOM (1024 elements; banner text in body: False; role=alert nodes: 0) | - | - | - |
| gocalendar @12s | manuk | NO close button in DOM (1024 elements; banner text in body: False; role=alert nodes: 0) | - | - | - |
| gomail @3s | chrome | NO close button in DOM (61 elements; banner text in body: False; role=alert nodes: 1) | - | - | - |
| gomail @3s | manuk | NO close button in DOM (1024 elements; banner text in body: False; role=alert nodes: 1) | - | - | - |
| gomail @12s | chrome | `<button class="MuiButtonBase-root MuiIconButton-root Mu" aria-label="close">` display=flex visibility=visible opacity=1 rect=[973, 8, 28, 28] | `<div class="MuiAlert-action css-1mzcepu" aria-label="">` display=flex visibility=visible opacity=1 rect=[841, 184, 16, 4] | 1036 elements; banner text in body: True; role=alert nodes: 2 | 1 |
| gomail @12s | manuk | NO close button in DOM (1024 elements; banner text in body: False; role=alert nodes: 1) | - | - | - |
