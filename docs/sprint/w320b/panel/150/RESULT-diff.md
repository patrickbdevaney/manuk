# W3.20.b.c.a.b: Chrome 150 WebMCP panel names vs manuk tools/list

chrome_ver=150.0.7871.124 (own measurement; never diffed against 145.0.7632.116 or 149.0.7827.155).

## Method
- Panel side: `pizza-maker/names.txt` (new, written by `check-page.py probe-150.out pizza-maker`; 16 toolsAdded events,
  7 names, `zz` self-test excluded), `french-bistro/names.txt`, `hotel-chain/names.txt` (from W3.20.b.c.a.a).
- Manuk side: `diff-manuk.mjs` starts `manuk-mcp --backend chrome` (release build, MANUK_CHROME = the 150 binary) once per
  demo page, navigates, waits 6 s, calls `tools/list`, and keeps the `webmcp_` entries with the prefix stripped.
- Set diff on names only (schemas are not in this node's gate).

## Result (diff.tsv)
| page | panel | manuk | missing in manuk | extra in manuk | verdict |
|---|---|---|---|---|---|
| pizza-maker | 7 | 7 | - | - | MATCH |
| french-bistro | 1 | 1 | - | - | MATCH |
| hotel-chain | 3 | 3 | - | - | MATCH |

## Gate red-ability
`DROP=toggle_layer node diff-manuk.mjs` removes one panel name before diffing. Result (diff-mutant.tsv): pizza-maker
DIFF with `extra_in_manuk=toggle_layer`, exit code 1. The unmutated run exits 0.

## Scope
Names only, three pages, Chrome 150. The 149 and 145 rows are not touched.
