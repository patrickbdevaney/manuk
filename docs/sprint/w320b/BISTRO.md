# W3.20.b.b: where french-bistro's `book_table_le_petit_bistro` comes from

chrome_ver=145.0.7632.116. Trace script: `trace-bistro.mjs` (native Chrome, wraps `registerTool` before page script, logs stack + outcome).

## Page-level trace

- Page HTML: `<form id="reservationForm" toolname="book_table_le_petit_bistro" tooldescription="...">` (declarative markup).
- Page loads `shared/webmcp-polyfill.js` (imperative API only) and `shared/webmcp-declarative.js`.
- Recorded call: `modelContext.registerTool({name:"book_table_le_petit_bistro", ...})` from
  `webmcp-declarative.js:313 register` <- `:358 sync` <- `:364` (the polyfill scans `form[toolname]` and registers each).
  Outcome in the pinned native Chrome: `resolved`; `navigator.modelContextTesting.listTools()` then lists the tool with the full 7-property schema.
- Not synthesized by the manuk bridge: the bridge's form branch skips any name already in its registry (`if (reg.has(name)) return`).

So the source is **registerTool through the page's declarative polyfill**. The tool is declarative in markup but arrives as a registerTool call.

## MCP label

`webmcp_tool` (mcp/src/server.rs) now emits `_meta["manuk/webmcpSource"]` from the bridge's `source` field (declarative | registerTool | synthesized; set by the bridge, not page text). On french-bistro the label is `registerTool`, matching the trace. A bridge-side `declarative` label would only appear for a `form[toolname]` with no polyfill; `synthesized` for forms with no `toolname`.

## Earlier "native 0" (W3.20.b)

W3.20.b recorded native 0 for french-bistro. Rerun today (same pin) gives native 1, 3 consecutive observations (trace + 2 compare runs). The earlier 0 is not reproduced; cause not identified (the demo site is live and could have changed, or a load-timing flake). Stated as unexplained.

## Rerun (compare.mjs with source check, `bb/`)

pages_pass=5 of 6: pizza, coffee, ticket, smart-home, french-bistro PASS (names + schemas under the W3.20.b.a rule, plus source label on french-bistro). `doors` shows FAIL only because compare.mjs requires >=1 native tool; it has 0 on both sides (same as in W3.20.b, where it was n/a).

Red control: `SRC_MUTANT=declarative` (expected label forced wrong) -> french-bistro FAIL, pages_pass=4 (`bb-mutant-src/`).
