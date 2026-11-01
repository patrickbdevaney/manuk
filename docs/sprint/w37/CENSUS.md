# W3.7 step 1 — how much of the interactive surface sits in an iframe

chrome_ver=145.0.7632.116 (all rows). Tool: `scripts/w37_census.py` (drives `manuk-mcp --backend chrome`, `navigate`,
3 s wait, `observe`, counts `iframe <origin>` regions and the interactive lines nested under them).
Rows: `CENSUS.tsv`; URL list: `census-urls.txt`.

**Corpus caveat.** No file in the tree names a "76-site corpus". This census ran over 105 URLs: the 25 live-site start URLs of the
40 R0.2 tasks (`bench/agent-tasks/suite-v1.json`) plus the 93 `docs/bench/oracle-corpus-reachable.txt` sites, de-duplicated. The 15
local-fixture R0.2 tasks contain no `<iframe>` (grep: 0 in `fixtures.py`). The census sees each site's landing page, not the
write flow behind it, so it bounds the *landing* surface only.

| measure | value |
|---|---|
| URLs observed, no error | 105 / 105 |
| pages with at least one iframe | 6 (5.7 %) |
| of those, with a cross-origin frame | 5 (the sixth, jsonformatter.org, has 2 same-origin frames) |
| interactive lines on all pages | 11381 |
| of those inside an iframe | 29 (0.25 %) |

Pages with frames: web.dev (196 cross-origin frames, 7 interactive lines in them), julialang.org (18 / 13), target.com (28 / 5),
w3.org (8 / 4), aljazeera.com (1 / 0), jsonformatter.org (2 same-origin / 0).

Reading: on landing pages a decisive control inside a frame is rare in this set. The case the node exists for, a payment card
field in a third-party frame, sits behind checkout flows this census does not reach; the fixture gate below covers that case.
