# R1.4.a — shield false-positive audit

`audit.py` checks every text run the injection shield withholds from `extract` and `observe`
(manuk-mcp, release build) against Chrome's own paint. Measurements: `chunks.tsv` (one row per
withheld run) and `pages.tsv` (one row per page). Both carry `chrome_ver`.

Run: `MANUK_CHROME_PIN=145.0.7632.116 /tmp/audit-venv/bin/python -I docs/sprint/shield-audit/audit.py`
(venv = `python3 -m venv` + `pip install playwright pillow psutil`). Add `--skip-live` for planted + local
fixtures only, `--mutate blind` to force every Chrome verdict to HIDDEN.

## Method

1. Withheld text = word diff of `extract(format=text)` and `observe(mode=full)` with and without
   `include_hidden=true`. The shield only removes words, so every inserted word is withheld text.
   `⚠hidden-text` marker lines and `[hN]` handles are stripped first.
2. Each run is matched in Chrome's DOM (whitespace-insensitive, across text nodes; `observe` names are
   cleaned of role words and `↑/↓offscreen` tokens first). `chrome_matches` records how many places
   match; more than 1 means the verdict is ambiguous.
3. For the matched text, the element carrying it is scrolled into view and the glyph box is screenshotted
   (viewport, 1280x800), then the element is set to `visibility:hidden` and screenshotted again. At least
   3 pixels changing by luma >= 24 means **VISIBLE** (a human sees it). No change means **HIDDEN_CONFIRMED**.
   Boxes outside the viewport or zero-sized are `UNVERIFIABLE_*`. No match is `UNMATCHED`.

Sample: 37 pages = 1 planted page (grey-on-tint false-positive plus an opacity:0 control), 14 local fixture
pages (the 15 F-tasks are 14 unique URLs, since F01 and F02 share `/login`), 12 live suite pages (the 25 L-tasks
are 12 unique start URLs), and the first 10 URLs of `docs/bench/oracle-corpus-reachable.txt`. Source of the task list: `bench/agent-tasks/suite-v1.json`.

## Results (chrome_ver=145.0.7632.116)

| group | pages | withheld marker fragments | runs verified VISIBLE | HIDDEN_CONFIRMED | UNMATCHED | UNVERIFIABLE |
|---|---|---|---|---|---|---|
| planted | 1 | 21 | 1 (the planted FP) | 1 (control) | 0 | 0 |
| fixture | 14 | 47 | 0 | 2 | 0 | 1 |
| task-live | 12 | 1481 | 41 (incl. observe duplicates) | 4 | 7 | 3 |
| corpus | 10 | 66 | 4 (incl. ambiguous) | 5 | 5 | 5 |

Planted gate: the grey-on-tint false positive (`#bbb` on `#ddd`, contrast 1.42 < 1.5) is withheld and
VISIBLE; the opacity:0 control is HIDDEN_CONFIRMED. The gate is green.

Visible-but-withheld text on task-relevant pages (reliable matches, `chrome_matches=1`):

- **quotes.toscrape.com, /, /js/, /scroll** (tasks L08 to L13): tag pills (`love`, `life`, `inspirational`,
  `misattributed-eleanor-roosevelt`, ...) are withheld as `low-contrast`. The text is white (`rgb(255,255,255)`)
  on the `<a>`'s own background `rgb(124,163,230)`, a contrast of about 2.55, which the threshold of 1.5 does
  not withhold. Chrome's computed styles confirm this (probe in this session). Hypothesis: the shield's
  `Ctx.bg` chain (`agent/src/shield.rs`, `walk`) only picks up backgrounds from block-level `LayoutBox`es, not
  inline `<a>` backgrounds, so white text reads as white-on-white. Not yet confirmed in the layout code.
- **en.wikipedia.org/wiki/Alan_Turing** (tasks L20): the 87-word quotation paragraph ("I am sure I could not have
  found anywhere another companion ...") is withheld, while Chrome shows it at normal colour
  (`rgb(32,33,34)`, opacity 1, all ancestors non-zero size). The reason is not isolated: the page-level marker
  reports `opacity` and `zero-size` only. Chrome's ancestor sizes do not explain it, so the cause is probably in
  manuk's own layout boxes.
- **Wikipedia footer** (`Privacy policy About Wikipedia Disclaimers ...`): observe-level rows are withheld and
  VISIBLE after scroll. The matching extract-level rows are still UNMATCHED (see limitations).

Excluded from the reliable count:
- `fasterthanli.me` `.`: a one-character run matching 414 places. The verdict is meaningless (ambiguous).
- `a11yproject.com` `link "Posts" ...`: 2 matches, ambiguous.

The shield withholds **a lot** on these live pages: 1481 marker fragments across 12 suite pages, mostly
Wikipedia `zero-size` (298 on Python, 262 on Rust, 462 on Turing) and `opacity` (sr-only and hidden chrome,
probably correct). Most of those were not checked for visibility because they are UNMATCHED or
out-of-viewport.

## Verdict on the node

The node's gate is "a table of withheld-but-visible fragments per page, planted FP shows in the table". Both
hold: `pages.tsv` and `chunks.tsv` are the table, and the planted FP is in it. The node's **target** ("0 on
task-relevant text") is **not met**: the quotes tag pills and the Alan Turing paragraph are task-relevant
false positives. Those become follow-up rows R1.4.a.a and R1.4.a.b, not a reason to reopen this node.

## Limitations

- Matching is first-occurrence over DOM text; `chrome_matches` flags ambiguous rows, which are not verified.
- Shadow DOM and iframes are not searched, so those withheld runs are UNMATCHED.
- The visibility screenshot compares only the glyph box. A visible element overlapping the box in the same
  pixels would count as VISIBLE (a possible over-count, not under-count).
- Chunk-level reasons are not in the MCP output (only page-level counts). Per-run reasons need a shield
  debug output (follow-up R1.4.a.c).
- The observe diff repeats text already in the extract diff, so the observe VISIBLE rows double-count.
- `marker_withheld` counts text fragments (a word or span each), not runs. Planted: 21 fragments, 2 runs.
- Single run per page at a 1280x800 viewport; no scrolling beyond the element's own scrollIntoView.

## R1.4.a.b (chrome_ver=145.0.7632.116)

The Alan Turing quotation was withheld as `zero-size`: the layout box carrying it is a 0-wide, 1651px-tall
wrapper at (76, 3214) with `overflow: visible`, so its text still paints outside it. The shield now treats
a collapsed box as hiding text only when its element clips (`overflow` other than `visible`). Rerun: the
paragraph is no longer in the withheld set; suite pages report 0 visible-but-withheld runs.
