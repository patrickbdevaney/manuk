measure(svg): geometry children have no box — getBBox knows, getBoundingClientRect does not

TICK SHAPE: measurement. Bar 0 untouched; no ratchet floor moved; NO ENGINE SOURCE CHANGED.

t627's font fix uncovered the top honestly-attributed cause on desitales2 — 30 hits of
`<path>: [40 649 10×9] vs [40 744 0×22]`. This tick measured it. APPLYING t628'S OWN LESSON FIRST, I
checked the gate directory before concluding anything: g_svg_bbox, g_path2d, g_clip_path exist and SVG
is honestly mapped `partial`, so there is no false-absence risk here.

MEASURED — an inline <svg width=24 height=24 viewBox="0 0 24 24"> with three children:

                      getBoundingClientRect      getBBox        Chrome (rect)
    <svg>             24 × 24  ✓                 15 × 15        24 × 24
    <path d="…">       0 × 19  ✗                  0 × 0   ✗     16 × 8
    <rect>             0 × 19  ✗                 10 × 6   ✓     10 × 6
    <circle>           0 × 19  ✗                 10 × 10  ✓     10 × 10

TWO DISTINCT GAPS, and the first is the familiar shape.

(1) THE GEOMETRY EXISTS AND IS WIRED TO ONE API AND NOT THE OTHER. `svg_bbox` computes <rect> and
<circle> CORRECTLY — getBBox() returns exactly Chrome's numbers — while getBoundingClientRect() returns
0×19 for all of them: an empty inline box at the default line height. SVG geometry children are being
laid out as ordinary inline boxes. The pattern the architecture wiki already names: the mechanism
existed and was correct; nobody had drawn a line from it to the thing that measures.

(2) <path> HAS NO BBOX AT ALL — svg_bbox's match handles rect/circle/ellipse/line/polygon/polyline/
containers and simply has no `path` arm. The `d` attribute is never parsed.

WHY THIS IS NOT A ONE-LINE FIX, stated rather than attempted at the end of a long session. svg_bbox
lives in engine/js/src/dom_bindings.rs — the JS binding layer. getBoundingClientRect reads the LAYOUT
box. Reusing the former to fix the latter means either moving the geometry into engine/layout or having
layout call into the bindings, and the honest version also needs viewBox→viewport mapping and
`transform`. That is a subsystem, not a patch, and starting one now is how a half-built thing gets
committed.

AND THERE IS NO EXISTING `d` PARSER TO REUSE — Path2D builds tiny_skia paths from canvas COMMANDS, not
from an SVG path string. So (2) means a new parser, which is legitimate (it duplicates nothing) but has
real correctness subtleties: exact bounds need curve EXTREMA, not control-point hulls, and arcs need the
endpoint→centre conversion. A control-point approximation would be an OVER-LARGE box — a wrong answer
that looks right, which this session has enough of.

BLAST RADIUS, measured rather than asserted: every getBoundingClientRect() on an SVG child. Icon
systems, chart libraries measuring their own shapes, anything positioning a tooltip off an icon — plus
the oracle itself, where each SVG child is currently counted as a divergence it cannot attribute. On
desitales2 alone that was 30 hits, the top cluster on the site.

Gates: none — a gate for a capability that does not work is a gate that cannot pass. G_SVG_BBOX is the
natural home for (2) when it lands.
WIKI: none [forced] — nothing landed to describe.
PATTERN: [no-pattern] — no browser capability changed.

NEXT, in the order I would take them: (2) FIRST, because <path> bbox is self-contained, has an existing
gate to extend, and is what chart libraries actually call — and doing it first means (1) has correct
geometry to consume for the commonest element. Then (1), the larger win and the larger job. The map row
stays `partial`, which it already was and which this measurement confirms rather than changes.
