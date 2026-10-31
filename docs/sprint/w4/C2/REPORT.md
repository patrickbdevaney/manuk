# W4.C2: layer-applied retry for dead clicks (3 arms, no LLM)

chrome_ver=145.0.7632.116; harness `bench/agent-tasks/c2_retry.py`; data `retry.tsv` (3 runs per cell).
Change: `mcp/src/session.rs` `act` — when a click's own effect diff is "no change, url unchanged", the layer retries
before returning: keyboard Enter, Space, then the target's interactive descendant; an attempt that lands on an error
page is undone with `back`. `MANUK_NO_RETRY=1` disables it. Tests: `mcp/tests/retry.rs` (5, incl. the baseline).

Metric: tool calls a scripted agent needs to see the fixture's token (it climbs click → observe → key → inner link; cap 8).
This is a proxy for a model, NOT the REAL / own-suite x3 LLM run the node's gate also names (see follow-up W4.C2.c).

| fixture | manuk (layer retry) | manuk, retry off | cdp-layer (same layer code) | playwright |
|---|---|---|---|---|
| keydown-only (handler only on keydown) | 1,1,1 calls, 3/3 | 3,3,3, 3/3 | 1,1,1, 3/3 | 4,4,4, 3/3 |
| inner-link (handler on a child link) | 1,1,1, 3/3 | 5,5,5, 3/3 | 1,1,1, 3/3 | 6,6,6, 3/3 |
| error-page (click goes to a 404) | 1,1,1, 3/3 | 3,3,3, 3/3 | 6,6,6, **0/3** | 5,5,5, 0/3 |
| plain-button (control: click works) | 1,1,1, 3/3 (no retry fired) | 1,1,1 | 1,1,1 | 2,2,2 |

## Reading (only what the numbers support)
- Gate met: a no-op click recovers in 1 tool call on manuk (3/3); mutation (`MANUK_C2_MUTATE=1`, retry off) → 3 calls, GATE FAIL.
- **The recovery on keydown-only and inner-link is a LAYER win**: cdp-layer runs the same layer code over Chrome and ties at
  1 call; no engine claim. Both beat playwright (4–6 calls) and retry-off (3–5), bands are zero-width (identical every run).
- error-page: manuk 1 call vs cdp-layer 6 (0/3). Cause NOT established (candidates: the chrome backend's `back`/page-text
  differs, or Chrome's error rendering defeats `looks_like_error`). Not claimed as an engine win; follow-up W4.C2.a.
- Control: on a working click no retry fires (`layer_retried` 0).
- Risk: Enter/Space after a "no change" click can double-fire a handler that acted invisibly (e.g. a Copy button).
  Retry only fires on zero observable effect, but this is unmeasured on real pages.
- **Engine gap found**: inline `onclick="...;return false"` on `<a href="#">` does not cancel navigation (the link was followed
  and the page reloaded); `event.preventDefault()` works. Follow-up W4.C2.b.
- **C1 caveat**: `@playwright/mcp@0.0.83`'s `browser_click` takes `target`, not `ref`; c1_settle.py sends `ref`, so its
  playwright arm may have errored (0/3, 1 ms). The C1 playwright column is unproven; follow-up W4.C2.d.
