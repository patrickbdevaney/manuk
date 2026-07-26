measure(loop): constitution check #44 — the loop is under-sceptical of bad news about itself

TICK SHAPE: the due constitution check + a re-verification that upgrades an existing negative from "the
probe saw nothing" to "the mechanism is absent, here it is". Bar 0 untouched; no ratchet floor moved;
NO ENGINE SOURCE CHANGED.

CHECK #44 (due every 8 ticks; last at 615). Its finding is about the SHAPE OF THE WHOLE WINDOW: t616-623
is eight ticks and FOUR PRODUCED NO CAPABILITY AT ALL — an audit, a parked negative result, a pinning
pass and a retraction. That is not drift: PART VII.1 makes the certificate the gate, so pointing the
instrument at the map IS the main line. The one capability result that matters is t617 (welt.de 0.0% ->
94.9% coverage), and its lesson generalises: A RULE WHOSE TWO CASES COINCIDE IN THE COMMON CONFIGURATION
GETS WRITTEN FOR WHICHEVER CASE THE AUTHOR HAD IN FRONT OF THEM — and both halves of that bug sat under
comments stating the general rule correctly.

§VI.3.5 GAINS ITS SECOND CLAUSE. Check #43 added "establish whether a measurement is a property of the
ENGINE or of the INSTRUMENT". This window fired it four more times — audit #34's "262 unreferenced gate
files" (truth: 6) and its dangling G_CAP_TOUCH_PROBE (which exists), t619's web-font blast radius
(predicted broad, measured ZERO sites), and t622's ResizeObserver (published as an inert stub; it
FIRES). So t622's rule joins it:

    Before publishing an ABSENCE, name the code path that WOULD deliver it and show that path ran.

AND UNDERNEATH IT, THE REAL FINDING: A NEGATIVE RESULT FEELS LIKE IT NEEDS NO CONFIRMATION. Every false
claim in this window — the bricked wall (t610), the getter-only list (t613), the oracle shell (t614),
ResizeObserver (t622) — was a NEGATIVE accepted cheaply, while a surprising positive would have been
re-run twice. THE LOOP IS SYSTEMATICALLY UNDER-SCEPTICAL OF BAD NEWS ABOUT ITSELF, which is the opposite
of the failure mode this project was built expecting.

SO I APPLIED THE NEW RULE TO THE OTHER NEGATIVE I PUBLISHED THIS WINDOW. t621 also declared
overflow-anchor missing. Re-verified by NAMING THE PATH: Stylo's generated property table, where
`overflow-anchor` appears ZERO times across all four built variants (it carries overflow-block,
overflow-clip-margin, overflow-inline, overflow-wrap, overflow-x, overflow-y — not this one). The
property cannot be parsed or computed, so the negative stands on a NAMED MECHANISM rather than on a
probe that returned nothing. That is the exact difference from ResizeObserver, where the delivering path
existed and my probe never called it — and checking took one grep.

§VI.3's fourth clause (one rule, N implementations) ran to NINE consecutive ticks before pausing, with
t616's variant teaching that AGREEMENT BETWEEN COPIES IS NOT EVIDENCE THE RULE IS PRESENT.

INVARIANTS: I2 held. I4 held and unusually well evidenced — t613 measured three properties at ZERO
corpus usage and declined to build them; t619 measured its own fix's blast radius at zero sites and said
so rather than claiming the class. I3 no exposure owed. I5 strengthened. PART VII/V1-SCOPE held: no
stealth work taken even though 5 of 20 HEAD sites bot-wall us, the largest measured blocker and
explicitly out of scope.

Gates: none — this tick is the cadence item and a receipt correction.
WIKI: none [forced] — no engine mechanism changed.
PATTERN: [no-pattern] — no browser capability changed.

⚠ STANDING FOR THE OBSERVER (both still open): (1) manuk-wpt's tests are NOT in the wall — 54 tests,
14s, and G_CERT_FALSIFIABLE's own doc comment says "re-run on every wall". (2) map-reconcile.sh scans
only engine/page/tests/, so a real gate under tests/wpt/tests/ reads as dangling; that single false
positive is the whole remaining drift and what blocks --strict.

NEXT on capability: dynamic import() is parked one question from working (t620); document.fonts is
undefined at 1-of-16 measured sites.
