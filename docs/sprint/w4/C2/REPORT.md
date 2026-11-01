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

## Addendum W4.C2.a: why cdp-layer failed error-page (chrome_ver=145.0.7632.116)
- Cause (reproduced by hand): on Chrome the click on `onclick="location='/gone'"` really navigates to the 404. The layer
  retried only on "no change, url unchanged", so a navigation to an error page was returned as-is (6 calls, 0/3). It was a
  LAYER bug. On manuk's engine the `location=` assignment from the handler is silently dropped, so the click was a
  no-op and the existing retry fired: manuk's "1 call" on error-page was an engine gap, not a win. Follow-up W4.C2.a.a.
- Fix: `mcp/src/session.rs` `act` — a click that navigated to an error page is undone with `back`, then retried
  (Enter, Space, descendant). If no retry works the result says so and the page is the one before the click.
- After: cdp-layer error-page 1,1,1 calls, 3/3 (was 6,6,6, 0/3); manuk 1,1,1 (via the no-op path); retry-off 3,3,3; playwright 5,5,5 0/3.
- Gate red-proof: `mcp/tests/retry.rs` `chrome_error_page_baseline_without_retry_stays_on_the_404` (MANUK_NO_RETRY) shows the 404 with no
  recovery; `chrome_click_to_an_error_page_is_undone_and_retried` passes only with the fix. 7/7 pass.

## Addendum W4.C2.c: LLM measurement (Haiku, claude -p), chrome_ver=145.0.7632.116
Data: `docs/sprint/w4/C2c/` (own-*.tsv, real-*.tsv, transcripts). New arm `manuk-noretry` (= manuk with `MANUK_NO_RETRY=1`) in `bench/agent-tasks/run_arm.py`.
manuk and manuk-noretry were run fresh on this tree, same session, 3 runs each; cdp-layer and playwright REAL rows are reused from `docs/sprint/w4/A6-real.tsv`
(same pin, earlier tree, so that comparison is indicative only). Playwright REAL has 1 run (no band). The cdp-layer own suite (40/40/40) is the A6 figure; playwright own suite was not re-run.

| Arm | own suite x3 (pass of 39 per run; mean turns) | REAL det. subset x3 (per-run of 47; pass >=2/3) | REAL mean turns, in+out tokens/task |
|---|---|---|---|
| manuk, retry on | 39, 39, 39 (F08 fails; 3.98) | 15, 13, 12 (band 12-15); 13/47 | 30.1, 5600 |
| manuk, retry off | 37, 39, 39 (F08, L07, L12 fail; 3.93) | 13, 12, 14 (band 12-14); 13/47 | 28.4, 5088 |
| cdp-layer (A6) | 40, 40, 40 | 15, 18, 16 (band 15-18); 15/47 | 43.1, 7969 |
| playwright (A6, 1 run) | n/a | 25 (single run) | 36.0, 5091 |

Reading (only what the numbers support):
- Retry-on does NOT beat retry-off outside the 3-run band: REAL bands 12-15 vs 12-14 overlap, both 13/47 at the >=2/3 rule; own suite 37-39 vs 39-39 overlap (the one 37 is a single run). Turns and tokens are not lower with retry on (30.1 vs 28.4 turns).
- The scripted-agent win (C2 REPORT, 1 call vs 3-5) does not show up in model-driven pass rate: no claimed benefit on these suites.
- Engine vs layer: cdp-layer (15-18) is above manuk on REAL; no engine win is claimed. Playwright's single run (25) is above both.
