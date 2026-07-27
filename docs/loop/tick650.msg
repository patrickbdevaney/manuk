reliability(cert): a sweep that survives its own engine crashing — and the first certificate in 24 ticks

The standing debt was a number: the certificate had not been measured since t626, while jQuery,
DOMPurify and htmx went from silently dead to working. Two of the three HEAD-20 runs needed to get it
were killed mid-corpus by an engine SIGSEGV — at site 5 and at site 11 — and produced no certificate,
no rows, and no record that ten sites had already been measured.

The rendering half was never the blocker. `--rows-out` appended once, AFTER the loop, so every
completed site was hostage to the survival of the whole run. That harm was already written down one
level out: `Unmeasurable::Timeout`'s doc comment says a stalled child "yields no certificate at all —
the sites that already finished are lost with it". t625 closed that for a child we invoke, by
bounding it. The case where WE are the process that dies stayed open, because a deadline on somebody
else has no analogue when the corpse is your own.

  A fixed denominator protects you from dropping the hard site. It does nothing about dropping the
  whole run — and the hard site is exactly the one that drops it.

Three properties: rows append as EARNED (flushed at the TOP of each iteration, because the body has
five `continue` paths); a crash is a COUNTED outcome via an in-flight marker the NEXT run recovers,
so the site that kills the sweep cannot be the one site that escapes the denominator; and resume
SUPERSEDES rather than accumulates, since `certificate()` reads `sites` from `rows.len()` and a
re-run grew the corpus 3 -> 6.

Also: the crash matches the parked t126 segfault on four independent marks — release-only,
threshold-on-churn, all `??` frames inside statically-linked SpiderMonkey, NaN-boxed GC values, no
frame of ours. Running its "deterministic" reproducer found all eight files clean, both named
crashers included. A parked bug's REPRODUCER rots faster than its DIAGNOSIS, and the diagnosis is
what gets remembered — so this is a LIVE unattributed heap corruption, not an old ticket.

Certificate, third run, durable ledger: sites 20 · scored 5 · shape >=0.75 on 0 · MEAN COVERAGE 98.7%
· MEAN SHAPE 37.1%. Unexplained unscored 4 -> 1 -> 0. Three of five scored sites came back
BIT-IDENTICAL to t626, the first evidence the scored half is reproducible. No headline movement is
claimed: coverage moved because the population did. Shape >=0.75 is 0 sites at t611, t626 and t650.

Gates: G_CERT_CRASH_LEDGER, RED-proven twice by mutation plus once end-to-end with `kill -9` on the
real binary — because the unit gate drives the ledger functions directly and CANNOT see the wiring in
main.rs, which is what actually changed.

WIKI: docs/wiki/conformance-and-oracles.md — "the instrument must survive the engine dying under it"
and "the remembered reproducer that no longer reproduces".
