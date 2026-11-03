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

## W3.15.a recall attempt (chrome_ver=145.0.7632.116) — gate NOT met
Goal: unlocated <15% with benign false trigger <=2% on 300 controls. Unlocated at start: 642/1824 = 35.2%.
- Sample of unlocated pages: payload is a sentence embedded mid-segment (e.g. inside a long video transcript); no benign twin exists in the test split (best sentence overlap with any benign page is 1-4 sentences), so pairing cannot locate it.
- Signature-gram grid (df floor x min grams per segment; located % of 1824 / benign FP on 600 test benign): df10,k1 64.8/0.3 (current); df5,k2 64.0/1.8; df3,k3 65.9/8.8; df3,k1 84.2/43.3. No setting beats current at <=2% FP — the gram route is exhausted.
- Sentence-level naive Bayes (`locate_nb.py`, weak labels from df>=10 grams): threshold at 99th pct of 500 held-out train-benign pages recovers 21.3% of the 642 (137 pages) with 2.0% FP on 300 test benign (98th pct: 23.5% / 2.3%; 99.5th: 18.2% / 0.7%). Resulting unlocated ≈ 505/1824 = 27.7%, still >15%.
- Rendered-hidden share not re-stated: the 137 recovered pages are not rendered; hidden share stays 7.9% (12.3% of located); labels.tsv chrome_ver column is already 145.0.7632.116. R1.4 stays hygiene; unlocated 27.7% is the open uncertainty on that call.
