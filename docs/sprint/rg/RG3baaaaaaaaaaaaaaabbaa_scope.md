## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b.b.a.a.a

chrome_ver=145.0.7632.116. Combobox nodes in opendining-4's observation after 7 steps (Select-time dropdown closed) and after 8 (step #7 opened it):

| backend | dropdown | observe(interactive) | observe(full, include_hidden) |
|---|---|---|---|
| manuk | closed | 4 | 4 |
| manuk | open | 0 | 0 |
| chrome | closed | 4 | 4 |
| chrome | open | 4 | 4 |

Isolated rule: local page, `<div id=root [aria-hidden=true]>` with 2 buttons + a select, beside a `role=listbox` portal with 1 button; page buttons listed by observe(interactive) (portal's own button counts 1):

| backend | root | buttons listed |
|---|---|---|
| manuk | aria-hidden | 1 |
| manuk | control | 3 |
| chrome | aria-hidden | 3 |
| chrome | control | 3 |

RULE CONFIRMED: manuk's a11y builder prunes the aria-hidden app root that the page's modal manager sets while the dropdown is open; the CDP builder does not read aria-hidden.
