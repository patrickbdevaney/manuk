# PERF.1.2.b.b: dynamic-scripts settle split

chrome_ver=145.0.7632.116 · rows from `docs/sprint/perf1-dyn2/runs.jsonl`. Each phase is split into eval (JS, `dynscript_eval`), fetch_wait (`dynscript_wait`), restyle (cascade+layout events, nested in neither) and other (remainder). Median seconds across runs.

| URL | runs | phase | eval | fetch_wait | restyle | other | min named |
|---|---|---|---|---|---|---|---|
| https://comix.to/ | 3 | 0.012 | 0.000 | 0.002 | 0.000 | 0.010 | 0.178 |
| https://client.dkitrxmdwoqruvsi.net/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.393 |
| https://www.mobile.bg/ | 3 | 1.817 | 0.240 | 1.510 | 0.056 | 0.006 | 0.995 |
| https://app.ordertime.com/ | 3 | 0.975 | 0.452 | 0.503 | 0.010 | 0.003 | 0.996 |
| https://www.puentedemando.com/ | 3 | 2.412 | 0.490 | 1.858 | 0.000 | 0.022 | 0.989 |
| https://basescan.org/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.092 |
| https://ebanking.bdm-sa.com/ | 3 | 0.008 | 0.000 | 0.001 | 0.000 | 0.006 | 0.141 |
| https://admin.zoomph.com/ | 3 | 0.001 | 0.000 | 0.000 | 0.000 | 0.001 | 0.308 |
| https://books.toscrape.com/ | 3 | 0.213 | 0.049 | 0.074 | 0.081 | 0.003 | 0.983 |
| https://quotes.toscrape.com/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.849 |
| https://quotes.toscrape.com/js/ | 3 | 0.002 | 0.000 | 0.000 | 0.000 | 0.001 | 0.261 |
| https://quotes.toscrape.com/scroll | 3 | 0.001 | 0.000 | 0.000 | 0.000 | 0.001 | 0.062 |
| https://example.com/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.142 |
| https://httpbin.org/html | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.222 |
| https://en.wikipedia.org/wiki/Python_(programming_language) | 3 | 2.401 | 0.850 | 0.451 | 1.050 | 0.042 | 0.974 |
| https://en.wikipedia.org/wiki/Rust_(programming_language) | 3 | 2.755 | 0.931 | 0.599 | 1.008 | 0.047 | 0.980 |
| https://en.wikipedia.org/wiki/Alan_Turing | 3 | 1.850 | 0.671 | 0.486 | 0.731 | 0.049 | 0.972 |
| https://en.wikipedia.org/wiki/Main_Page | 3 | 0.589 | 0.243 | 0.245 | 0.106 | 0.010 | 0.983 |
| https://www.scrapethissite.com/pages/simple/ | 3 | 0.873 | 0.256 | 0.587 | 0.000 | 0.030 | 0.963 |
| https://books.toscrape.com/catalogue/page-2.html | 3 | 0.256 | 0.055 | 0.077 | 0.106 | 0.003 | 0.987 |

Σ medians over 20 URLs: phase 14.17 s · eval 4.24 · fetch_wait 6.40 · restyle 3.15 · other 0.23

**SUM CHECK: GREEN** (named parts >= 95% of the phase on every run whose phase is >= 50 ms)

- excluded (unreachable from baseline, PERF.md): 6 run(s), ['https://mkp.gem.gov.in/', 'https://virtual.lapampa.edu.ar/']
- exempt (phase < 50 ms, nothing to split): 29 run(s)
  - ('https://comix.to/', 2, 0.0088)
  - ('https://comix.to/', 3, 0.0121)
  - ('https://client.dkitrxmdwoqruvsi.net/', 1, 0.0)
  - ('https://client.dkitrxmdwoqruvsi.net/', 2, 0.0001)
  - ('https://client.dkitrxmdwoqruvsi.net/', 3, 0.0)
  - ('https://basescan.org/', 1, 0.0001)
  - ('https://basescan.org/', 2, 0.0001)
  - ('https://basescan.org/', 3, 0.0001)
  - ('https://ebanking.bdm-sa.com/', 1, 0.0076)
  - ('https://ebanking.bdm-sa.com/', 2, 0.0066)
  - ('https://ebanking.bdm-sa.com/', 3, 0.0099)
  - ('https://admin.zoomph.com/', 1, 0.0014)
  - ('https://admin.zoomph.com/', 2, 0.0012)
  - ('https://admin.zoomph.com/', 3, 0.0014)
  - ('https://quotes.toscrape.com/', 1, 0.0004)
  - ('https://quotes.toscrape.com/', 2, 0.0008)
  - ('https://quotes.toscrape.com/', 3, 0.0004)
  - ('https://quotes.toscrape.com/js/', 1, 0.0015)
  - ('https://quotes.toscrape.com/js/', 2, 0.0015)
  - ('https://quotes.toscrape.com/js/', 3, 0.0009)
  - ('https://quotes.toscrape.com/scroll', 1, 0.0009)
  - ('https://quotes.toscrape.com/scroll', 2, 0.001)
  - ('https://quotes.toscrape.com/scroll', 3, 0.0049)
  - ('https://example.com/', 1, 0.0001)
  - ('https://example.com/', 2, 0.0002)
  - ('https://example.com/', 3, 0.0001)
  - ('https://httpbin.org/html', 1, 0.0)
  - ('https://httpbin.org/html', 2, 0.0)
  - ('https://httpbin.org/html', 3, 0.0)
