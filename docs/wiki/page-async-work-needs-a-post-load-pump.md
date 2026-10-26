# A page's fetches and timers need a pump after load, not only during it

`Page::finish_loading` is the only place that performed a page's queued `fetch()`/XHR requests, and the
virtual event-loop clock stops at the load horizon (`__timeBudget` = load time + 60 s). So under
manuk-mcp anything started later was dead: a click handler's `fetch(...).then(...)` (cookie banner
POST, subscribe dialog) never reached the server, and an interval armed at load (an infinite feed's
500 ms `setInterval`) never fired again.

Mechanism (R2.1.3): `Page::settle_async(fonts, vw, budget)` opens the virtual clock 600 ms at a time
(3 slices, `eval_for_test("__timeBudget = max(...)+600")`, which also drains the loop), and after each
slice runs `pump_page_fetches`. `AgentBrowser::settle()` wraps it with a 2 s real-time bound
(`SETTLE_BUDGET`); it runs after `dispatch_click` in `activate`, after a scroll, and before every
read-only MCP tool (`observe`/`extract`/`find`/`screenshot`). `AgentBrowser::scroll_by` now calls
`Page::view_changed`, so scroll listeners and observers fire on a real move.

Time is virtual, so nothing sleeps and a `--deterministic` run replays. The gate is
`scripts/r213-gate.py` (banner POST, feed item 45, probe fetch seen by the server); it is red when
`settle` is made a no-op.
