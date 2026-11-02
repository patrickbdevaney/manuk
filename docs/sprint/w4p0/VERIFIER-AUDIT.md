# W4.P0.e verifier audit

chrome_ver=145.0.7632.116. Table: `VERIFIER-AUDIT.tsv` (37 rows, one per audited run, each with a transcript or
stored-answer pointer). Check: `python3 -I bench/agent-tasks/verifier_audit.py` (`--selftest` proves a blank
pointer, a dead pointer and an edited transcript each go red). `FROZEN.sha256` pins SUCCESS.md, suite-v1.json and
suite-v1-external.json (unchanged).

Scope: all 30 manuk external-live runs (X01-X10 x 3, read from the full transcripts in `docs/sprint/ext-run/transcripts`),
F08 r3, gomail-8 r1-3, and cdp-layer opendining-3/5/8 (stored answer only). The REAL click/typeahead rows are
not audited here: their transcripts were not kept.

Verdicts: 17 FALSE_PASS, 10 TRUE_FAIL, 5 TRUE_PASS, 2 FALSE_FAIL, 3 UNVERIFIABLE.
- External manuk: 17 of the 19 scoreboard passes in X01-X10 (X01 3, X02 2, X03 1, X05 3, X06 2, X09 3, X10 3) are answers
  that say the page was not read (from memory, own calculation, or Wolfram never rendered). Only X04 r1/r3 are real passes.
  That moves manuk's external count from 19 scoreboard passes to 2 supported ones; the compass agreement on those tasks
  counted the unsupported passes.
- X04 r2 and F08 r3 are false fails (correct answer, brittle exact string).
- gomail-8: the COMPASS "trashes 1 of 4, false pass" finding is withdrawn. The task's own evals require exactly one
  updated email (id 3), so the 3/3 stands.
- opendining-3/5/8 (cdp): UNVERIFIABLE without the transcript or end state.

SUCCESS.md is frozen, so the fixes (require evidence from the page, accept date formats) go to a suite v2.
