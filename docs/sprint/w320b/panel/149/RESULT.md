# W3.20.b.c.b.b: why native Chrome 149 lists 0 tools on coffee-shop, ticket-booking, smart-home

chrome_ver=149.0.7827.155 (own measurement row; never diffed against 145.0.7632.116 or 150.0.7871.124).
Probe: `docs/sprint/w320b/panel/probe-register.mjs`, output `docs/sprint/w320b/panel/149/probe-register.tsv`.
No manuk bridge in the probe; each page gets load + 12 s settle, then reads `document.modelContext`,
`navigator.modelContext`, `navigator.modelContextTesting`, and their tool lists.

## Measurement

| page | document.modelContext | navigator.modelContext | modelContextTesting | doc tools | nav tools | testing tools |
|---|---|---|---|---|---|---|
| coffee-shop | undefined | object | object | - | 0 | 0 |
| ticket-booking | undefined | object | object | - | 0 | 0 |
| smart-home | undefined | object | object | - | 0 | 0 |
| pizza-maker (control) | object | object | object | 7 | 0 | 0 |

Gate mutation: probing the nonexistent page name `no-such-demo` gives title `Page not found · GitHub Pages`
instead of `The Morning Ritual | Specialty Coffee & Equipment` for coffee-shop; the output row changes with the page name.

## Why native shows 0

The three pages register on `document.modelContext`, which native Chrome 149 does not define on them, and the
pages' own code skips registration when it is absent:

- coffee-shop: its inline script wraps `document.modelContext.registerTool(...)` in `if (document.modelContext) {`
  (demo HTML, around line 283).
- ticket-booking: `script.js` has `if (document.modelContext) { document.modelContext.registerTool(...) }`.
- smart-home: its bundled script runs a feature check, then polls `document.modelContext` every 500 ms for 20
  tries (10 s) and gives up. The probe at 12 s still sees `undefined`, so nothing is registered.

No page error is thrown (`page_errors` is `-` on all rows). The native registry is therefore empty because
nothing registers, not because the registry cannot be read. `navigator.modelContextTesting.listTools()` is
the correct fallback reading for these pages, and it is 0.

## Why manuk lists 4 / 3 / 6

`mcp/src/webmcp.js:24-32` installs `document.modelContext` (and `navigator.modelContext`) when the page has
none. The pages then see the guard pass and register into that shim, so manuk lists the tools:
coffee-shop 4, ticket-booking 3, smart-home 6 (`docs/sprint/w320b/panel/RESULT.tsv`, sources present).

So on Chrome 149 these three pages expose 0 tools natively and manuk's polyfill makes them expose 4/3/6.
That is a real divergence, but in the direction of manuk exposing tools native Chrome does not. It is not a
manuk bug in the bridge's own listing: the bridge supplies an API that native 149 lacks on these pages.

## Verdict

The native 0 is a measured fact of Chrome 149 on these pages, not a harness fault. The bridge's extra tools
are caused by the polyfill. Whether a 149 reference should count polyfill-supplied tools as a PASS is a
reference-rule decision, recorded as a follow-up (W3.20.b.c.b.b.a); it is not decided here. The 149
compare verdict for these pages stays FAIL as measured.

Raw rows: `149/probe-register.tsv`. Mutation output: `no-such-demo` row, title `Page not found · GitHub Pages`.
