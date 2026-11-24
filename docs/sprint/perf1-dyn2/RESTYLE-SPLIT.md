# PERF.1.2.b.b.b: restyle share of the dynamic-scripts phase, by location

chrome_ver=145.0.7632.116 · recomputed from the raw traces in `docs/sprint/perf1-dyn2/traces` for the runs in `docs/sprint/perf1-dyn2/runs.jsonl`. restyle_eval = cascade+layout inside a `dynscript_eval` span; restyle_between = the same work in the phase outside every eval span (between-round apply_stylesheets). eval+between must equal the stored restyle column within 1 ms on every run. Median over runs; forced = `reflow forced` events (in-eval / between).

| URL | runs | phase | restyle | restyle_eval | restyle_between | forced reflows (eval/between) | overlap |
|---|---|---|---|---|---|---|---|
| https://comix.to/ | 3 | 0.012 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://client.dkitrxmdwoqruvsi.net/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://www.mobile.bg/ | 3 | 1.817 | 0.056 | 0.056 | 0.000 | 1 / 0 | 0.000 |
| https://app.ordertime.com/ | 3 | 0.975 | 0.010 | 0.010 | 0.000 | 1 / 0 | 0.000 |
| https://www.puentedemando.com/ | 3 | 2.412 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://basescan.org/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://ebanking.bdm-sa.com/ | 3 | 0.008 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://admin.zoomph.com/ | 3 | 0.001 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://books.toscrape.com/ | 3 | 0.213 | 0.081 | 0.081 | 0.000 | 6 / 0 | 0.000 |
| https://quotes.toscrape.com/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://quotes.toscrape.com/js/ | 3 | 0.002 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://quotes.toscrape.com/scroll | 3 | 0.001 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://example.com/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://httpbin.org/html | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://en.wikipedia.org/wiki/Python_(programming_language) | 3 | 2.401 | 1.050 | 1.050 | 0.000 | 3 / 0 | 0.000 |
| https://en.wikipedia.org/wiki/Rust_(programming_language) | 3 | 2.755 | 1.008 | 1.008 | 0.000 | 3 / 0 | 0.000 |
| https://en.wikipedia.org/wiki/Alan_Turing | 3 | 1.850 | 0.731 | 0.731 | 0.000 | 3 / 0 | 0.000 |
| https://en.wikipedia.org/wiki/Main_Page | 3 | 0.589 | 0.106 | 0.106 | 0.000 | 1 / 0 | 0.000 |
| https://www.scrapethissite.com/pages/simple/ | 3 | 0.873 | 0.000 | 0.000 | 0.000 | 0 / 0 | 0.000 |
| https://books.toscrape.com/catalogue/page-2.html | 3 | 0.256 | 0.106 | 0.106 | 0.000 | 6 / 0 | 0.000 |

Σ medians over 20 URLs: restyle 3.15 s · restyle_eval 3.15 · restyle_between 0.00 · forced reflows 24

**RESTYLE SPLIT CHECK: GREEN** (eval+between = stored restyle within 1 ms on every OK run)


## PERF.1.2.b.b.b.a: before/after (same chrome_ver=145.0.7632.116; baseline = docs/sprint/perf1-dyn)

restyle_eval, Σ median over Wikipedia Python+Rust+Alan_Turing: **4.797 s -> 2.789 s (-41.9%)**; restyle_between 0.00 s both. JS eval (same 3 URLs) 3.137 s -> 2.452 s. `MANUK_INCR_VERIFY=1` over the 22 sweep URLs: 0 INCR MISMATCH. Different sweeps, different host load: the paired same-binary arm (`MANUK_INCR_NOPRUNE=1`) on Alan_Turing, forced cascades 731/87/62 ms -> 276/60/39 ms.
