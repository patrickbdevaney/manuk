# RG.S1.a.a.a.a: zilloft-2 location is not lost — the site's own search is token-OR over the whole catalogue

chrome_ver=145.0.7632.116. Source: the page's own bundle (`/_next/static/chunks/pages/_app-*.js`, `SearchProvider.n()`), 522 embedded listings,
run offline (no browser): the query is lower-cased and split on " "; a listing matches if its address/city/state/zip text contains ANY token
(`p.some(t=>e.includes(t))`); then the for-sale, beds, baths (exact or >=) filters apply client-side. No location param, no fetch, no URL query.

| typed query | for-sale matches | exact 3bd/2ba | 3+bd/2+ba |
|---|---|---|---|
| `San Francisco` | 56 | **16** | 36 |
| `San Francisco, CA` (token `ca` matches every listing) | 522 | **147** | 387 |

So the observed manuk results are what Chrome shows for the same typed text: run 3 (`San Francisco`) = 16 results = the task's expected value
(its answer was hedged, which the judge scored 0); runs 1-2 typed `San Francisco, CA` = 147, the correct count for that query.
There is no engine defect to fix here; the 0/3 is agent query choice + answer hedging. Substring matching also explains Santa Monica / San Jose / San Diego rows.
