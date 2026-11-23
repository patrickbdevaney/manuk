# W3.20.b.a: pre-registered rule for tools registered without an inputSchema

chrome_ver=145.0.7632.116. Written and committed BEFORE the rerun; the data it applies to is the 2026-11 W3.20.b run
(RESULT.md), where 7 native tools had no schema (the native registry reports `''`) and manuk emitted
`{"type":"object","properties":{}}`.

## Rule (one rule, no other equivalences)

For a tool present on both sides:
1. If the native `inputSchema` is absent (missing, null, or the empty string `''`), the manuk schema is EQUAL iff its
   canonical JSON (keys sorted) is exactly `{"properties":{},"type":"object"}`. Any other manuk schema (`{}`, extra
   keys such as `required`, non-empty properties) is a DIFFERENCE.
2. If the native `inputSchema` is present (non-empty), the rule does not apply: canonical JSON must be identical,
   as in W3.20.b. Nothing is relaxed for tools with a real schema.

Rationale: MCP requires `tools/list` entries to carry an object inputSchema, so a schema-less WebMCP tool must be
rendered as the empty-object schema; that carries the same information (no parameters).

Not changed: name comparison, the pass condition (`native_n>0`, no missing, no extra, no schema diff), french-bistro
(extra tool, still FAIL), doors (0 tools, still not a pass). The gate threshold (>=2 pages) is unchanged.

## Red control

`RULE_MUTANT=drop-default node compare.mjs`: the comparator replaces the manuk-side default schema with `{}` (as if
the bridge dropped the default) before applying the rule. Expected: pages that passed only via rule 1 go FAIL.
This mutates the comparator input, not the manuk binary.

## Rerun result (after this file was committed as 36202736)

chrome_ver=145.0.7632.116. `compare.mjs` -> `RESULT.tsv`, `transcript.json`: **pages_pass=5 of 6** (pizza-maker, coffee-shop,
ticket-booking, smart-home, french-bistro PASS; doors FAIL because both sides list 0 tools). Gate (>=2) met.
- No tool with a real native schema differs on any page (schemaDiff empty on all six).
- Mutant `RULE_MUTANT=drop-default` (`mutant-drop-default/`): pages_pass=2; pizza-maker, coffee-shop, smart-home flip PASS->FAIL
  (7 tool schema diffs: share_pizza, get_machine_specifications, get_order_history, getGuestMessages, getPlaylistQueue,
  lockFrontDoor, unlockFrontDoor). The rule goes red. ticket-booking and french-bistro stay PASS (no schema-less tools).
- **Surprise, not explained:** french-bistro native registry listed 1 tool this run (0 in the first W3.20.b run), so it now matches
  manuk's `book_table_le_petit_bistro`. The native side is timing/origin-trial dependent on that page; W3.20.b.b (source of the tool)
  stays open and a repeat run is needed before treating french-bistro as stable.
- This does not unblock W3.20.b: the DevTools panel reference is still absent on the pinned build (W3.20.b.c).
