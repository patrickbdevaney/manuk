capability(svg): `<path>` had no bbox arm at all, and now has an EXACT one

TICK SHAPE: capability (getBBox() on <path> goes from 0x0 to exact). Bar 0 untouched; no ratchet floor
moved; no capability traded — the arc refusal is unchanged and <text> still reports its honest zero.

t629 ranked this first of two: <path> — the element every icon set, chart shape generator and logo is
made of — had NO ARM IN svg_bbox's MATCH AT ALL, so getBBox() answered 0x0 for the single most common
SVG element.

AND IT WAS A DELIBERATE ZERO, WHICH IS WHY IT SURVIVED. G_SVG_BBOX's own doc comment said "<text> and
<path> report zero size ON PURPOSE" — the same honest refusal <text> makes, because a plausible guess
mis-places every label that trusts it. That reasoning was right about guessing and wrong about the
alternative: AN EXACT ANSWER IS AVAILABLE, AND IT IS NEITHER A ZERO NOR A GUESS.

MEASURED against Chromium, and the curves are the proof:
                                            manuk        Chrome
  M4 4 L20 4 L20 12 L4 12 Z                4,4 16x8     4,4 16x8    ok
  M0 0 C 0 20 20 20 20 0                   0,0 20x15    0,0 20x15   ok   control points at 20
  M0 0 Q 10 20 20 0                        0,0 20x10    0,0 20x10   ok   control point at 20
  m5 5 l10 0 l0 6 z                        5,5 10x6     5,5 10x6    ok   relative + subpath close
  M0 0 A 5 5 0 0 1 10 10                   0,0 0x0      (real)      refused, see below

THE CUBIC IS THE WHOLE DIFFICULTY. Its control points sit at y=20; the curve only reaches y=15. A
CONTROL-POINT HULL — the easy, obvious, wrong implementation — reports 20, is strictly larger than the
curve, and looks entirely plausible. It would mis-position every tooltip anchored to an icon and
mis-size every chart hit-area while reading as "close enough". So each cubic and quadratic is solved for
the roots of its derivative in (0,1).

ELLIPTICAL ARCS STILL REFUSE, DELIBERATELY. Bounding one exactly needs the endpoint->centre
parameterisation and the extrema of a rotated ellipse over the swept angle range. That work is not done,
and the honest answer to "what is this path's box" when part of it cannot be bounded is NO ANSWER — not
a guess in either direction.

⚠ AND MY FIRST RED PROBE FOR THAT REFUSAL WAS VACUOUS. I made the arc "guess" by adding the current
point, which is (0,0) — producing 0x0, indistinguishable from the refusal, and the gate stayed green. A
realistic guess consumes the seven arc parameters and takes the endpoint, giving 0,0,10,10 — which the
gate does catch. A NEGATIVE ASSERTION NEEDS A PROBE THAT PRODUCES A DIFFERENT WRONG ANSWER, not a
differently-shaped absence. Fourth vacuity caught by RED-probing this session, and the only one where
the probe rather than the claim was at fault.

RED-PROVEN THREE WAYS:
  control-point hull instead of extrema  -> RED  pc=0,0,20,20   (the plausible wrong implementation)
  remove the `path` arm                  -> RED  every path 0,0,0,0   (the pre-t630 state)
  let an arc guess its endpoint          -> RED  pa=0,0,10,10

REGRESSION: g_svg_bbox, g_path2d, g_clip_path, g_first_paint all green.

Gates: G_SVG_BBOX extended with five path cases, two of which exist specifically to fail against a
control-point hull.
WIKI: docs/wiki/box-layout.md — "an exact path bbox is the extrema, not the control-point hull".
PATTERN: [no-pattern] — getBBox is a measurement API; the class of site this unlocks is the one t629
named and it needs part (1), which is untouched.

NEXT: t629's part (1), unchanged and now the larger of the two — getBoundingClientRect() on an SVG child
still returns 0x19, an empty inline box, because svg_bbox lives in the JS binding layer and the CSS box
comes from layout. That is the subsystem; this tick makes sure it will have correct geometry to consume
for the commonest element when it lands.
