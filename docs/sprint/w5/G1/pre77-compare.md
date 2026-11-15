# W5.G1.b.a.a.a.a: cdp-layer pre-77b97d40 vs HEAD v2/v3 at one pin (GATE PASS)

chrome_ver=145.0.7632.116. Set A = `cdp-layer-pre77.tsv`, set B = `cdp-layer-v3.tsv`. Same 10 tasks x 3 runs.

| task | v2 passes | v3 passes | abs(v2-v3) | v2 runs | v3 runs |
|---|---|---|---|---|---|
| dashdish-10 | 3/3 | 3/3 | 0 | 3 | 3 |
| dashdish-7 | 2/3 | 2/3 | 0 | 3 | 3 |
| fly-unified-6 | 0/3 | 0/3 | 0 | 3 | 3 |
| gomail-3 | 0/3 | 2/3 | 2 | 3 | 3 |
| gomail-5 | 0/3 | 1/3 | 1 | 3 | 3 |
| gomail-8 | 1/3 | 1/3 | 0 | 3 | 3 |
| opendining-10 | 0/3 | 0/3 | 0 | 3 | 3 |
| opendining-4 | 2/3 | 2/3 | 0 | 3 | 3 |
| staynb-2 | 3/3 | 3/3 | 0 | 3 | 3 |
| staynb-4 | 1/3 | 2/3 | 1 | 3 | 3 |
| **total** | 12/30 | 16/30 | 4 | | |

Band (cdp-layer-v2.tsv 13/30 vs cdp-layer-v3.tsv 16/30): 3 runs. The --v2 total 12/30 is INSIDE the band (outside = more than 3 runs from both HEAD totals).
Build check (pre77-build.tsv): ok, the two binaries differ and carry the right marker

No problems: every task has 3 rows in each set.
