# RG.2.0.b.a.b — does Chrome's AX tree drop the controls behind a popover?

chrome_ver=145.0.7632.116 (pin `MANUK_CHROME_PIN`)

**Method.** `docs/sprint/rg/R20ba-chrome-ax.js` drives the pinned Chrome (CfT 145) through playwright-core (the
engine Playwright MCP uses; the MCP tool itself was not called) with the same click sequence as the
task: zilloft-5 = homepage → search "San Jose, CA" → results → Price button; gocalendar-2 = /calendar →
view selector "5 days" → Month → "+N more" cell. It reads CDP `Accessibility.getFullAXTree` before and
after each popover opens, and counts each named button/link/combobox/option that was present before and
is still present, not ignored, after. Three full passes; output in `R20ba-chrome-ax.json`; gate
`R20ba-chrome-ax-gate.py`.

**Result (identical in all 3 runs).**

| state (after the click) | named controls before | still in Chrome AX after | dropped | DOM: visible controls under an `aria-hidden=true` ancestor |
|---|---|---|---|---|
| zilloft-5 Price popover | 6 | 0 | 6 (Price, For Sale, Beds & Baths, Home Type, Save search, Go to homepage) | 228 of 229 |
| gocalendar-2 view popover ("5 days") | 36 | 0 | 36 (incl. Previous/Next month, Create, day cells) | 67 of 67 |
| gocalendar-2 "+N more" popover (Month) | 41 | 8 | 33 (day cells 01–31 and the "+N more" opener) | 0 of 45 |

**Per task, the stale control.**
- zilloft-5: the controls behind the Price popover (Home Type, Beds & Baths, For Sale) are **absent** from Chrome's AX tree.
- gocalendar-2: the controls behind the view popover and behind the "+N more" popover are **absent** from Chrome's AX tree.

**Reading.** Chrome's own accessibility tree drops the background controls, so a manuk refusal that the
handle is *behind an open layer* describes the page correctly; it is not a manuk-only artifact. The
zilloft and view-popover cases match the `aria-hidden` mechanism the fixture uses (RG.2.0.b.a). The
"+N more" case does **not** set `aria-hidden` on its siblings (0 of 45 behind it), so Chrome drops those
cells for some other reason that this measurement does not identify.

**manuk side (for comparison, not re-measured here).** `docs/sprint/rg/R20ba-real-manuk.tsv` (RG.2.0.b.a,
chrome_ver=145.0.7632.116): zilloft-5 now gets "hidden behind an open menu [h993]" in 11 STALEs; gocalendar-2
still gets bare STALE. So for zilloft-5 manuk already refuses in the way Chrome's tree implies; the open
gap is the gocalendar-2 role-less layer (RG.2.0.b.a.a).
