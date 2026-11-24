# PERF.1.2.b.b: dynamic-scripts settle split

chrome_ver=145.0.7632.116 · rows from `docs/sprint/perf1-dyn/runs.jsonl`. Each phase is split into eval (JS, `dynscript_eval`), fetch_wait (`dynscript_wait`), restyle (cascade+layout events, nested in neither) and other (remainder). Median seconds across runs.

| URL | runs | phase | eval | fetch_wait | restyle | other | min named |
|---|---|---|---|---|---|---|---|
| https://comix.to/ | 3 | 0.012 | 0.000 | 0.003 | 0.000 | 0.010 | 0.185 |
| https://client.dkitrxmdwoqruvsi.net/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.370 |
| https://www.mobile.bg/ | 3 | 1.796 | 0.217 | 1.437 | 0.075 | 0.010 | 0.992 |
| https://app.ordertime.com/ | 3 | 1.002 | 0.465 | 0.558 | 0.010 | 0.003 | 0.996 |
| https://www.puentedemando.com/ | 3 | 1.550 | 0.438 | 1.099 | 0.000 | 0.021 | 0.986 |
| https://basescan.org/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.167 |
| https://ebanking.bdm-sa.com/ | 3 | 0.008 | 0.000 | 0.001 | 0.000 | 0.007 | 0.129 |
| https://admin.zoomph.com/ | 3 | 0.002 | 0.000 | 0.001 | 0.000 | 0.002 | 0.222 |
| https://books.toscrape.com/ | 3 | 0.425 | 0.075 | 0.228 | 0.146 | 0.004 | 0.968 |
| https://quotes.toscrape.com/ | 3 | 0.001 | 0.000 | 0.001 | 0.000 | 0.000 | 0.855 |
| https://quotes.toscrape.com/js/ | 3 | 0.002 | 0.000 | 0.001 | 0.000 | 0.001 | 0.315 |
| https://quotes.toscrape.com/scroll | 3 | 0.001 | 0.000 | 0.000 | 0.000 | 0.001 | 0.143 |
| https://example.com/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.134 |
| https://httpbin.org/html | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.225 |
| https://en.wikipedia.org/wiki/Python_(programming_language) | 3 | 2.850 | 1.025 | 0.511 | 1.301 | 0.070 | 0.975 |
| https://en.wikipedia.org/wiki/Rust_(programming_language) | 3 | 3.434 | 1.049 | 0.625 | 1.685 | 0.074 | 0.971 |
| https://en.wikipedia.org/wiki/Alan_Turing | 3 | 3.587 | 1.063 | 0.538 | 1.811 | 0.067 | 0.979 |
| https://en.wikipedia.org/wiki/Main_Page | 3 | 0.608 | 0.181 | 0.222 | 0.198 | 0.011 | 0.980 |
| https://www.scrapethissite.com/pages/simple/ | 3 | 1.016 | 0.207 | 0.776 | 0.000 | 0.034 | 0.961 |
| https://books.toscrape.com/catalogue/page-2.html | 3 | 0.374 | 0.088 | 0.099 | 0.172 | 0.007 | 0.979 |

Σ medians over 20 URLs: phase 16.67 s · eval 4.81 · fetch_wait 6.10 · restyle 5.40 · other 0.32

**SUM CHECK: GREEN** (named parts >= 95% of the phase on every run whose phase is >= 50 ms)

- excluded (unreachable from baseline, PERF.md): 6 run(s), ['https://mkp.gem.gov.in/', 'https://virtual.lapampa.edu.ar/']
- exempt (phase < 50 ms, nothing to split): 29 run(s)
  - ('https://comix.to/', 2, 0.0111)
  - ('https://comix.to/', 3, 0.0124)
  - ('https://client.dkitrxmdwoqruvsi.net/', 1, 0.0)
  - ('https://client.dkitrxmdwoqruvsi.net/', 2, 0.0)
  - ('https://client.dkitrxmdwoqruvsi.net/', 3, 0.0001)
  - ('https://basescan.org/', 1, 0.0)
  - ('https://basescan.org/', 2, 0.0001)
  - ('https://basescan.org/', 3, 0.0)
  - ('https://ebanking.bdm-sa.com/', 1, 0.0085)
  - ('https://ebanking.bdm-sa.com/', 2, 0.0075)
  - ('https://ebanking.bdm-sa.com/', 3, 0.01)
  - ('https://admin.zoomph.com/', 1, 0.0034)
  - ('https://admin.zoomph.com/', 2, 0.0024)
  - ('https://admin.zoomph.com/', 3, 0.0022)
  - ('https://quotes.toscrape.com/', 1, 0.0008)
  - ('https://quotes.toscrape.com/', 2, 0.0012)
  - ('https://quotes.toscrape.com/', 3, 0.0006)
  - ('https://quotes.toscrape.com/js/', 1, 0.0018)
  - ('https://quotes.toscrape.com/js/', 2, 0.0012)
  - ('https://quotes.toscrape.com/js/', 3, 0.0028)
  - ('https://quotes.toscrape.com/scroll', 1, 0.001)
  - ('https://quotes.toscrape.com/scroll', 2, 0.0014)
  - ('https://quotes.toscrape.com/scroll', 3, 0.0021)
  - ('https://example.com/', 1, 0.0)
  - ('https://example.com/', 2, 0.0)
  - ('https://example.com/', 3, 0.0001)
  - ('https://httpbin.org/html', 1, 0.0)
  - ('https://httpbin.org/html', 2, 0.0)
  - ('https://httpbin.org/html', 3, 0.0001)
