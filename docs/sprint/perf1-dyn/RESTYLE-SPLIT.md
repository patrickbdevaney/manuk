# PERF.1.2.b.b.b: restyle share of the dynamic-scripts phase, by location

chrome_ver=145.0.7632.116 · recomputed from the raw traces in `docs/sprint/perf1-dyn/traces` for the runs in `docs/sprint/perf1-dyn/runs.jsonl`. restyle_eval = cascade+layout inside a `dynscript_eval` span; restyle_between = the same work in the phase outside every eval span (between-round apply_stylesheets). eval+between must equal the stored restyle column within 1 ms on every run. Median over runs; forced = `reflow forced` events (in-eval / between).

| URL | runs | phase | restyle | restyle_eval | restyle_between | forced reflows (eval/between) | overlap |
|---|---|---|---|---|---|---|---|
| https://comix.to/ | 3 | 0.012 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://client.dkitrxmdwoqruvsi.net/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://www.mobile.bg/ | 3 | 1.796 | 0.075 | 0.075 | 0.000 | 1 / 0 | 0.000 |
| https://app.ordertime.com/ | 3 | 1.002 | 0.010 | 0.010 | 0.000 | 1 / 0 | 0.000 |
| https://www.puentedemando.com/ | 3 | 1.550 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://basescan.org/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://ebanking.bdm-sa.com/ | 3 | 0.008 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://admin.zoomph.com/ | 3 | 0.002 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://books.toscrape.com/ | 3 | 0.425 | 0.146 | 0.146 | 0.000 | 6 / 0 | 0.000 |
| https://quotes.toscrape.com/ | 3 | 0.001 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://quotes.toscrape.com/js/ | 3 | 0.002 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://quotes.toscrape.com/scroll | 3 | 0.001 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://example.com/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://httpbin.org/html | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://en.wikipedia.org/wiki/Python_(programming_language) | 3 | 2.850 | 1.301 | 1.301 | 0.000 | 3 / 0 | 0.000 |
| https://en.wikipedia.org/wiki/Rust_(programming_language) | 3 | 3.434 | 1.685 | 1.685 | 0.000 | 3 / 0 | 0.000 |
| https://en.wikipedia.org/wiki/Alan_Turing | 3 | 3.587 | 1.811 | 1.811 | 0.000 | 3 / 0 | 0.000 |
| https://en.wikipedia.org/wiki/Main_Page | 3 | 0.608 | 0.198 | 0.198 | 0.000 | 1 / 0 | 0.000 |
| https://www.scrapethissite.com/pages/simple/ | 3 | 1.016 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://books.toscrape.com/catalogue/page-2.html | 3 | 0.374 | 0.172 | 0.172 | 0.000 | 6 / 0 | 0.000 |

Σ medians over 20 URLs: restyle 5.40 s · restyle_eval 5.40 · restyle_between 0.00 · forced reflows 24

**RESTYLE SPLIT CHECK: GREEN** (eval+between = stored restyle within 1 ms on every OK run)

