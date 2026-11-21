# RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b — why opendining's time list starts at '12:00 AM' on manuk, '7:00 AM' on Chrome

chrome_ver=145.0.7632.116, 2026-11-21. Host TZ America/New_York.
Note: `target/release/manuk-mcp` was built from HEAD plus the uncommitted aria-hidden WIP left in `mcp/src/cdp.rs` by the
BLOCKED sibling node `.a`. That WIP changes only the Chrome backend's aria-hidden pruning and does not affect clock or scroll.

## Verdict: it is not the clock, timezone or locale

`bench/agent-tasks/rg3baaaaaaaaaaaaaaab_clock.py` (raw output: `RG3baaaaaaaaaaaaaaab-clock.txt`):

| backend | tz | offset | locale / lang | page clock − host clock | first options after step #7 | options listed |
|---|---|---|---|---|---|---|
| manuk | America/New_York | 300 | en-US / en-US | +0 s | 12:00 AM, 12:30 AM, 1:00 AM | 48 (12:00 AM..11:30 PM) |
| chrome | America/New_York | 300 | en-US / en-US | +0 s | 7:00 AM, 7:30 AM, 8:00 AM | 34 (7:00 AM..11:30 PM) |

Both backends report the same clock, timezone and locale, yet the first options differ. That meets the node's pre-registered
red condition exactly: **gate RED, so the clock/TZ hypothesis is refuted.** (`--mutate same-opts`: exit 0; see the journal.)

The site never reads the clock for this list. Its page chunk (`page-765ea8f5b6890574.js`) holds the options as a static
48-entry array `["12:00 AM","12:30 AM",…,"11:30 PM"]` rendered as MUI `MenuItem`s inside a `Select` (default
`"1:00 PM"`). Every slot exists on both backends; what differs is which slots each observation **lists**. Chrome omits
exactly the first 14 (12:00 AM..6:30 AM), and '7:00 AM' is index 14.

## What does explain it: listbox scroll position × what the observation keeps

`bench/agent-tasks/rg3baaaaaaaaaaaaaaab_scroll.py` (raw output: `RG3baaaaaaaaaaaaaaab-scroll.txt`) uses a data: page with a
400px overflow:auto listbox of 48 × 36px options:

| mode | manuk scrollTop / listed / first | chrome scrollTop / listed / first |
|---|---|---|
| no scroll | 0 / 48 / opt 0 | 0 / 48 / opt 0 |
| `scrollTop = 504` | 504 / 48 / opt 0 | 504 / 35 / opt 13 |
| `focus()` on opt 26 | **0** / 48 / opt 0 | **754** / 28 / opt 20 |

The probe shows two manuk differences, and either one alone is enough to put '12:00 AM' first:

1. **`focus()` does not scroll its scroll container on manuk** (scrollTop stays 0; Chrome scrolls to 754 to show the
   option). MUI's `MenuList autoFocusItem` focuses the selected item when the menu opens, so this is how Chrome's menu ends up
   scrolled down.
2. **manuk's observation lists options scrolled above a scroll container's visible area.** At the same scrollTop=504,
   Chrome's backend withholds options 0–12 and manuk keeps all 48. Options below the visible area are kept by both, so the
   Chrome backend's clipping is asymmetric (it drops only content above the view).

Gate for this part: exit 0 when the first listed option differs in a scrolled mode (it does: `scrolltop`, `focus`). It goes
red (exit 1) under `--mutate same`, where Chrome's listing is replaced by manuk's.

## Not settled here (follow-ups)

- Which of (1) and (2) is active on opendining itself: read the MUI menu paper's scrollTop after step #7 on each backend.
- Whether manuk's `focus()` should scroll into view as Chrome does (HTML focusing steps: "scroll into view").
- Which observation contract is right for content scrolled out of a container: withhold only above (Chrome backend), withhold
  none (manuk), or withhold both sides.
