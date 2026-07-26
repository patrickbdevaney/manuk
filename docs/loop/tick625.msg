reliability(cert): one stuck child process costs the WHOLE corpus, and nothing was bounded

TICK SHAPE: reliability (a single stuck subprocess can no longer discard an entire sweep). Bar 0
adjacent — a hang bound, on the instrument rather than the engine. No ratchet floor moved.

I set out to re-measure the HEAD-20 certificate, to see whether this session's five capability fixes
moved it. THE SWEEP NEVER FINISHED. It hung on the ninth site, was killed by its outer `timeout` after
~45 minutes, and printed NO CERTIFICATE AT ALL — so the nine sites that HAD completed were lost with it.

THAT IS THE FIXED-DENOMINATOR RULE'S BLIND SPOT, AND IT IS WORSE THAN THE THING THE RULE PREVENTS. §0
makes a timeout a COUNTED outcome — but only once something NOTICES the timeout. Every Chrome invocation
in the instrument used `Command::output()`, WHICH HAS NO TIMEOUT OF ANY KIND. An unbounded child is not
counted, not excluded and not reported: it is total loss of the measurement, strictly worse than the
flattering silent drop §0 was written to stop.

It is also the project's own LESSON 4 in a new place — "an oracle must never be able to charge its own
slowness to your account". The differential crawl solved exactly this with a per-site process and a
watchdog. THE CERTIFICATION SWEEP NEVER INHERITED EITHER, and nobody noticed because a sweep that
finishes looks identical to one that could not have failed.

⚠ I COULD NOT REPRODUCE THE HANG, AND THAT CHANGED WHAT THIS TICK CLAIMS. www.ebay.com — the site it
stalled on — completes alone in 32s (manuk 19.3s · chromium 12.8s). So I have NO EVIDENCE ebay hangs,
and this entry does not say so. The defect being fixed is that AN UNBOUNDED WAIT EXISTS AT ALL, which is
true whether or not any particular page triggers it today. Naming the site would have been the t622
mistake again: publishing a conclusion the measurement does not support.

THE FIX is `output_with_deadline`, a dozen lines of try_wait polling with a kill on expiry, used by both
Unmeasurable-typed Chrome capture paths. On expiry the site becomes a COUNTED `Unmeasurable::Timeout`
row — the vocabulary t611 built — so one stuck site costs one row instead of the corpus. The default is
a deliberately generous 90s: a deadline that fires on a working site would quietly convert a fidelity
measurement into a timing one, the trap t606 already found once. THE KILL IS NOT OPTIONAL — returning
without it leaks a headless Chrome per stuck site, and the sweep's whole problem is that it runs long.

RED-PROVEN, and the number is the proof:
  with the deadline      the gate returns in    1.07s
  deadline disabled      the gate returns in  120.05s   <- the full `sleep 120`, i.e. still unbounded
The gate tests the MECHANISM, not the site, precisely because the site was not reproducible — and it
asserts the ordinary case too: a fast child must still return its output unchanged, or "bounded" would
have quietly become "broken".

⚠⚠ AND MY FIRST FIX WAS A DEADLOCK, WHICH THE WALL CAUGHT AND I THEN MISREAD. The poll loop called
try_wait and nothing else. `Command::output()` drains stdout/stderr CONCURRENTLY with the wait; a poll
loop that does not lets the child fill the 64KB pipe buffer, BLOCK ON WRITE, and never exit — so
try_wait never reports completion and the deadline fires on a process that was working perfectly.
Chrome's --dump-dom emits hundreds of KB, so this is not an edge case, it is EVERY REAL PAGE.

The wall failed it immediately: `2×timeout-90s` on G1's own two snapshots. I READ THAT AS "THE DEADLINE
IS TOO TIGHT" AND RAISED IT TO 300s — and they simply took the longer deadline instead, turning a 63s
gate phase into a >600s one. The second reading is the one that should have been suspicious: a deadline
is not supposed to be reached by a healthy page AT ALL, so a page that always reaches whatever number
you pick is not slow, it is stuck.

A BOUND THAT TURNS A WORKING PROCESS INTO A TIMEOUT IS A WORSE BUG THAN THE UNBOUNDED WAIT IT REPLACED
— the unbounded wait at least never lied about a healthy site. Fixed by draining both pipes on their own
threads. G1 now runs in 4.2s, both sites ok, no timeouts. The gate gained the case that would have
caught it first time: a child emitting 4MB (~64 pipe buffers) must COMPLETE and return all of it;
RED-proving that by not draining stdout hangs the test until its outer timeout kills it.

Gates: +1 G_SUBPROCESS_DEADLINE (tests/wpt/src/chrome.rs); Unmeasurable::Timeout joins the round-trip
assertion so the reason survives the chunk boundary like every other.
WIKI: none [forced] — no engine mechanism changed; this is the instrument.
PATTERN: [no-pattern] — no browser capability changed.

NEXT: the re-measurement this tick set out to do is still owed and can now actually complete. Two things
already visible in the partial run: ebay.com probes 25 paths with only 4 comparable (an
unexplained-unscored, the class t614 named), and comix/naukri remain shell-only — the oracle's file://
limitation, the one open question t614 deliberately did not answer.
