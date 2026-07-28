fidelity(svg): every <path> on the web was a zero-width box one line-height tall

t703 ended by saying to stop generating spec hypotheses and go read the failing case. Reading it
found a different and much larger bug sitting in the same cluster report:

  geometry/mis-sized: width ~8px (<path>)   Chrome [40 2514 12x12]   ours [40 2595  0x22]
  geometry/displaced  svg/path              Chrome [47 3957  7x12]   ours [40 4010  0x22]

`boxes --why "svg path"` gave the mechanism in one run: `<path>` computes display:inline, has no
text, and an inline box with no text is exactly 0 x line-height. We laid the <svg> out atomically
AND THEN let CSS layout loose on its subtree. That is cluster Ccd7f -- 34 sites, 1,658 hits, third
on the board's mechanism list.

AND IT HAD A BUILD SPEC, FROM TICK 393. docs/wiki/box-layout.md L1052 says "borrow usvg, don't
hand-write geometry"; the paint half landed at t394 and its closing line reads "Remaining from the
spec: child geometry mapping". That sentence sat unread for 310 ticks while the symptom it predicts
sat at #3 in the priority ledger. A build spec whose second half is unbuilt is an untriaged tick with
excellent prose, and no ranking instrument the loop has can see one.

THE BUILD -- engine/page/src/svg_geometry.rs. Same serialized markup as the raster, usvg parsed once
per svg per decode, leaves paired to DOM shape elements in document order, applied AFTER layout via a
new Page::set_root_box. Post-layout is the point: the svg's used box exists only then, and a pass
that runs after layout cannot perturb geometry outside an <svg>. set_root_box exists because there
were THREE `self.root_box = layout_document(...)` sites, and a post-pass wired into two of three is a
pass that silently does not run on the third.

The spec said "fall back to document-order pairing". This REFUSES instead: counts must match exactly,
<foreignObject> refuses the whole svg (it holds real HTML whose boxes CSS owns, and this pass replaces
an svg box's children wholesale), and a mismatched aspect refuses too. A wrong pairing attributes one
shape's bounds to another element -- a plausible-looking false number, strictly worse than the honest
0x22 it replaces. A refused svg keeps its old boxes.

                          HEAD      fill-bbox     stroke-bbox (landed)
  desitales2  SHAPE       61.5%       70.3%          70.3%   <- byte-reproducible control
  desitales2  VISUAL      65.1%       65.1%          65.1%
  desitales2  COVERAGE    98.7%       98.7%          98.7%
  en.wikipedia SHAPE      55.8%       55.5%  <-!!     55.8%
  en.wikipedia VISUAL     88.7%       88.7%          88.7%
  blog.rust-lang SHAPE    62.9%       62.9%          62.9%
  layout suite            94/94       94/94          94/94

getBoundingClientRect IS THE DECORATED BOUNDING BOX -- IT INCLUDES THE STROKE. SVG 2 gives an element
two boxes and the two DOM methods return different ones: getBBox() is fill/geometry,
getBoundingClientRect() the transformed decorated one. The oracle probes with the latter, so
abs_stroke_bounding_box() is right and abs_bounding_box() is not.

The motivating site could not tell them apart. desitales2's icons are unstroked, where the two boxes
are identical -- so the fill box matched Chrome EXACTLY there (7x12, 10x9, 12x12) and quietly cost
-0.3 SHAPE on en.wikipedia.org, whose icons are stroked. Deterministic both ways. Three ticks refused
a -0.7 on a much larger win; a -0.3 of my own gets the same answer, and here the answer turned out to
be a spec question rather than a tolerance to widen.

Also measured, also not the obvious reading: abs_bounding_box() is in Tree::size() CANVAS space, not
viewBox units -- the viewBox transform is already folded in. The first gate asserted otherwise and
failed, which is the gate doing its job on its author.

One VISUAL reading of 64.8 on desitales2 was a DRAW, not a regression (65.1 on the three readings
either side). Separately, the 67.9% VISUAL seen in this tick's first run was the PARKED t701 patch's
effect, not this one's -- that patch is worth +2.8 VISUAL, which three parking ticks never recorded.

Gates: svg_geometry 5/5 (mechanism + three refusals + the canvas-space scale), layout 94/94.
Pre-existing RED, present at HEAD and unrelated: manuk-page lib test
static_import_scanner_finds_specifiers_and_skips_the_rest. Not touched, not caused here.
Constitution check #N (tick 704) recorded.

WIKI: docs/wiki/box-layout.md -- the tick-393 SVG-internals spec's geometry half closed out, with
both measured corrections to the spec (decorated/stroke bbox; canvas-space bounds) and the refusal
list; docs/wiki/INDEX.md updated.
