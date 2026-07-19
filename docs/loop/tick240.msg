fix(loop tick 239): the gate that guards the ledger was itself red, and nothing was asking

Probing where to put a new page gate turned up the reason the last one did not matter:
scripts/verify.sh names 19 of the 104 gate files in engine/page/tests/. The only
package-wide manuk-page invocation (verify.sh:96-104) is a --no-run PRE-WARM — it links
every binary and executes none of them, which is the cruellest shape available, because a
build failure in any of them still REDs the wall, so the gates LOOK tended. 85 page gates
do not run in the verify wall.

THE IMPORTANT HALF OF THE RESULT IS THE 98. Full sweep (--no-fail-fast): 98 passed, 2
failed over 86 targets. No capability had been silently lost. This is a blind spot, not a
disaster — but CONSTELLATION.tsv marks rows `gated` naming gates inside that 85 (g_mse,
g_media_buffered, g_canvas_text, g_canvas_image, g_hydration, g_crypto, g_a11y_state,
g_capability), and those rows were claiming a ratchet tooth nothing bites on.

THE ONE REAL RED IS THE INSTRUMENT ITSELF. g_capability — the gate written because the
pattern ledger had been wrong six times — had gone stale in exactly the way it exists to
catch. Its claim: createDocumentType('') throws InvalidCharacterError. That is the
pre-2020 rule, when the argument was validated against the XML QName production. The DOM
spec now validates against "valid doctype name": reject ONLY ASCII whitespace, U+0000 and
`>`.

SETTLED BY WPT, NOT BY REASONING, AND THIS IS THE PART WORTH KEEPING. I nearly "fixed" the
engine before checking, and the engine was right.
dom/nodes/DOMImplementation-createDocumentType.html lists ~70 cases: exactly TWO expect
INVALID_CHARACTER_ERR (`edi:>` and `edi:a `), and ["", "", "", null] expects NO THROW.
`1foo`, `@foo`, `:foo`, `foo:`, `a.b:c` all expect a doctype back. So tick 135's relaxation
was CORRECT, and it left both the claim and the code comment describing a spec that no
longer exists — the file argued with itself for ~100 ticks while the gate that would have
said so was never invoked. When a gate and the engine disagree, the gate is not
automatically right; check the spec's own test before believing either side.

THE CLAIM MOVED, NOT THE ENGINE. The engine diff is comment-only. The new claim asserts
BOTH directions — the two names that must throw AND the empty name that must not — because
a one-sided claim is precisely what let the stale version keep looking reasonable.

TWO RED PROBES EXECUTED, NOT ASSERTED (process rule 3):
  · drop `>` from the rejection set       -> `createDocumentType validates` FAILS (throw direction)
  · add `name === ''` to the rejection set -> FAILS (the no-throw direction is load-bearing)
A third was caught by discipline rather than by the gate: the first probe was a `perl -0pi`
whose escaping silently matched nothing, and the grep assert on the replacement caught it
and aborted — "assert every scripted edit" paying for itself in the tick it was applied.

TICK SHAPE: measure gate coverage, fix the one real red it exposed, report the harness gap.

HARNESS NOTE (observer-owned, NOT touched): the recommendation is deliberately NOT "add 85
_launch lines" — that re-creates the same staleness one commit later, and wall cost is
unmeasured against a 93s ceiling. The shape that cannot go stale is a sweep with a NAMED
deny-list (new gates watched BY DEFAULT), run OFF the per-tick path and banked into
RATCHET.tsv, exactly the trade FID-SWEEP already made. Named network-dependent exclusions
and the full list: docs/loop/GATE-COVERAGE.md. Also open and pre-existing: manuk-page --lib
hard_wall_detection_and_honest_interstitial is RED and the wall misses it; manuk-media's
audio_decode/video_decode (ticks 235/236) are watched by NOTHING at all.

Constellation corrected where it was factually wrong rather than merely unwatched: the
H.264 row still read `missing` after M5 landed decode (now `partial`, with the honest
"NO PIXEL REACHES A PAGE YET" — decode is proven in isolation, <video> still paints only
its poster), and both media rows now carry the not-in-the-wall caveat.

[no-pattern] — this tick removes a false claim from WEB-PATTERNS.md rather than unlocking a
new class of the web; the DocumentType row is re-pinned to the current spec, not extended.

WIKI: docs/wiki/conformance-and-oracles.md (+ INDEX.md)
