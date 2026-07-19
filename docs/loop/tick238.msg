probe(loop tick 238): measure-and-pin — the checklist was lying in BOTH directions

Process rule 2 says re-probe a stale unknown before building it, and rule 1 of tick 237 proved
why: CO-#1 lever (C) "wire canvas fillText to the swash raster" was ALREADY DONE AND GATED.
This tick pins what that near-miss exposed — the constellation goes stale FROM OUR OWN LANDED
TICKS, and a stale row costs a whole tick to rediscover.

FOUR ROWS CORRECTED, EACH WITH A MEASUREMENT RATHER THAN A JUDGEMENT:

  platform/canvas fillText     missing -> gated (g_canvas_text)
    Not "probably works" — the gate was RUN. It asserts ink, ink colour, sparse coverage,
    pen placement, proportional advance, size scaling, real font metrics, textAlign,
    textBaseline and clearRect erasure, all as PIXELS read back. It has been green the whole
    time the board listed the work as unbuilt.

  platform/canvas drawImage    NEW ROW -> gated (g_canvas_image)
    The real hole behind the stale one. See tick 237.

  doc/images: AVIF             unknown -> missing  (receipt was literally "NEVER MEASURED")
  doc/JPEG XL                  unknown -> missing

THE AVIF/JXL MEASUREMENT IS A DEPENDENCY-TREE FACT, NOT A GUESS. `cargo tree -p manuk-page`
contains ZERO avif / dav1d / ravif / jxl crates, so those formats cannot decode BY
CONSTRUCTION — that is stronger than "we tried a file and it failed", which cannot distinguish
a missing decoder from a bad fixture. The same command shows what IS present: png, zune-jpeg,
image-webp, gif, weezl, resvg — every one PURE RUST, which is the no-C-image-FFI property the
engineering synthesis says to publish rather than rebuild. It is now recorded as measured.

THE FINDING WORTH MORE THAN THE FOUR ROWS: AVIF is an AV1 STILL IMAGE. The media ladder
already plans re_rav1d (pure-Rust AV1, BSD-2) for video, so ONE decoder closes both the video
gap and the AVIF image gap. Do NOT buy a separate C dav1d for images — that would spend the
memory-safety property to solve a problem the media track is already paying for. Recorded in
the AVIF row so the next builder finds it before shopping for a decoder.

Direction of the corrections is the point: fillText was pessimistic (built, believed missing),
AVIF/JXL were optimistic-by-omission (unknown reads as "maybe fine"). The instrument was wrong
in BOTH directions, which is exactly what the fidelity sweep found at the site level.

Readiness 53% -> 54%; platform gated 8 -> 9; unknowns 8 -> 6. No capability was claimed that a
gate does not hold.

NOT LANDED ON main: verify wall 426s vs a 93s ceiling, all 65 gates GREEN. Parked as 235/236/237
were. Harness-owned; see JOURNAL.md.
