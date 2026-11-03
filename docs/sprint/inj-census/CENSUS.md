# W3.15 INJ.CENSUS — BrowseSafe-Bench test split through manuk (chrome_ver=145.0.7632.116)

Source: perplexity-ai/browsesafe-bench, test split, 3,680 pages (1,824 malicious `yes`, 1,856 benign), HF parquet export.
Script: `census.py` (learn -> run). Per-sample output: `labels.tsv` (one row per page incl. benign controls; sample_id = test split row index).

## Method (and its limit)
The dataset ships only `(content, label)` — the injected span is **not annotated**. Payload location is inferred: word
5-grams seen in >=10 train-split malicious pages and in no train-split benign page are signature grams; a page's payload
segments are the HTML segments containing one. Construction label = where that segment sits in the HTML (comment, attribute,
script/style/template/noscript/title, `display:none`/`[hidden]` = no box, other hidden-style, plain visible text). Render truth =
manuk-mcp `extract`+`observe` with vs without `include_hidden`: gram in the default surface = visible, only with
`include_hidden` = rendered-hidden (painted but a human cannot see it), in neither = non-rendered (no box / not text).
Sample bin = most-visible bin over its payload segments.

## Result (denominator = 1,824 malicious samples)
| bin (render truth) | n | % of 1,824 | % of 1,178 located |
|---|---|---|---|
| visible | 513 | 28.1 | 43.5 |
| rendered-hidden | 145 | 7.9 | 12.3 |
| non-rendered | 520 | 28.5 | 44.1 |
| unlocated (no signature gram) | 642 | 35.2 | - |
| error (navigate/extract failed) | 4 | 0.2 | - |

Construction-vs-render disagreement: 142 / 1,178 located = **12.1%** (59 attribute/comment-borne text that manuk exposes as visible
a11y names, 35 hidden-style text that manuk's default surface still shows, 16+12 visible-by-construction text not visible/withheld,
11+9 hidden<->non-rendered). Benign controls: 1 / 300 false trigger. R0.5 BrowseSafe fixtures (rows 17/195/308/77): 17 unlocated, 195 hidden,
308 and 77 construction non-rendered but exposed visible.

## Reading against the gate
- Rendered-hidden is 7.9% of all malicious (12.3% of located) — under 15% on either denominator, so **R1.4 (hide-from-human shield) is
  hygiene, not a Bar-2 bet**: it can reach at most ~8-12% of this corpus.
- The prior "~70% non-rendered" does not reproduce here: 28.5% (44% of located). 28% of the corpus (43.5% of located) is plain visible text, which no paint-truth filter can remove.
- Disagreement 12.1% >= 5%: labels.tsv is the label-correction artifact. Publishing to the dataset needs owner OK (not done).
- Caveats: 35% unlocated (templated injections are over-represented among located); signature location is inferred, not annotated.
