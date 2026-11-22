# RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.c.a.a — why opendining step #7 now lists '12:00 AM' first on Chrome

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Measured 2026-11-21. Gate: `bench/agent-tasks/rg3baaaaaaaaaaaaaaaacaa_cause.py`
(exit 0 = cause named and checked against the records; `--mutate clock` and `--mutate nocause` both exit 1).

## Verdict

**The page is not the cause, and the replay is not made unfit by time.** The live page's time list is a static array, not
a clock-driven list. The change from '7:00 AM' to '12:00 AM' is an observation-contract change in our own Chrome backend:
commit `94c5e2fd` changed how `shield_of` in `mcp/src/cdp.rs` judges "offscreen", so options scrolled above their listbox
are no longer withheld. The step-#13/#14 unresolved state is a second, separate contract change (`7b556dab`, Chrome honours
`aria-hidden`), and it hits both backends. The replay is not fit as a fixed bar *across contract versions*: its counts move
with the observation contract, not with the page. This node does not relax or edit any bar.

## Evidence

**Static option list** (`option-source.txt`, verbatim module 81628 of `page-765ea8f5b6890574.js`, sha256
`8a9100c80f1299e3afe6e709ff358834dc0772fabef562be44e3538e1ecf1ad6`, live site fetched 2026-11-21): `let o=[...]` is a literal
48-entry array from "12:00 AM" to "11:30 PM". It has no clock read (no `Date()`, `Date.now`, `getHours`, `toLocaleTimeString`).
The word "Date" in that module is the occasion label "Date night". The `Select time` default is the literal `"1:00 PM"`.
The time in the URL, not the clock, selects the value.

**Step #7 records** (all chrome, `+N` = added nodes; listed options = `+N` minus the one `button`):

| record | chrome step #7 result | first listed | index in static list | options listed |
|---|---|---|---|---|
| earlier 26d5b8da (`earlier-26d5b8da-step7-run1.json`) | `+35 ~1 -0`, button + `7:00 AM`... | 7:00 AM | 14 | 34 |
| earlier d38119e6 (`earlier-d38119e6-step7-run1.json`) | `+48 ~0 -105`, `12:00 AM`... | 12:00 AM | 0 | 48 |
| rerun1 (`step7-run1.json`) | `+48 ~0 -105`, `12:00 AM`... | 12:00 AM | 0 | 48 |
| rerun2 (`step7-run2.json`) | `+48 ~0 -105`, `12:00 AM`... | 12:00 AM | 0 | 48 |

The earlier record lists exactly the suffix from index 14 (48 − 14 = 34). The reruns list the whole array. Both re-runs
reproduce the d38119e6 numbers exactly. chrome_ver is pinned in all three match runs.

**Cause 1: the shield withheld scrolled-above options, and commit `94c5e2fd` stopped that.** Before `94c5e2fd` the Chrome
backend judged "offscreen" at the scrolled position. MUI's `autoFocusItem` scrolls the selected option (1:00 PM, index 26)
into view, which puts ~14 options above the listbox viewport. Those 14 options (12:00 AM..6:30 AM) were withheld, so the list
began at 7:00 AM. `94c5e2fd` (`mcp/src/cdp.rs`, `shield_of`) adds an `unscroll` offset so "offscreen" is judged with
every container unscrolled. Nothing is withheld above the view any more. Manuk's engine already listed all 48, so the
backends now agree. The page's scroll state did not change; the observation changed.

**Cause 2: the Select-time combobox is `aria-hidden` while the menu is open, and commit `7b556dab` made Chrome honour that.**
MUI sets `aria-hidden` on the app root while the time dropdown is open. Manuk's builder already pruned it (`a91d676d`).
Before `7b556dab` the Chrome backend kept it. After, the open-dropdown comboboxes went 4→0 on Chrome (`7b556dab` message).
In both re-runs `chrome #13` and `#14` are `-` (unresolved), exactly as `manuk #13`/`#14` already were. Commit 26d5b8da's
7:00 AM record had `#13 live [h5] combobox [expanded]`. That state is gone under the current contract, on both backends.

## Is the ordinal replay unfit as a fixed bar?

- **Not because of time.** The option list is static and the clock is not read (above). The clock/TZ hypothesis was already
  refuted in `aaa30bd4` / `RG3baaaaaaaaaaaaaaab-clock.md`.
- **Yes, as a bar that is independent of observation contract.** Its counts moved from 11/13 (26d5b8da) to 12/13 (b3fb58f0,
  before `aria-hidden`) and now 10/13 on both backends (d38119e6, reproduced in both re-runs). Each move follows a contract
  commit, not a page change. The pre-registered stale-reference rule (d38119e6) was designed for a stale transcript; this
  result is not a stale transcript. Recording #13 against a combobox that is `aria-hidden` while the menu is open is a
  contract question. It is not something the match rule can settle.
- **Consequence, not a relaxation:** MIN_CHROME=12 stays red. No bar is edited; `rg3baaaaaaa_spread.py` is byte-identical to HEAD.
  The spread verdict for the 3x re-measure remains red (`chrome 10/13 < 12`). The correct follow-up is a new pre-registered
  decision on the contract (does the replay's `#13` mean the combobox, or the dropdown, once the menu opens?), recorded
  before any further re-measure. This node does not make that decision.

## Not settled here

- Whether the `aria-hidden` contract is the right reading of Chrome's own AX tree for an open MUI Select (`7b556dab` claims it is).
- Whether manuk's observation should keep scrolled-out options listed for agents (current behaviour on both backends).
