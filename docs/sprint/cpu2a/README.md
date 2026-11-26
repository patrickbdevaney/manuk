# CPU.2.a — observe on an unchanged page no longer re-cascades (chrome_ver=145.0.7632.116)

Cause (found with a MANUK_BUILD_TRACE + a temporary `mark_dirty` backtrace): not `observe` itself but the server's
own `webmcp_list` probe after navigate, via `AgentBrowser::eval_value`, which appends a scratch `<div id=__manuk_eval>`
to `<body>` and removes it. That append/remove marked body dirty, so the next `settle`/read re-cascaded and re-laid-out.

Fix: `eval_value` runs the caller's script first (value parked in a page global, no DOM write); if the DOM was clean
before and is still clean after, the scratch write/read/remove is followed by `Dom::restore_clean(seq)`. A script
that really changes the DOM (or a promise settling later) keeps its dirt.

Gate (`gate.py <manuk-mcp>`, output `gate.out`): first observe after navigate on small/div1500/t1500 = 0 cascades 0 layouts
(old behaviour, `MANUK_EVAL_DIRT_MUTATE=1`: 1 cascade 1 layout on all three); every tool output byte-identical to the old
build; the click still shows its change. Red proof: `gate.py --mutant` exits 1.
