correct(map): ResizeObserver is NOT an inert stub, and t621 said it was

TICK SHAPE: correction. Bar 0 untouched; no ratchet floor moved; NO ENGINE SOURCE CHANGED — the engine
was right and the measurement was wrong.

t621's headline finding was wrong, and it shipped. I pinned ResizeObserver as "constructs, observes,
callback NEVER called" — the G_MUTATION shape — in the map, the journal AND a commit message. It fires:

  t621 probe (script evaluation only)   fired:0
  driven as the engine drives it        fired:1   contentRect.height 55  ✓

`__runObservers` — the engine's one honest moment to ask "did this box change size?" — is called from
`view_changed`, i.e. from a real frame. My probe drove the page with eval_for_test and never called it.
A CAPABILITY MEASURED THROUGH A PATH THAT CANNOT DELIVER IT READS EXACTLY LIKE A CAPABILITY THAT DOES
NOT EXIST, and the two are indistinguishable from the JS side: the constructor works, observe() accepts
the element, and nothing arrives.

The delivery code was there the whole time AND SO WAS A PASSING TEST FOR IT — engine/page/src/lib.rs
scenario 21 asserts `ro:top=40` and drives it with page.view_changed(...). I did not look for an
existing test of the thing I was about to declare absent.

⚠ THIS IS THE SESSION'S OWN LESSON, IGNORED BY THE PERSON WHO WROTE IT TWICE. "Suspect the instrument
before the subject" is in audit #34 (t618), where it fired twice in ONE tick — the "262 unreferenced
gate files" that were 6, and the dangling G_CAP_TOUCH_PROBE that exists. Three ticks later I did it
again, and this time it was not caught by a re-check because A NEGATIVE RESULT FEELS LIKE IT NEEDS NO
CONFIRMATION. That asymmetry is the whole defect: `fired:0` was accepted at face value, while a
surprising POSITIVE would have been re-run twice.

The rule that would have caught it, and it is cheap: BEFORE PUBLISHING AN ABSENCE, NAME THE CODE PATH
THAT WOULD DELIVER IT AND SHOW THAT PATH RAN. I could not have named __runObservers at t621 — which is
exactly the state in which one must not publish "inert".

⚠⚠ AND THE NEGATIVE ASSERTION IS WHY IT WAS CAUGHT AT ALL. t621 pinned `roFired:0` into the gate because
the reconciler refused a bare `partial`. That assertion is what made the claim FALSIFIABLE, and
re-reading it while looking at __runObservers is what exposed it — one tick later instead of fifty. A
WRONG CLAIM WRITTEN AS AN ASSERTION IS ENORMOUSLY BETTER THAN A WRONG CLAIM WRITTEN AS PROSE, and t621's
own entry called that line "the one that rots". It rotted immediately, in the useful direction.

The gate now asserts the POSITIVE (roFired:true, roSize:55) and its harness calls view_changed, so it
exercises the real delivery path. RO gets a DEDICATED element: the forced-reflow assertion mutates #box,
and an observer on the same node would report that mutation — two assertions silently measuring each
other.

RED-PROVEN: disabling the `ro._cb(res, ro)` delivery call -> RED roFired:false, roSize: empty. Map row
corrected to `gated`; drift back to 0.

Gates: G_UNKNOWNS_PINNED amended — the ResizeObserver claim inverted from negative to positive, harness
now drives view_changed.
WIKI: none [forced] — no engine mechanism changed.
PATTERN: [no-pattern] — no browser capability changed.

NEXT: overflow-anchor remains genuinely missing (the property is not computed) — re-checked against the
same standard and it stands. The broader risk this exposes: EVERY `unknown`->verdict pinned from a probe
needs the delivering code path NAMED before the verdict is written.
