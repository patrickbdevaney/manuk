# RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.c.a — decision (written BEFORE any re-measure)

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Chosen before the re-run; the counts below are the pre-run counts
(manuk 10/13, chrome 11/13 match; spread gate red on `chrome 11/13 < 12`).

## Choice: a pre-registered match-rule exception. No re-recording.

Re-recording the transcript is rejected: it would be done after seeing the result and could be steered to land on 12/13
(a tuning risk). It would also change the frozen replay input for every later comparison.

Rule (`stale-reference tolerance`, added to `rg3baaaaaaa_match.py`, applied identically to manuk and chrome):

1. A transcript reference is **stale** iff, in the same recorded task, an EARLIER step's recorded result contains
   `NO EFFECT` (the recording's click did not do what it was meant to, so later references were recorded against a
   state the page would have left had the click worked).
2. For a stale reference whose line is `combobox [collapsed]`, a live line of the same role with state `[expanded]`
   (and no other difference) is scored a match. Nothing else is widened: no other role, no other state pair, no
   non-stale reference, no value change.
3. Every row scored under the rule is tagged `(stale-ref)` in the md/json so an outsider can recount strictly.
   The strict count (rule off) is also written, so both numbers are published.

Pre-registered expectation: exactly one row changes (opendining-4 chrome #13 -> yes), chrome 12/13, manuk unchanged
10/13 (its #13 is unresolved, the rule cannot score an unresolved step). gocalendar-2 #6 is not affected.

MIN_CHROME=12, MIN_MANUK=9 and EXPECT=13 in `rg3baaaaaaa_spread.py` are not edited (the file stays byte-identical to HEAD).

## Red conditions
- Strict count must stay 11/13 chrome (the rule off reproduces the pre-run number).
- Mutant: the rule with the stale condition removed from the transcript (no NO EFFECT) must leave #13 a miss -> chrome 11/13.
- If the bar is still red after the rule, this doc gives no reason to relax it; the node would be BLOCKED.

## Outcome (re-measure 3x, chrome_ver=145.0.7632.116): rule did not fire, bar still red, node BLOCKED

Run1-3 (docs/sprint/rg/RG3baaaaaaaaaaaaaa/): manuk 10/13, chrome 10/13 in all three runs, strict count equal (10/10), so the
stale-reference rule scored 0 rows. `rg3baaaaaaa_spread.py --dir` exit 1: `chrome 10/13 < 12` x3. MIN_CHROME/MIN_MANUK/EXPECT unedited
(spread script byte-identical to HEAD).

Why the pre-registered expectation (chrome #13 -> yes, 12/13) did not hold: the page is not the one the earlier verdict measured.
Step #7 on chrome now returns `+48 ~0 -105` with options starting `12:00 AM` (earlier runs: `+35 ~1 -0`, options from `7:00 AM`),
and opendining-4 chrome #13 and #14 are both UNRESOLVED (no combobox ordinal 1/2 exists after the click), so a rule that scores
resolved-but-expanded lines has nothing to score. The dropdown contents differ between measurement days, i.e. the replay target is
a live page whose time-dependent options change the post-click tree. This is a different cause from the stale transcript; the
rule is kept (it is harmless: strict == tolerant here) but cannot rescue the bar, and the bar is not relaxed.
Follow-up: pin or explain the live-page drift (.a).
