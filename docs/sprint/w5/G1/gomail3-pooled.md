# W5.G1.b.a.a.a.a: cdp-layer pre-77b97d40 vs HEAD v2/v3 at one pin (GATE PASS)

chrome_ver=145.0.7632.116. Set A = `cdp-layer-gomail3-repeat-pre77.tsv,cdp-layer-gomail3-x9-pre77.tsv,cdp-layer-pre77.tsv`, set B = `cdp-layer-gomail3-repeat-head.tsv,cdp-layer-gomail3-x9-head.tsv,cdp-layer-v2.tsv,cdp-layer-v3.tsv` (pooled per side).

| task | v2 passes | v3 passes | abs(v2-v3) | v2 runs | v3 runs |
|---|---|---|---|---|---|
| gomail-3 | 0/15 | 8/18 | 8 | 15 | 18 |
| **total** | 0/15 | 8/18 | 8 | | |

Band (cdp-layer-v2.tsv 3/3 vs cdp-layer-v3.tsv 2/3): 1 runs. The --v2 total 0/15 is OUTSIDE the band (outside = more than 1 runs from both HEAD totals).
Gap gomail-3: |0 - 8| = 8 runs (min 3).
Build check (pre77-build-x9.tsv): ok, the two binaries differ and carry the right marker

Sources (pooled per side):
- v2 `cdp-layer-gomail3-repeat-pre77.tsv`: 3 rows, 0 passes, expects 3 per task
- v2 `cdp-layer-gomail3-x9-pre77.tsv`: 9 rows, 0 passes, expects 9 per task
- v2 `cdp-layer-pre77.tsv`: 3 rows, 0 passes, expects 3 per task
- v3 `cdp-layer-gomail3-repeat-head.tsv`: 3 rows, 0 passes, expects 3 per task
- v3 `cdp-layer-gomail3-x9-head.tsv`: 9 rows, 3 passes, expects 9 per task
- v3 `cdp-layer-v2.tsv`: 3 rows, 3 passes, expects 3 per task
- v3 `cdp-layer-v3.tsv`: 3 rows, 2 passes, expects 3 per task

No problems: every file has the expected rows per task, and no row is shared between files.
