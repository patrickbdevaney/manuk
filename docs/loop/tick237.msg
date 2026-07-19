feat(loop tick 237): canvas drawImage — pixels finally flow INWARD

Lever (C) on the board said "wire canvas fillText to the swash raster". Process rule 2 says
re-probe a stale unknown before building it, and fillText was ALREADY DONE — wired to
engine/text with a pixel gate (g_canvas_text) asserting ink/placement/proportionality. The
board is stale by at least a tick. The REAL canvas hole was drawImage, an honest no-op
carrying its own explanation: "no image source plumbing yet".

THE GAP WAS DIRECTIONAL, WHICH IS WHY IT WAS A STUB RATHER THAN A BUG. Every canvas op
before this draws something the SCRIPT described — a colour, a path, a string. drawImage
draws something the HOST owns: the decoded bytes of an <img> the network fetched. Canvas had
exactly one pixel channel, canvas_bitmaps() -> the image map the painter reads, and it
pointed outward. manuk_js::publish_image_source is its deliberate mirror, keyed by the same
NodeId. Sources are named by NODE, never by copying pixels across the FFI — a sprite sheet is
megabytes and an animation loop would copy it 60x/second.

CANVASES AND IMAGES NEED SEPARATE REGISTRIES EVEN THOUGH Page MERGES THEM. drain_canvases
drops finished canvases into self.images beside <img> — the trick that lets the painter treat
a canvas as a replaced element and know nothing about canvas. So the obvious "look the source
up in self.images" is wrong in a way that is hard to see: CANVASES holds LIVE surfaces,
self.images holds a snapshot from the END OF THE PREVIOUS SCRIPT ROUND. Shared, the standard
double-buffer idiom dst.drawImage(scratch,0,0) composites the previous frame. Canvases are
excluded from publishing; CANVASES is looked up first.

A NEGATIVE EXTENT MEANS TWO DIFFERENT THINGS. On the source rect it re-anchors the same
region. On the destination rect it MIRRORS — drawImage(img, x+w, y, -w, h) is how a sprite
sheet draws a character facing the other way. Conflating them leaves every sprite in a game
facing the same direction, silently.

IMPLEMENTED AS A PATTERN FILL, not tiny-skia's draw_pixmap, which takes an integer offset and
cannot express the source crop, the non-uniform dst/src scale and the ctx transform at once.
A Pattern carries its own matrix so all three compose. SpreadMode::Pad, not Repeat: bilinear
reads half a texel past the crop and repeating wraps the opposite edge in as a fringe.

THE TRAP, AND THE GATE ONLY CAUGHT IT BECAUSE THE CLAIM WAS STRENGTHENED. tiny-skia
concatenates fill_path's transform onto the SHADER as well as the path, so the pattern matrix
must stay in user space. Pre-multiplying xform(m) into it type-checks, reads as obviously
correct, and PASSES A ONE-CORNER PIXEL ASSERTION BY ACCIDENT: the doubly-transformed sample
lands off the image, Pad clamps every pixel to the source's top-left texel, and a flat fill of
that colour satisfies any claim naming it. The fixture's top-left is red and the naive claim
asserted red — green on a completely broken draw. Fixed in the GATE as well as the code:
xform now asserts all FOUR quadrants of an asymmetric fixture, which a flat clamped fill
cannot impersonate. General rule recorded in the wiki.

TWO BUGS THE GATE FOUND THAT REVIEW DID NOT: the above, and the shim reading ctx.M for the
transform when M is a CLOSURE variable — ctx.M is undefined, the matrix decoded as identity,
and every transformed draw landed silently at the untransformed origin.

Gate: engine/page/tests/g_canvas_image.rs, 9 pixel claims, both source paths.
THREE RED PROBES EXECUTED, NOT ASSERTED (process rule 3):
  · restore ctx.drawImage = function(){}   -> 8/9 claims FAIL (undecoded correctly survives:
    a no-op and a spec no-op are indistinguishable, which is why it is a separate claim)
  · delete both publish hooks              -> imgblit/imgcrop FAIL, every canvas->canvas claim
    stays GREEN — the two source paths are independently exercised, not one claim twice
  · fold xform(m) into the pattern matrix  -> xform FAILS on four quadrants (and PASSED on the
    one-corner version, which is how the latent bug surfaced)

Neighbours re-run green: g_canvas, g_canvas_text, g_inline_image_size, g_first_paint, g_dedup.
Residue: putImageData and clip() stay honest no-ops; ImageBitmap/OffscreenCanvas/<video> have
no __nodeId and are skipped explicitly rather than throwing; canvas keeps its own FontContext.

WIKI: docs/wiki/text-layout.md (+ INDEX.md)

NOT LANDED ON main: the verify wall is 426s against a 93s ceiling — all 65 gates GREEN, twice.
Parked exactly as ticks 235/236 were. See the harness note in JOURNAL.md: the cause is gate
EXECUTION under contention, not the cold build, and scripts/ is observer-owned.
