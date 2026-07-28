audit(surface #43): the death-tail's price moved, and nobody re-checked it

The surface audit is due (last 699). Audit #42 left a SPECIFIC assignment rather than a generic one:
"check whether any of the six [death-tail rows] has crossed into real usage — scroll-driven animations
and anchor positioning are the two most likely to."

THE ANSWER IS YES, AND IT IS BOTH OF THE TWO IT NAMED:

  scroll-driven animations   ~4.695% OF CHROME PAGE LOADS  ·  82.58% global support (caniuse)
  CSS anchor positioning     Chrome/Edge/Opera STABLE, feature-complete · Safari 18.x partial -> full

One page load in twenty-one uses a feature we have written down as do-not-build. Not a map error —
both rows are present and honestly `missing`. A PRICING error, and there is a standing rule for
exactly it: when a capability is deferred as "needs X", RE-PRICE X. The t543 death-tail priced these
when they were experimental and single-engine. At 82.58% support authors ship them UNGUARDED, so
"feature-detect cleanly and degrade" now degrades on real pages.

Recorded as a re-pricing, NOT taken as a scope change. Phase 0 is "runs almost every website" and the
binding constraint is still SHAPE 43% (t706). But the deferral is now dated and priced instead of
inherited, and the next audit re-checks the NUMBER rather than the list.

THE OUTSIDE VIEW CONFIRMS OUR OWN DOCTRINE. Ladybird crossed 90% of all WPT subtests (Oct 2025) and
reached 2,078,912 by Jun 2026 — and still names its hardest problems as web compatibility, real-site
quirks and modern CSS layout. An independent engine at >90% WPT is not daily-drivable for the same
reason we are not: capability% cannot see feature-present-but-site-broken. That is PROCESS RULE (1)
confirmed from the only place it can be — outside.

AND A STALE INSTRUCTION IN MY OWN LAUNCH PROMPT: constellation unknowns has been 0 for FOUR
consecutive audits, yet the launch prompt still carries "(D) PROBE the ~35 constellation unknowns" as
a CO-#1 item. There are none, and have been none for ~40 ticks. A launch prompt only updates on
RELAUNCH, so this is reported for the observer — the same class as the t684 "do NOT grind the
CSS-layout tail" block that contradicted the live board for three ticks.

  map rows 395 · unknowns 0 · Interop 2026 areas 20, absent from map 0, marked missing 8
  of those 8: 6 are on the explicit death-tail; 2 are excused by NOTHING —
      container STYLE queries · CSS shape()

Sources read today, not recalled: web-platform-tests/interop 2026/README.md, web.dev/blog/interop-2026,
webkit.org/blog/17818, ladybird.org/newsletter/2026-06-30, Chrome platform status + caniuse.

ALSO DUE THIS TICK — WALL-TIME AUDIT (last 689). Total 242s and it is ONE section:

  172s  P · parity (72/72 vs headless Chrome)  71%
   25s  T   21s  G6   16s  B   5s  G1   4s  D   2s  F   1s  F4   0s  everything else

The wall is NOT lean and the hotspot is a known shape: parity launches one headless Chrome per
fixture, SERIALLY (tests/wpt/src/parity.rs). T + G6 + B together are less than a third of P. The
admissible fix by the audit's own rules — same assertion, fewer seconds, no dropped gate, no widened
floor, no sampling — is to reuse ONE Chrome across the fixture set. Parity is a CORRECTNESS
comparison over LOCAL fixtures, not a benchmark, so the exclusion that protects the perf floors does
not apply, and each fixture still fails independently for its own bug. Found and priced, NOT trimmed:
doing it badly leaks cross-fixture state into the one gate that certifies us against Chrome, so it is
named as the next wall tick rather than rushed into an audit tick.

WIKI: none [forced] — an audit of the map against the outside world; the engine is unchanged.
[no-pattern]
