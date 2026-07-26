reliability(map): SURFACE AUDIT #35 — audit #34 was wrong, and its own first step caught it

TICK SHAPE: reliability (the due surface audit; three capability claims restored to `gated` on evidence
that predates this session by 400+ ticks). Bar 0 untouched; no ratchet floor moved; NO ENGINE SOURCE
CHANGED — every one of these was already working.

The surface audit was due. Its finding is that AUDIT #34 — mine, ten ticks ago — demoted three rows that
ALREADY HAD GATES, one of them for 420 ticks:

  scroll anchoring   #34 said `missing`   -> G_SCROLL_ANCHOR + G_SCROLL_ANCHOR_LIVE, tick 203/204, passing
  forced reflow      #34 said `unknown`   -> G_FORCED_REFLOW, tick 213  (then t621 credited it to my gate)
  real-world quirks  #34 said `unknown`   -> g_quirks_mode.rs exists and passes

THE METHOD WAS THE BUG. Audit #34 asked "does this ROW cite a gate?" and answered "no gate exists".
Those are different questions, and SEARCHING THE MAP FOR THE GATE CAN ONLY CONFIRM WHAT THE MAP ALREADY
SAYS — which is precisely the thing under audit. Audit #31 established the right move (diff
engine/page/tests/ against the map); #34 ran that diff in ONE direction only — gates with no row — and
never the other, rows with no searched-for gate.

AND t623 COMPOUNDED IT WHILE APPLYING THE RULE MEANT TO PREVENT EXACTLY THIS. t622's rule is "before
publishing an absence, name the code path that would deliver it and show that path ran". At t623 I
applied it to scroll anchoring, found `overflow-anchor` absent from all four of Stylo's built property
tables, and called the negative confirmed. `overflow-anchor` IS THE OPT-OUT PROPERTY. The capability is
the behaviour — a feed that does not jump when content loads above where you are reading — and it has
been implemented and gated since t203. THE RULE IS ONLY SOUND IF THE PATH YOU NAME DELIVERS THE THING
YOU ARE RULING ON. I named the path for a different thing and got a perfectly true fact about it, which
is the most convincing possible way to be wrong.

THREE FALSE ABSENCES IN ONE SESSION — ResizeObserver (t621->t622), scroll anchoring and forced reflow
(t618/t621->here) — against ZERO false presences of my own making. Constitution check #44 named the
asymmetry: a negative result feels like it needs no confirmation. This is its third instance, and the
one where I believed I was checking. The loop's scepticism is well calibrated for good news and badly
calibrated for bad news — measured now, not suspected.

The practical correction to the standing rule, cheap and would have caught all three:
  Before demoting a row for want of a gate, `ls` the GATE DIRECTORY for the capability's own words —
  not the map for the row's citation. A capability's gate is named after the capability, and the map is
  the thing you already distrust.

Drift back to 0 after the corrections; G_CONSTELLATION_WELLFORMED green. Full entry:
docs/loop/SURFACE-AUDIT.md #35.

Gates: none added — three existing gates were FOUND and cited.
WIKI: none [forced] — no engine mechanism changed; the finding's home is the audit ledger.
PATTERN: [no-pattern] — no browser capability changed.

NEXT: t627's capability lead is untouched and now the best-attributed on the board — SVG <path> sized
0x22 where Chromium has 10x9, 30 hits on one site. And the ratchet marks lowered at t618 were lowered
partly on this bad method; two of the three restored rows are `doc` class, so that mark should recover
as the ratchet re-banks — worth watching rather than hand-editing.
