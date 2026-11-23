# W3.20.b.c.b.b.a: does native Chrome 150 define document.modelContext on coffee-shop, ticket-booking, smart-home?

chrome_ver=150.0.7871.124 (own measurement row; never diffed against 149.0.7827.155 or 145.0.7632.116).
Probe: `docs/sprint/w320b/panel/probe-register.mjs`, output `docs/sprint/w320b/panel/150/probe-register.tsv`.
No manuk bridge in the probe; each page gets load + 12 s settle, then reads `document.modelContext`,
`navigator.modelContext`, `navigator.modelContextTesting`, and their tool lists.

Command: `MANUK_CHROME=~/.local/chrome-cft/150.0.7871.124/chrome-linux64/chrome MANUK_CHROME_PIN=150.0.7871.124 node probe-register.mjs 150/probe-register.tsv coffee-shop ticket-booking smart-home`

## Measurement

| page | document.modelContext | navigator.modelContext | modelContextTesting | doc tools | nav tools | testing tools |
|---|---|---|---|---|---|---|
| coffee-shop | object | object | object | 4 | 4 | 4 |
| ticket-booking | object | object | object | 3 | 3 | 3 |
| smart-home | object | object | object | 6 | 6 | 6 |

Tool names match the manuk arm's names recorded in `docs/sprint/w320b/panel/RESULT.tsv` for these pages
(e.g. coffee-shop: get_machine_specifications, get_order_history, reorder_product, search_catalog).
`page_errors` is `-` on all three rows.

## Gate result

The node gate says: if `document.modelContext` is defined natively on >=2 of the 3 pages AND a non-empty
doc_tools list appears, the 149 native-0 is recorded as a 149-only fact. Met on 3/3 pages.

**Verdict: the 149 native-0 is a 149-only fact.** Chrome 150 registers the demo pages natively on
`document.modelContext`, so the native tool count is 4/3/6 on 150 without any polyfill. Chrome 149 lacks that
API on these pages, so its native count is 0. The polyfill-vs-native question from W3.20.b.c.b.b is therefore
about a 149 gap only; it does not affect 150-based comparisons.

## Mutations (the probe goes red)

1. Bad binary path (`.../150.0.7871.124/nope/chrome`): `chrome failed to start: spawn ... ENOENT`, exit code 2.
2. Bogus page (`no-such-demo`): title `Page not found · GitHub Pages`, doc_tools/nav_tools/testing_tools all `[]`,
   row differs from the three real pages.
