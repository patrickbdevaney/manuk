# PERF.1.2.b.b: dynamic-scripts settle split

chrome_ver=145.0.7632.116 · rows from `docs/sprint/perf1-arrival/runs.jsonl`. Each phase is split into eval (JS, `dynscript_eval`), fetch_wait (`dynscript_wait`), restyle (cascade+layout events, nested in neither) and other (remainder). Median seconds across runs.

| URL | runs | phase | eval | fetch_wait | restyle | other | min named |
|---|---|---|---|---|---|---|---|
| https://comix.to/ | 3 | 0.006 | 0.000 | 0.001 | 0.000 | 0.005 | 0.219 |
| https://client.dkitrxmdwoqruvsi.net/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.340 |
| https://www.mobile.bg/ | 3 | 1.862 | 0.224 | 1.385 | 0.237 | 0.014 | 0.992 |
| https://app.ordertime.com/ | 3 | 1.265 | 0.428 | 0.687 | 0.060 | 0.003 | 0.997 |
| https://www.puentedemando.com/ | 3 | 2.890 | 0.478 | 2.342 | 0.000 | 0.023 | 0.991 |
| https://basescan.org/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.154 |
| https://ebanking.bdm-sa.com/ | 3 | 0.006 | 0.000 | 0.001 | 0.000 | 0.004 | 0.172 |
| https://admin.zoomph.com/ | 3 | 0.002 | 0.000 | 0.000 | 0.000 | 0.001 | 0.269 |
| https://books.toscrape.com/ | 3 | 0.425 | 0.064 | 0.097 | 0.244 | 0.005 | 0.987 |
| https://quotes.toscrape.com/ | 3 | 0.001 | 0.000 | 0.001 | 0.000 | 0.000 | 0.857 |
| https://quotes.toscrape.com/js/ | 3 | 0.001 | 0.000 | 0.000 | 0.000 | 0.001 | 0.304 |
| https://quotes.toscrape.com/scroll | 3 | 0.001 | 0.000 | 0.000 | 0.000 | 0.001 | 0.333 |
| https://example.com/ | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.231 |
| https://httpbin.org/html | 3 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.259 |
| https://en.wikipedia.org/wiki/Python_(programming_language) | 3 | 6.163 | 0.999 | 0.055 | 5.006 | 0.052 | 0.991 |
| https://en.wikipedia.org/wiki/Rust_(programming_language) | 3 | 6.432 | 0.997 | 0.047 | 5.197 | 0.054 | 0.991 |
| https://en.wikipedia.org/wiki/Alan_Turing | 3 | 6.738 | 1.067 | 0.055 | 5.580 | 0.060 | 0.990 |
| https://en.wikipedia.org/wiki/Main_Page | 3 | 1.210 | 0.188 | 0.568 | 0.420 | 0.025 | 0.976 |
| https://www.scrapethissite.com/pages/simple/ | 3 | 1.031 | 0.222 | 0.731 | 0.000 | 0.046 | 0.948 |
| https://books.toscrape.com/catalogue/page-2.html | 3 | 0.407 | 0.063 | 0.100 | 0.238 | 0.004 | 0.987 |

Σ medians over 20 URLs: phase 28.44 s · eval 4.73 · fetch_wait 6.07 · restyle 16.98 · other 0.30

**SUM CHECK: RED** (named parts >= 95% of the phase on every run whose phase is >= 50 ms)

- not explained: ('https://www.scrapethissite.com/pages/simple/', 1, 'named 0.948 < 0.95')
- excluded (unreachable from baseline, PERF.md): 6 run(s), ['https://mkp.gem.gov.in/', 'https://virtual.lapampa.edu.ar/']
- exempt (phase < 50 ms, nothing to split): 29 run(s)
  - ('https://comix.to/', 2, 0.0048)
  - ('https://comix.to/', 3, 0.006)
  - ('https://client.dkitrxmdwoqruvsi.net/', 1, 0.0)
  - ('https://client.dkitrxmdwoqruvsi.net/', 2, 0.0)
  - ('https://client.dkitrxmdwoqruvsi.net/', 3, 0.0001)
  - ('https://basescan.org/', 1, 0.0)
  - ('https://basescan.org/', 2, 0.0001)
  - ('https://basescan.org/', 3, 0.0)
  - ('https://ebanking.bdm-sa.com/', 1, 0.0052)
  - ('https://ebanking.bdm-sa.com/', 2, 0.0056)
  - ('https://ebanking.bdm-sa.com/', 3, 0.0068)
  - ('https://admin.zoomph.com/', 1, 0.0013)
  - ('https://admin.zoomph.com/', 2, 0.0016)
  - ('https://admin.zoomph.com/', 3, 0.0016)
  - ('https://quotes.toscrape.com/', 1, 0.0007)
  - ('https://quotes.toscrape.com/', 2, 0.0008)
  - ('https://quotes.toscrape.com/', 3, 0.0005)
  - ('https://quotes.toscrape.com/js/', 1, 0.0015)
  - ('https://quotes.toscrape.com/js/', 2, 0.0009)
  - ('https://quotes.toscrape.com/js/', 3, 0.0011)
  - ('https://quotes.toscrape.com/scroll', 1, 0.0013)
  - ('https://quotes.toscrape.com/scroll', 2, 0.0011)
  - ('https://quotes.toscrape.com/scroll', 3, 0.0011)
  - ('https://example.com/', 1, 0.0)
  - ('https://example.com/', 2, 0.0)
  - ('https://example.com/', 3, 0.0)
  - ('https://httpbin.org/html', 1, 0.0)
  - ('https://httpbin.org/html', 2, 0.0)
  - ('https://httpbin.org/html', 3, 0.0)
