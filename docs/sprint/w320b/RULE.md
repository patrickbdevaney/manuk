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
