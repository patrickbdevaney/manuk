# manuk-mcp: tool x backend (CB.1)

chrome_ver=145.0.7632.116. Every "works" cell is proved by `mcp/tests/backends.rs` (real stdio, one loopback
page, both backends); `cdp.rs` / `checkpoint.rs` / `world.rs` cover the deeper per-backend behaviour.
`manuk` = in-process engine; `chrome` = `manuk-mcp --backend chrome`.

| tool | manuk | chrome | gate |
|---|---|---|---|
| navigate | works | works | backends.rs `every_tool_on_the_*_backend` |
| observe (full/viewport/diff) | works | works | same |
| extract (text/markdown/table) | works | works | same |
| find | works | works | same |
| act click / check / select | works | works | same (click effect read back; checkbox + select driven) |
| act focus / hover | untested here | untested here | follow-up CB.1.a |
| type | works | works | same (typed value observed) |
| scroll (dy) | works | works | same |
| screenshot | works (PNG) | works (PNG, `Page.captureScreenshot`) | same; mutant `MANUK_CDP_MUTATE=no-screenshot` -> `chrome_screenshot_mutant_fails` (and the pre-CB.1 code was exactly that mutant) |
| storage_state export/import | works (all origins) | works (cookies; localStorage of the CURRENT page origin only) | same: seeded localStorage exported, imported cookie read back |
| checkpoint / restore / fork / commit | works | differs: replay over a fresh Chrome per page works (page state rewinds), but the checkpoint's world tag says CLIENT though Chrome's network is invisible to the write-fence | `rollback_on_the_*_backend`; tag fix = R4.1.b |

## Differences that are not failures
- **Tree shape.** On the same page chrome's tree has more nodes: paragraphs, generic text, and a `button` node for a `<label>` (manuk omits these). Handles are therefore not comparable across backends; the page header, role and name of the interactive controls are.
- **storage_state localStorage scope.** manuk exports/imports every origin in its store; chrome can reach only the origin of the loaded page (cross-origin storage needs a navigation per origin). An import for another origin is skipped silently (counts report what was written). Follow-up CB.1.b.
- **screenshot `handle`.** Ignored on both backends (viewport only), as the tool reply says.
- **Injection shield.** Both backends return a verdict; chrome computes it from DOMSnapshot paint data (R4.1), manuk from its own layout.

## Follow-ups (no cell above is "missing" without one)
- CB.1.a: act `focus` / `hover` on both backends.
- CB.1.b: chrome storage_state across origins.
- R4.1.b (existing): chrome checkpoint must not claim CLIENT.
