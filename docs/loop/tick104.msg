feat(loop tick 104): open the aperture -- css-values/position/display/color join the sweep

Acting on surface-audit #3: the WPT sparse checkout held only 9 css subtrees, so css-values,
css-position, css-display, css-color (and html/semantics) were invisible zeros. Fetched them and
added the four crash-free css areas to the swept AREAS, turning unknown breadth into a ranked,
ratchet-protected work-list:

  css-values   280/1461 (19.2%)  -- 1181 failing, biggest new lever (units, calc(), var())
  css-position  63/260  (24.2%)  -- 197 failing
  css-color     27/106  (25.5%)  -- 79 failing
  css-display   10/24   (41.7%)  -- 14 failing

TOTAL 389098 -> 389478 (+380 banked passes now measured), crashes=0.

A NEW Bar-0 found by opening the aperture, correctly NOT swept: html/semantics is the biggest
single mass (~8879 failing, forms+elements) but has 2 real per-page crashes (SIGSEGV even in
isolation, not the recoverable ACCUM class). Checked out but held out of AREAS until those crashers
are root-caused -- a tracked Bar-0. The aperture found both a large mass AND a real crash.

Next capability tick: probe css-values (the top usage-weighted CSS lever now visible).

WIKI: none -- instrument/aperture tick (scripts only, no engine mechanism); the finding is in
docs/loop/JOURNAL.md + SURFACE-AUDIT.md.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
