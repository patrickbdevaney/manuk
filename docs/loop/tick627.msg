reliability(oracle): the diff's font field compared a DECLARATION to a RESOLUTION

TICK SHAPE: reliability/measurement. Bar 0 untouched; no ratchet floor moved; NO ENGINE SOURCE CHANGED.

t626 ranked placement as the largest remaining gap, so I went to the root-cause clusters. The top ones
were dominated by a font signal:

  4 site(s) · 28 hit(s)  geometry/mis-sized: height ~16px  (<div>)
    …div:nth-child(1): [0 4152 1160×483] {-apple-system/17} vs [0 4228 1160×537] {sans-serif/17}

Which reads as "Chromium used the system font and we fell back to sans-serif" — the shape of t563's Lora
finding, and a tempting concrete lead. IT MEANS NO SUCH THING.

THE TWO SIDES OF THAT FIELD WERE MEASURING DIFFERENT QUANTITIES. Chromium's probe emits
`getComputedStyle(e).fontFamily.split(',')[0]` — the first DECLARED entry of the stack, because
getComputedStyle cannot report the face actually used and NO DOM API CAN. Our side emitted
`resolved_family_name(...)` — the RESOLVED family. So the field compared a declaration against a
resolution and therefore differed on essentially every element of every page shipping a font stack,
which is every page. Chromium on Linux has no -apple-system either and falls back exactly as we do.

The comment above it asserted the opposite in so many words — "resolved family name (not the declared
stack) and used size, SO THE TWO ARE COMPARABLE" — the same failure as t617's module base URL and t624's
registry lifetime: A COMMENT STATING THE PROPERTY THE CODE DOES NOT HAVE. Fifth instance this session of
a doc comment being the most confident wrong thing in the file.

t563 ADDED THIS FIELD TO ATTRIBUTE TEXT-METRIC DIVERGENCES AND IT COULD NOT DO THE ONE JOB IT WAS BUILT
FOR. Worse than useless: it manufactured a plausible false cause and put it at the TOP of the ranked
cluster list, which is exactly where the next tick looks.

FIXED by having our side report the first declared family too. The field now honestly answers "did the
two engines' CASCADES arrive at the same font-family declaration for this element" — a real question
with a real failure mode — and no longer pretends to answer "did they use the same face", which needs a
channel Chromium does not expose.

MEASURED, and the correction is visible immediately. Re-running desitales2, every one of those clusters
now reads {-apple-system/14} on BOTH sides — identical, confirming the divergence was the instrument.
And the real cause underneath, previously buried:

  30 hit(s)  geometry/mis-sized: height ~8px  (<path>)
    …svg:nth-child(1)/path:nth-child(2): [40 649 10×9] vs [40 744 0×22]

<path> elements: 10×9 in Chromium, 0×22 here. A zero-width SVG path is a real, specific, attributable
bug — and it was sitting underneath a font signal that does not exist.

RED-PROVEN: restoring resolved_family_name reinstates {-apple-system} vs {sans-serif} on every text
element of the page. No engine behaviour changes either way — this is the instrument's own output, which
is precisely why it went unchallenged for 64 ticks.

WALL-TIME AUDIT #18 was also due, and it found the wall LEAN at 67s warm — which is the wrong number,
and that is its finding. This session banked 18 real readings, bimodal: docs-only ticks 64-70s; any tick
touching engine/ 512-716s; the SAME tree re-run immediately after 64s. Cause isolated at t610 and now
confirmed nine more times — manuk-wpt is a 51MB binary under `lto = true, codegen-units = 1`, four
`cargo run --release` calls sit INSIDE the gate phase, and the relink is attributed to gate runtime
(build_seconds: 33, unattributed_seconds: 692). Three things the window proved about the trigger: a
TEST-ONLY file counts (t621/t622 paid 710s/691s for engine/page/tests/*.rs), a REVERT counts (t620's
final tree changed no engine source and paid 708s), and THE SECOND RUN IS FREE — which is exactly what
made the first reading look like a regression. Recommendation: a per-package profile override for the
harness (lto = "thin"), which changes no assertion, drops no gate, widens no floor and moves nothing to
CI. Cargo.toml is observer-owned, so it is RECORDED with its two unruled-out confounders, not applied.
Nothing trimmed. Full entry: docs/loop/WALL-AUDIT.md #18.

Gates: none added — the change is to what the instrument RECORDS, and its correctness is the
declared-vs-declared identity now visible on live output.
WIKI: docs/wiki/conformance-and-oracles.md — "a diff field must measure the same quantity on both sides".
PATTERN: [no-pattern] — no browser capability changed.

⚠ RETROACTIVE: t563's {Lora/13} vs {serif/13} finding is the same shape and its FONT attribution is not
safe. The box widths there (293 vs 619) were a real divergence, so something was genuinely wrong; the
claim that it was "a webfont Chromium loads and we do not" rested on this field and should be
re-derived, not inherited.

NEXT: the SVG <path> zero-width cluster is now the top honestly-attributed cause on desitales2 — 30 hits
on one site, and path sizing is a bounded thing to fix.
