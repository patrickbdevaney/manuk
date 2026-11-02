# W3.20.b: WebMCP tools/list on Chrome demo pages vs Chrome's own registry

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Node: `docs/sprint/w320b/compare.mjs`. Raw: `RESULT.tsv`, `transcript.json`.

## Gate as written

"names + JSON schemas of webmcp_* tools equal the DevTools panel list on >=2 demo pages".

## What the reference is

- **The DevTools WebMCP panel cannot be read on the pinned build.** `chrome.real` 145.0.7632.116 has no WebMCP inspector domain (grep of the binary finds only `WebMCP`, `WebMCPTesting`, `ModelContext*` strings). The panel is documented for later Chrome (the docs describe it under Application > WebMCP).
- **Reference used:** `navigator.modelContextTesting.listTools()` in the same pinned build, launched with `--enable-features=WebMCP,WebMCPTesting`. This is Chrome's native registry, which is what the panel would read. It is not the panel itself.
- **Spelling shim on the reference side only:** the demos register on `document.modelContext`, which the pinned 145 build does not define. The reference adds `document.modelContext = navigator.modelContext` before page script (`Page.addScriptToEvaluateOnNewDocument`). Without it the native registry was empty on every demo (the first run; its output was overwritten, so this is a note, not a file).
- **Manuk side:** `manuk-mcp --backend chrome`, `navigate` then `tools/list`, filtered to `webmcp_*`.
- **Demos:** `https://googlechromelabs.github.io/webmcp-tools/demos/<name>/` (six pages; live on 2026-11-01).

## Result (strict comparison, pre-declared: names equal and canonical JSON schemas equal)

| demo | native | manuk | verdict | difference |
|---|---|---|---|---|
| ticket-booking | 3 | 3 | PASS | none |
| pizza-maker | 7 | 7 | FAIL | `share_pizza` schema: native has none, manuk emits `{"type":"object","properties":{}}` |
| coffee-shop | 4 | 4 | FAIL | `get_machine_specifications`, `get_order_history`: same absent-schema case |
| smart-home | 6 | 6 | FAIL | 4 tools with absent schema (`getGuestMessages`, `getPlaylistQueue`, `lockFrontDoor`, `unlockFrontDoor`) |
| french-bistro | 0 | 1 | FAIL | manuk lists `book_table_le_petit_bistro`, native registry does not contain it (likely a declarative `<form toolname>` tool, which the pinned build does not register natively; not verified) |
| doors | 0 | 0 | n/a | no tools on either side (page uses `resumeTool`, not a registration) |

Strict result: **1 of 6 pages pass** (gate needs >=2). Names match on 4 pages (pizza, coffee, ticket, smart-home), and on 3 of them the schemas also match exactly. The only schema differences are the 7 absent-schema tools; no tool with a real schema differs.

**Not a pre-registered rule, disclosed:** if "absent native schema" is counted as equal to manuk's MCP default object schema, 4 pages pass (pizza, coffee, ticket, smart-home) and only french-bistro fails. That equivalence was identified after seeing the data, so it is recorded as a follow-up decision (W3.20.b.a), not applied to the verdict.

## Red control

`MANUK_CDP_MUTATE=webmcp-no-bridge` (bridge off, `mutant-no-bridge/`): manuk lists 0 tools on every page; the native side is unchanged. Ticket-booking goes PASS -> FAIL, so the comparison can go red.

## Verdict

BLOCKED. Two reasons:
1. The literal reference (DevTools panel) does not exist on the pinned build; the comparator is the native registry instead (see follow-up W3.20.b.c for a panel reference on a newer pin, which is an owner decision).
2. Strict gate is red on 5 of 6 pages: schema-less tools are represented differently, and french-bistro lists a tool the native registry does not see.
