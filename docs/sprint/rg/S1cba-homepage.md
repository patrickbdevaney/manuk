# RG.S1.c.b.a: OpenDining home page, restaurant cards in Chrome vs manuk (chrome_ver=145.0.7632.116)

Raw: `docs/sprint/rg/S1cba-homepage.tsv` (one row per surface, chrome_ver on every row).
Measure: `MANUK_CHROME_PIN=145.0.7632.116 python3 -I docs/sprint/rg/S1cba-measure.py docs/sprint/rg/S1cba-homepage.tsv`
Gate: `python3 -I docs/sprint/rg/S1cba-gate.py docs/sprint/rg/S1cba-homepage.tsv 145.0.7632.116` → **GATE RED**, exit 1.
Chrome side: `docs/sprint/rg/S1cba-chrome-home.js` (Playwright DOM on the pinned CfT 145, 6 s settle).

| surface | Chrome cards | manuk cards | verdict |
|---|---|---|---|
| extract text, default (the harness's manuk read) | 20 | 6 | **RED** (gated) |
| extract text, include_hidden=true | 20 | 20 | ok (gated) |
| extract markdown, default | 20 | 20 | ok (info) |
| extract markdown, include_hidden=true | 20 | 20 | ok (info) |
| observe full, default (what navigate shows first) | 20 | 5 | short (info) |
| observe full, include_hidden=true | 20 | 5 | short (info) |

## What the page holds
- Chrome's DOM has 60 "View details" links = **20 distinct restaurants**, each repeated three times (react-multi-carousel).
- The five cards at the start of the carousel are the visible ones. The other 15 sit in an `overflow: hidden` track
  (`DIV.react-multi-carousel-list`) to the right of the viewport (e.g. Bar Central at x=1501 with a 1280 px viewport).
- So "the home page exposes only five restaurant cards" is **false in Chrome**: it exposes 20, and a human reaches the
  other 15 by sliding the carousel ("Next slide").

## Engine, per surface
- The **extract** tool is faithful. With include_hidden=true, text and markdown both return all 20 names.
- The default text extract withholds the 14 offscreen carousel cards: the shield reports
  `hidden-text: 1005 fragments withheld (offscreen 1003, opacity 2)`. The default therefore returns 6 names
  (the five visible cards plus Bar Central, which is partly in the list). This is the gated RED.
- **observe** (the first thing navigate returns) lists only the five visible cards plus a "Next slide" button, with
  or without include_hidden. The 15 clipped carousel cards are absent from the observation tree altogether, not just
  withheld. Chrome's DOM holds them (see above), so this is an omission in observe, not a withholding.

## Link to the failed opendining-8 runs
- The runs that failed (1 and 3 in S1cb-classify.md) booked The Royal Dine from the five cards in the first observation,
  without clicking "Let's go". The engine did give those agents the observation that showed 5 cards. Run 2, which
  clicked through, found Bar Central.
- So the failure is an engine gap (observe omits clipped carousel items; default text withholds them) that the agent
  cannot see past without a hint. It is not only agent choice. The text in the shield warning does not say that the
  withheld fragments are restaurant cards.

## Not claimed
- No engine change in this node. The shield's offscreen rule is a deliberate hidden-instruction defence; whether
  human-reachable carousel items should count as visible is a policy decision for a follow-up gated on the injection slice.
- No task edits. The opendining-8 task and its evals are unchanged.
