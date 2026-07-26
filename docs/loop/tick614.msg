reliability(cert): the oracle was rendering a SHELL, and the certificate was scoring it

TICK SHAPE: reliability/measurement. Bar 0 untouched; no ratchet floor moved; NO VERDICT CHANGED.
(The due self-audit ran and is CLEAN — methodology and reality agree, 49 recorded process defects,
every gate declaring how to break it. LAST_AUDIT_TICK moves to 614.)

t611 ended with a residue it could not explain: "4 of those UNSCORED sites have NO recorded reason —
the instrument could not say why, which is an instrument gap, not a result." This closes it, and the
cause is THE ORACLE'S OWN REFERENCE, not the sites.

MEASURED — the same Chrome, the same URL, two ways:

  comix.to    file:// snapshot (what the instrument scores)    28 elements ·  4 with a box
              live navigation                                ~2643 tags

A 94x gap. The instrument feeds both engines ONE fetched copy served from a file:// temp file —
deliberately, and for a good reason recorded in chrome.rs: pointing Chrome at the live URL makes the
TWO CHROME PROBES render different pages (Wikipedia's origin injects a banner a local copy never sees,
which once pinned a metric at 5,122px across four correct fixes). THE COST OF THAT CHOICE WAS NEVER
PRICED: from file:// the page's origin is null, so a JS-rendered site's fetches and module loads are
cross-origin and blocked, and Chrome builds essentially nothing.

And the certificate scored the small side of it. comix.to reported `coverage 66.7%` — computed over
THREE elements — printed in the same column, in the same units, as bbs.ruliweb.com's 4,122-path score.
That is not a measurement of comix.to. It is a measurement of comix.to's pre-hydration shell.

THIS TICK DOES NOT FIX THE ORACLE. IT STOPS THE ORACLE LYING, and the distinction is deliberate: the
file:// choice defends a real invariant, and replacing it is a design change needing its own tick and
its own evidence. What is fixed is that a shell is NAMED rather than scored — Unmeasurable::ShellOnly(n),
carrying the element count that proves it. The threshold is CERT_MIN_SHAPE_SAMPLE, REUSED RATHER THAN
INVENTED: the certificate already refused to score a placement ratio over fewer than ten elements, so
no verdict changes. What changes is that the refusal states its cause.

  comix.to        UNMEASURABLE [shell-only-3]
  www.naukri.com  UNMEASURABLE [shell-only-1]
  2 of 3 sites UNSCORED — 1×shell-only-1, 1×shell-only-3

AND THE HEADLINE WAS INFLATED BY THE SHELLS, WHICH IS THE PART WITH TEETH. shape_stats returns 1.0 over
an empty sample, so vacuous rows averaged a PERFECT score into MEAN SHAPE. Recomputed from t611's own
saved sweep output rather than by re-running it:

  comix.to      100.0%  n=2   <- vacuous      desitales2   63.0%  n=598
  welt.de         0.0%  n=1   <- vacuous      agoda.com     7.7%  n=13
  naukri.com    100.0%  n=0   <- vacuous      keirin.jp     9.9%  n=503
  ebay.com        0.0%  n=4   <- vacuous      ikea.com     51.7%  n=698
                                              ruliweb      53.1%  n=4091
  MEAN SHAPE as reported (9 rows)   42.8%
  MEAN SHAPE over n>=10 only        37.1%

t611's own headline was 5.7 points optimistic, from four rows scored over 0-4 elements, two of which
reported a flawless 100%. All three headline means now take the SAME SITE SET the certificate does —
otherwise two numbers three lines apart are computed over two different populations. On a three-site
run that correction alone moved MEAN SHAPE 70.0% -> 10.5%.

RED-PROVEN, TWO MECHANISMS (both new claims; the surrounding gate's others were RED-proven at t611):
  · give `shell-only-N` a tag with no reader  -> RED, the reason dies at the chunk boundary
  · let a reasoned row into the scoring loop  -> RED, a 3-element shell meets the certificate's bar
REGRESSION: G1's own corpus is server-rendered and untouched — file:// hn + wikipedia still probe 767
and 1,364 elements, score 99.9% coverage, both `ok`.

Gate: G_UNMEASURABLE_REASON extended (tests/wpt/src/fidelity.rs) — a shell stays in the denominator, is
never scored, and is named with the element count that proves it.
WIKI: docs/wiki/conformance-and-oracles.md — "the oracle renders a shell for JS-built pages, and the
certificate was scoring it".
PATTERN: [no-pattern] — no browser capability changed; this is the instrument.

NEXT: the real question this exposes and does not answer — CAN the oracle measure a JS-rendered page at
all? file:// protects one invariant (both Chrome probes see one document) at the cost of another (the
document is the one a user would get). Serving the snapshot from a local http://127.0.0.1 origin would
satisfy both and is the obvious candidate — but it changes what EVERY PAST NUMBER MEANT, so it wants
its own tick with a before/after on the same corpus, not a quiet swap.
