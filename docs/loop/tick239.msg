feat(loop ticks 235-238): land the four parked ticks — media M4/M5, canvas drawImage, stale-row probe

These four ticks were built, gated GREEN, and PARKED rather than landed: the verify wall read
426-593s against a 93s ceiling and the ratchet correctly refused to buy a capability with the
wall. The observer has since found and closed the root cause — verify.sh builds manuk-page under
TWO feature sets that share a name-stem, and the disk-hygiene stem prune ("keep newest per stem")
was deleting the second variant every 3 minutes, so every wall relinked ~23 mozjs test binaries.
An age floor on the prune fixed it. This commit lands the parked work against ONE verify of the
final tree; the four original messages are preserved verbatim in docs/loop/tick23{5,6,7,8}.msg
and on the wip/ branches, because the per-tick reasoning is the part worth keeping.

TICK 235 — media M4: AAC decodes, and the LENGTH proves it. engine/media/src/audio.rs decodes AAC
to interleaved f32 PCM via symphonia, pulled in narrowly (default-features = false, features =
["aac"]) so its audio-only ISO-MP4 demuxer is not silently acquired alongside re_mp4's. The
AudioSpecificConfig had to be REBUILT, not sliced — re_mp4 parses the esds descriptor into fields
without retaining the bytes, so there is nothing to slice. The load-bearing assertion: decoded PCM
frame count must equal the track's declared duration in its own timescale (121856 at 44100), two
INDEPENDENT sources, so a decoder that dropped, doubled, truncated or mis-read channels lands
elsewhere. Plus a non-zero peak, because correctly-sized SILENCE passes every length check.
Correction caught on re-landing and worth more than the feature: a member's `default-features =
false` does NOTHING for a `workspace = true` dependency unless the workspace declaration says it
too. The guard keeping the decoder out of ~25 gate binaries was never holding — manuk-media's
`default = []` did all the work. Nothing leaked (an empty default has nothing to leak) but the
line READ as protection. Moved to the workspace declaration where cargo enforces it.

TICK 236 — media M5: a real frame, and the honest refusal of the ones we cannot decode.
engine/media/src/video.rs decodes H.264 to RGBA via openh264. THE TRAIT IS THE DELIVERABLE, NOT
THE BACKEND: `trait VideoDecoder` ships with exactly one impl, deliberately, because that impl is
known to be temporary — openh264 is Constrained Baseline only while the web's H.264 is
overwhelmingly High. Firefox makes the same call. The work was a format mismatch that fails
SILENTLY, twice: MP4 stores AVCC length-prefixed NALs, not Annex-B start codes (prefix width read
from avcC, never assumed 4), and the SPS/PPS are not in the samples at all — they live once, out
of band, in avcC. The load-bearing assertion is NON-UNIFORMITY, because a correctly-sized flat
green field passes every size check ever written and is precisely what a mis-fed decoder emits.
isTypeSupported STAYS OFF and __mseCodecs stays empty: M5 decodes a frame, it does not play a
video, and flipping the advertisement on a partial pipeline is the black-rectangle failure.

TICK 237 — canvas drawImage: pixels finally flow INWARD. Board lever (C) said "wire fillText to
the swash raster"; process rule 2 (re-probe stale unknowns) caught that fillText was ALREADY DONE
AND GATED. The real hole was drawImage, an honest no-op carrying its own diagnosis. The gap was
DIRECTIONAL: every canvas op before this draws what the SCRIPT described; drawImage draws what the
HOST owns. Sources are named by NodeId, never copied across the FFI — a sprite sheet is megabytes
and an animation loop would copy it 60x/second. Canvases and images need SEPARATE registries even
though Page merges them: CANVASES holds live surfaces, self.images holds a snapshot from the end
of the previous script round, so sharing them makes dst.drawImage(scratch,0,0) composite the
PREVIOUS frame. Two bugs the gate found that review did not — the shim read ctx.M when M is a
CLOSURE variable (matrix decoded as identity, every transformed draw landed at the untransformed
origin), and folding xform(m) into the pattern matrix double-applies it because tiny-skia
concatenates fill_path's transform onto the SHADER too. That second one PASSED a one-corner pixel
assertion by accident: the doubly-transformed sample lands off the image, Pad clamps every pixel
to the source's top-left texel, and a flat fill of that colour satisfies any claim naming it. Fixed
in the GATE as much as the code — xform now asserts all four quadrants of an asymmetric fixture,
which a flat clamped fill cannot impersonate.

TICK 238 — measure-and-pin: the checklist was lying in BOTH directions. canvas fillText was carried
as MISSING while its pixel gate had been green for ticks (pessimistic); AVIF and JPEG XL sat at
UNKNOWN with the receipt "NEVER MEASURED", and unknown quietly reads as maybe-fine (optimistic).
The AVIF/JXL correction is a DEPENDENCY-TREE FACT, not a guess: `cargo tree -p manuk-page` contains
zero avif/dav1d/ravif/jxl crates, so those formats cannot decode BY CONSTRUCTION — stronger than a
failed fixture, which cannot distinguish a missing decoder from a bad file. The same tree shows
every present decoder (png, zune-jpeg, image-webp, gif, weezl, resvg) is pure Rust. The finding
worth more than the four rows: AVIF is an AV1 STILL IMAGE, and the media ladder already plans
re_rav1d, so ONE decoder closes the video gap AND the AVIF image gap — do not buy a separate C
dav1d for images. Readiness 53% -> 54%, platform gated 8 -> 9, unknowns 8 -> 6.

TICK SHAPE: land four gated-green parked ticks against one verify of the final tree.

RED probes were executed, not asserted, in each original tick (drop SPS/PPS -> first-frame FAILS;
feed raw AVCC -> FAILS; widen can_decode to any avc1.* -> High-profile refusal FAILS; restore
drawImage to a no-op -> 8/9 pixel claims FAIL; delete both publish hooks -> imgblit/imgcrop FAIL
while canvas->canvas stays green, proving the two source paths are independently exercised;
decode only the first AAC packet -> 1024 frames against 121856).

Gates: engine/media/tests/audio_decode.rs, engine/media/tests/video_decode.rs,
engine/page/tests/g_canvas_image.rs. Neighbours green: g_canvas, g_canvas_text,
g_inline_image_size, g_first_paint, g_dedup.

Residue stated rather than hidden: putImageData and clip() remain honest no-ops;
ImageBitmap/OffscreenCanvas/<video> have no __nodeId and are skipped explicitly rather than
throwing; there is no audio device path (cpal is a separate step — a gate that needs a working
sound card false-REDs on a headless box); isTypeSupported still answers false, because audio
decodes and video decodes but nothing plays yet.

WIKI: docs/wiki/media-pipeline.md, docs/wiki/text-layout.md (+ INDEX.md)
