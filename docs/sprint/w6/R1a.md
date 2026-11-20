# W6.R1.a — what Chrome places above the udriver/staynb headers that manuk does not

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN), viewport 1024x768. Tools: `bench/agent-tasks/r1a_above.py` (Chrome DOM
+ rect dump over CDP), `target/release/dom-eval` (same JS in manuk), `bench/agent-tasks/r1a_account.py` (accounting +
gate). Raw: `docs/sprint/w6/raw/R1a/` (`*-chrome-dom.json`, `*-manuk-dom.json`, `account.txt`).

## Finding

Both sites render, in Chrome, a **MuiAlert banner** at the top of the page (a `div.MuiPaper-root.MuiAlert-root`
with `role=alert`, 13 elements: icon, svg, message with two `<br>`, a close button). Text: "Version 2.0 launches soon!
Stay tuned." plus a non-commercial-clone disclaimer. **manuk never mounts it**: its wrapper `div.MuiBox-root` is
empty (`<div class="MuiBox-root css-0"></div>`, height 0) and manuk's DOM has 0 Alert elements (`hasAlert:false`).

| site | Chrome alert root | alert height | first-content y (Chrome / manuk) | offset | explained by alert |
|---|---|---|---|---|---|
| udriver | y=0 | 204 px | 204 / 0 (header) | 204 | **100.0%** |
| staynb | y=0 | 250 px | 250 / 0 (search block) | 250 | **100.0%** |

The same single element class (the Alert subtree) explains the whole offset on both sites. This also explains the
unmatched Chrome `close` button in the R1 captures (the Alert's dismiss button; manuk has none).

On udriver 73/79 non-alert paired elements (92.4%) sit exactly 204.4 px lower in Chrome (the remainder are 3
elements at dy 0 and 3 sub-pixel/ripple cases). On staynb the document-order pairing is ambiguous (repeated card
keys), so it is not reported as a share; hand-aligned, the first content block is shifted by 250 and the search
form's children by ~247 (3 px of intra-block difference).

## Gate

`python3 bench/agent-tasks/r1a_account.py docs/sprint/w6/raw/R1a` -> **GREEN** (explained >=90% on both sites).
Mutant `--mutant-wrong-class` (explain by the Toolbar / search input instead of the Alert) -> **RED**, 31.3% and
25.2%, exit 1. Falsifier (offset is layout-wide, no single class >=90%): **not fired**.

## Not measured

- *Why* manuk does not mount the Alert (the page's JS condition: state, timer, storage, or fetch). Follow-up W6.R1.a.a.
- staynb below the first viewport: manuk's document is taller (21911 vs 15086 px in Chrome, which includes the 250 px
  alert), so a second, separate card-grid/flex difference exists there. Not accounted for here.
- Only udriver and staynb were dumped; dashdish (~33 px offset) was not.
