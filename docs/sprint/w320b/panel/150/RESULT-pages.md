# W3.20.b.c.a.a: WebMCP.toolsAdded capture on two more demo pages (Chrome for Testing 150)

chrome_ver=150.0.7871.124 (own measurement; never diffed against 145.0.7632.116 or 149.0.7827.155).

## Method
`probe.mjs` from pizza-maker (same flags, same script, same `WebMCP.enable` before `Page.navigate`), with the
navigated URL swapped to each demo. Per-page copies: `french-bistro/probe.mjs`, `hotel-chain/probe.mjs`.
Raw stdout: `<page>/probe-150.out`. Checker: `check-page.py <probe-out> <page-dir>`; it writes `<page-dir>/names.txt`
and exits 1 on zero events (recorded as no-panel, not counted).

## Result
| page | toolsAdded events | distinct tools (self-test `zz` excluded) | names file | verdict |
|---|---|---|---|---|
| demos/french-bistro | 4 | 1: book_table_le_petit_bistro | french-bistro/names.txt | PANEL |
| demos/hotel-chain | 8 | 3: lookup_amenity, search_location, view_hotel | hotel-chain/names.txt | PANEL |

Both pages deliver WebMCP.toolsAdded events. The probe's own `document.modelContext.getTools()` readback (the `get`
field in the `Q` line) lists the same names as the events on each page. Repeat events come from the probe's
disable/enable cycle.

## Negative controls (gate can go red)
- `check-page.py control-nomcp-150.out` (no-tools data: page): 0 events, exit 1.
- french-bistro probe output with every `WebMCP.toolsAdded` line stripped: 0 events, exit 1.

## Scope
This node captures names on Chrome 150 only. The diff against manuk `tools/list` is W3.20.b.c.a.b. No Rust was
touched by this node.
