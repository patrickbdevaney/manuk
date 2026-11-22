# RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.c.a.a.a — scoring of a transcript step whose referenced combobox is aria-hidden while its menu is open (decision, written BEFORE any re-measure)

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Contract version: **aria-hidden contract 7b556dab** (the CDP/Chrome backend
honors `aria-hidden=true` like manuk's a11y builder; focus-retained exception), with the scroll contract `.b.b`
(content scrolled out of a scroll container stays listed) already in HEAD.

## The situation (facts, from the measured docs, not from this re-run)
- opendining-4, transcript #13 and #14 reference `[h8] combobox [collapsed]` and `[h10] combobox [collapsed]`: the two
  Select comboboxes of the picker, recorded before step #7 opened the time dropdown.
- After step #7 opens the dropdown, the page's modal manager sets `aria-hidden=true` on the app root. Under contract 7b556dab
  both backends prune that root: `observe(interactive)` lists **0** comboboxes with the dropdown open on manuk and on chrome
  (`RG3baaaaaaaaaaaaaaabbaaa_chrome.md`, `RG3baaaaaaaaaaaaaaabbaa_scope.md`).
- So the referenced combobox has no live handle after step #7: #13 and #14 are UNRESOLVED on both backends
  (`docs/sprint/rg/RG3baaaaaaa-match.md`, the 10/13 vs 10/13 table), and this is the cause named in
  `docs/sprint/rg/RG3baaaaaaaaaaaaaaaacaa/RG3baaaaaaaaaaaaaaaacaa-cause.md`, not a time or page effect.

## Choice: a combobox reference that the contract withholds is scored as an UNRESOLVED step, a non-match, on both backends. No tolerance, no re-recording.

Rule, applied identically to manuk and chrome, to the 13 ordinal steps of the replay:

1. A step is **contract-hidden** iff its transcript reference is a `combobox` AND the live observation after the step contains
   no handle that resolves to that ordinal (the reference is unresolved), AND the step opened or kept open a menu/listbox.
2. A contract-hidden step counts as **not matched**. It is not rescued by any other widening: no matching a different
   combobox by ordinal shift, no matching a listbox or option in its place, no name-based match, no stale-reference
   tolerance (the `[collapsed]`→`[expanded]` tolerance from `RG3baaaaaaaaaaaaaaaaca-decision.md` is unchanged and does
   not apply: its live line does not exist here).
3. The denominator stays 13 per backend. `MIN_MANUK=9`, `MIN_CHROME=12`, `EXPECT=13` in `rg3baaaaaaa_spread.py` and the
   match script's scoring code are NOT edited. The spread script stays byte-identical to HEAD.
4. The contract-hidden count is reported by the re-measure as a separate line (the number of unresolved combobox references
   after a menu opens), so the reader sees how many steps the contract, not the agent, removed.

## Why this rule and not the alternatives (decided before the numbers of this re-run exist)
- **Re-record the transcript** so #13/#14 reference the open listbox/options: rejected. It changes the frozen replay input
  for every later comparison and would be steered toward a passing number.
- **Score a withheld combobox by a live listbox or option under the open menu**: rejected. It widens the match to a different
  role, which is exactly the kind of tuning the success bars forbid; the listbox is not the element the agent referenced.
- **Lower MIN_CHROME to the new count**: rejected by SUCCESS.md (bars are fixed; a changed bar restarts the comparison).
- The rule above is the only one that is symmetric (same on both backends), needs no transcript edit and no bar edit, and
  reports the withheld steps openly.

## What this rule predicts (recorded before the run; a red result is an honest result)
- manuk 10/13 and chrome 10/13 on each of the 3 runs, the strict count equal to the tolerant count (the tolerance does not
  fire, as in the previous re-measure), and the contract-hidden count = 2 per backend (opendining-4 #13 and #14).
- The spread gate is expected to stay red on `chrome 10/13 < 12` and `manuk 10/13 >= 9` passes. This is the result of the
  contract, not a regression to fix by moving the bar. MIN_CHROME=12 was reachable only because chrome did not honor
  aria-hidden (its earlier 12/13 relied on a stale expanded combobox it no longer lists).

## Red conditions (fixed now)
- The rule is changed after the three runs in response to the numbers: red.
- Any run's counts differ from the per-run counts above in a way not explained by the rule: report it, do not edit scoring.
- `rg3baaaaaaa_spread.py` or the bar constants edited: red.
- Mutant (the contract-hidden rows scored as matches): must raise the count; if it does not, the rule is not scoring them.

## Outcome
Measured after this doc was committed (commit 15107b73, before any re-run), chrome_ver=145.0.7632.116:

| run | manuk match | chrome match | strict manuk / chrome | contract-hidden rows (#13, #14) |
|---|---|---|---|---|
| run1 | 10/13 | 10/13 | 10 / 10 | manuk 2, chrome 2 |
| run2 | 10/13 | 10/13 | 10 / 10 | manuk 2, chrome 2 |
| run3 | 10/13 | 10/13 | 10 / 10 | manuk 2, chrome 2 |

- Every prediction above held: 10/13 on both backends in all three runs, strict equal to tolerant (the stale-reference tolerance
  scored 0 rows), and the 2 contract-hidden steps per backend are opendining-4 #13 and #14 (`[h8]` and `[h10]` combobox
  [collapsed], unresolved). Scoring code and `MIN_*`/`EXPECT` unedited; `rg3baaaaaaa_spread.py` byte-identical to HEAD.
- `rg3baaaaaaa_spread.py` exit 1 with `chrome 10/13 < 12` on all three runs: the bar stays RED. It is not relaxed. The
  raw data is in `docs/sprint/rg/RG3baaaaaaa-match-run{1,2,3}.{json,md}` (overwritten from the fresh runs; the previous
  files were the pre-contract 9/13 manuk, 12/13 chrome runs, recoverable from git history).
- Gate sensitivity: the spread gate is red on the real runs above; with a wrong `MANUK_CHROME_PIN` it exits 1, and with a
  missing run directory it exits 1 (both deliberately broken inputs).

## Verdict
Node DONE. The scoring rule is pre-registered and applied; the outcome is the honest one: with the aria-hidden contract
honored on both backends, opendining-4 #13/#14 cannot resolve on either, so the replay matches 10/13 each and the chrome
bar (12) that the earlier 12/13 satisfied is not reachable through this transcript. The bar stays red by design of the
contract; moving it would be the tuning SUCCESS.md forbids. Follow-up (new graph row): decide whether the replay transcript
is still a valid frozen input for the aria-hidden contract (a re-recording is a new, separately pre-registered node).
