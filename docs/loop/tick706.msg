measurement(certificate): the first full-corpus number on the rebuilt instrument, and why the ledger never saw one

265 sites, one process per site, serial, 180s timeout, ~3h:

  sites 265 · scored 131 · shape >=0.75 on 11 (4.2%)
  h-overflow     clean on   76 sites (28.7%)
  overlap        clean on   49 sites (18.5%)
  reading-order  clean on   36 sites (13.6%)
  dead-target    clean on  107 sites (40.4%)

  over the 131 SCORED:  cov_mean 86.3%  shape_mean 43.0%  80,088 elements compared
  134 unscored, every one with a reason: 28 bot-wall-403 · 25 "crashed" · 21 render-failed
  · 14 thin-overlap-1 · 11 probe-blocked · 5 bot-wall-429 · 4 bot-wall-401 · 7 shell-only-N · ...

Shape is spread, not cliffed: 11 sites under 0.1, a broad mass at 0.2-0.6, 9 at 0.8+.

This is a NEW baseline and NOT differenceable against the 2026-07-20 ledger row: that row's
place_mean 6.4 is the OLD instrument's ABSOLUTE placement; 43.0% is the rebuilt instrument's
PARENT-RELATIVE shape. "6.4 -> 43.0" would be a metric swap dressed as progress. Rows banked at
docs/loop/SWEEP-t706-rows.tsv so the next sweep has something honest to diff.

THE LEDGER NEVER SAW A SWEEP BECAUSE ITS READER CANNOT READ THE INSTRUMENT.
FIDELITY-PROGRESS.tsv has ONE row, from 2026-07-20, and the board calls the corpus trend "BLIND".
Not because nobody ran a sweep — scripts/fidelity-sweep.sh greps a vocabulary the instrument stopped
printing at the t531-537 rebuild:

  the script greps                            the instrument prints
  structural: ... (241 ids, ...)              structural: 98.7% (597 paths, ...)
  PLACEMENT: X% within Npx | median offset    SHAPE: 70.3% ... | [diag] absolute PLACEMENT 1.2% (median dx=)
  MEAN VISUAL: X                              MEAN VISUAL: 65.1%      <- the only one still matching

Tested against verbatim instrument output: ids 0 matches, PLACEMENT: 0 matches, MEAN VISUAL 1 match.
Because visual matches and ids does not, the classifier takes status=NO_IDS ("unprobeable, NOT a
pass") for EVERY site that renders; fidelity-progress.sh aggregates $3=="OK" only, so scorable
computes as 0/265 and it refuses to record. Confirmed end-to-end: the script scored
blog.rust-lang.org NO_IDS/ids=0 while a direct run of the same URL on the same binary scored
structural 100.0% (1664 paths) / SHAPE 62.9%. Same site, same binary, two harnesses.

scripts/ is observer-owned and I did not touch it. Reported because it silently zeroes the headline
metric CO-#1 steers on. Its .git/fidelity-full/results.tsv was already all-NO_IDS, so no good
baseline was lost.

CORRECTION TO t705: that entry said "whichever sweep last worked must have used corpus-v2.tsv".
Wrong — grep -rn "urls-file" scripts/ returns nothing; no harness sweep has ever used --urls-file.
The certificate-over-an-empty-set defect t705 fixed is real and stays fixed, but it was not the
blocker. The vocabulary mismatch is.

"CRASHED" IS MOSTLY "TIMED OUT". The 25-row bucket is 22 timeouts at 180s, 2 segfaults, 1 other. A
crash is Bar-0; a timeout is perf. The instrument is not wrong to conflate them — recover_inflight
reports "died holding a marker" and an external timeout SIGKILL leaves the same marker a SIGSEGV
does. I know the split only because the runner logged rc= per site OUTSIDE the instrument.

No engine change this tick, so no new gates. CLUSTER: none shrunk, and that is this tick's honest
gap — --rows-out carries certificate terms, not divergence detail, so Ccd7f's corpus-wide site-count
is still unproven; t704's fix is confirmed on 3 sites by hand and 0 by ranking. The re-rank needs the
oracle crawl.

WIKI: none — a measurement of the engine, not a change to it; nothing is newly true of the browser
that was not true yesterday. [no-pattern]
