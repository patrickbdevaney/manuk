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
| act focus | works (focus accepted; the page-side effect is not read on manuk, see differences) | works (page `activeElement` poll sees the focus) | same; the page probe is read on chrome only (see differences) |
| act hover | works (`mouseover` handler observed) | works (`mouseover` handler observed) | same; mutant `MANUK_CDP_MUTATE=no-hover` makes chrome's hover a no-op -> `every_tool_on_the_chrome_backend` red at `hover did not reach the page` |
| type | works | works | same (typed value observed) |
| scroll (dy) | works | works | same |
| screenshot | works (PNG) | works (PNG, `Page.captureScreenshot`) | same; mutant `MANUK_CDP_MUTATE=no-screenshot` -> `chrome_screenshot_mutant_fails` (and the pre-CB.1 code was exactly that mutant) |
| storage_state export/import | works (all origins) | works (cookies; localStorage of every origin the session loaded; imported foreign-origin localStorage is seeded when that origin next loads) | same: seeded localStorage exported, imported cookie read back |
| checkpoint / restore / fork / commit | works | differs: replay over a fresh Chrome per page works (page state rewinds), but the checkpoint's world tag says CLIENT though Chrome's network is invisible to the write-fence | `rollback_on_the_*_backend`; tag fix = R4.1.b |

## Differences
- **Focus events (manuk).** `act focus` moves the engine's focus, but manuk fires no `focus`/`focusin` events (the JS `focus()` binding only queues the focus change). A page that reads focus from an event handler sees nothing on manuk; the test probe polls `document.activeElement` on a timer; it sees the focus on chrome and not on manuk. Follow-up CB.1.a.a.

## Differences that are not failures
- **Tree shape.** On the same page chrome's tree has more nodes: paragraphs, generic text, and a `button` node for a `<label>` (manuk omits these). Handles are therefore not comparable across backends; the page header, role and name of the interactive controls are.
- **storage_state localStorage scope (CB.1.b).** chrome exports the loaded page's localStorage live and every other origin it visited from the snapshot taken when that page was last observed; an import for an origin not currently loaded is applied by a new-document script on the next load of that origin (then removed), so it takes effect on visit, not immediately. Gate: `storage_state_two_origins_on_*_backend` + `chrome_current_origin_only_mutant_fails` in mcp/tests/backends.rs.
- **screenshot `handle`.** Ignored on both backends (viewport only), as the tool reply says.
- **Injection shield.** Both backends return a verdict; chrome computes it from DOMSnapshot paint data (R4.1), manuk from its own layout.

## Follow-ups (no cell above is "missing" without one)
- CB.1.a.a: manuk fires `focus`/`focusin` events on `act focus` (engine `el_focus` binding), so the page probe passes on both backends.
- R4.1.b (existing): chrome checkpoint must not claim CLIENT.
