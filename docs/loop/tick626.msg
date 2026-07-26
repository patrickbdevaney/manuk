measure(cert): the certificate re-measured — the constraint has MOVED from measurability to SCOPE

TICK SHAPE: measurement (the first complete HEAD-20 certificate since t611, and the first one a stuck
site could not destroy). Bar 0 untouched; no ratchet floor moved; NO ENGINE SOURCE CHANGED. One actuals
line appended to PHASE0-ROADMAP-ANCHOR.md per the standing rule.

t625 bounded the sweep's subprocesses so a stuck site could not discard the corpus. THIS IS THE RUN THAT
FIX MADE POSSIBLE, and it paid out on its first outing: exactly one site (bbs.ruliweb.com) hit
`timeout-300s` and cost ONE ROW INSTEAD OF ALL TWENTY.

  sites 20 · scored 6 · shape >=0.75 on 0 (bar 95%)
  MEAN COVERAGE 85.2%   MEAN SHAPE 40.0% (n>=10)   MEAN VISUAL 66.0%
  UNSCORED 14 — 5 bot-wall-403 · 4 unreachable · 1 empty-202 · 1 shell-only-1 · 1 shell-only-3 ·
                1 timeout-300s · 1 with NO recorded reason

AGAINST t611's RUN ON THE SAME TWENTY:
                              t611     t626
  scored                        5        6
  unscored with NO reason       4        1
  MEAN COVERAGE              62.8%    85.2%
  MEAN SHAPE (n>=10)         37.1%    40.0%

THE DECOMPOSITION IS THE RELIABLE MOVEMENT — 4 unexplained -> 1 — and it is instrument work, not engine
work. Every other number on a live-origin corpus is noisy in ways I measured directly this session:
ebay.com served 473KB one day and 13.5KB the next; desitales2 moved 8.1% -> 71.5% visual with no code
between runs. SO THE ONLY SITE-LEVEL CLAIM HERE IS www.welt.de COVERAGE 0.0% -> 95.7%, reproduced across
four runs and attributable to t617's module base-URL fix.

AND THE BINDING CONSTRAINT HAS MOVED, WHICH IS THE FINDING. t606 named it measurability — 9 of 14
unscoreable, p-hat unable to size anything. That is now largely closed: 13 of 14 unscored sites carry a
named cause. What replaces it is not a measurement problem at all:

    5 of 20 HEAD sites BOT-WALL us, and V1-SCOPE puts bot-detection explicitly OUT OF SCOPE.

So the achievable ceiling on this corpus is 15 of 20, not 20 of 20. That is NOT an argument for changing
the denominator — a fixed denominator is exactly what stops the hard sites vanishing — but it is an
argument for saying out loud that a quarter of the representative HEAD is unreachable BY A SCOPE
DECISION THIS PROJECT MADE DELIBERATELY, rather than leaving it to look like unfinished work.

WHAT IS ACTUALLY LEFT ON THE RENDERING SIDE, from the six sites that scored: shape >=0.75 on ZERO of
them against a 95% bar. welt.de 67.2% (n=2955), desitales2 63.7% (n=598), ikea 52.0% (n=698), aparat
46.2%, keirin 10.8% (n=556), agoda 0.0%. PLACEMENT, NOT PRESENCE — coverage averages 85.2% while shape
averages 40.0%, the same two-population split the board has carried since t267, now measured on a
representative corpus instead of a convenience one.

Gates: none — this tick runs the instrument and banks its output.
WIKI: none [forced] — no engine mechanism changed; the finding's home is the anchor's actuals line.
PATTERN: [no-pattern] — no browser capability changed.

NEXT, and it is not what the last measurement implied:
 (1) PLACEMENT on the six scoreable sites — shape 40.0% against a 95%-of-sites bar is now the single
     largest gap, and agoda at 0.0% with n=13 is the cheapest thread to pull.
 (2) The one remaining unexplained unscored (ebay: 25 paths probed, only 4 comparable) — the class t614
     named and did not close.
 (3) The oracle's file:// limitation (comix, naukri still shell-only), which caps what this corpus can
     ever say about JS-rendered sites.
Bot-walls are NOT on that list, by scope.
