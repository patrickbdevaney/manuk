feat(loop tick 236): media M5 — a real frame, and the honest refusal of the ones we can't

M3 could find the video track and name it (avc1, 640x360). M4 decoded the audio. Neither
produced a pixel. engine/media/src/video.rs decodes H.264 to RGBA via openh264, and the
gate asserts it is a PICTURE rather than a correctly-sized flat field.

THE TRAIT IS THE DELIVERABLE, NOT THE BACKEND. `trait VideoDecoder` ships with exactly one
impl, deliberately, because that impl is known to be temporary: openh264 is Constrained
BASELINE only (B-frame reordering unimplemented) while the web's H.264 is overwhelmingly
High profile. Firefox makes the same call — OpenH264 for WebRTC, never for <video>. What
Baseline buys is real (YouTube's no-MSE fallback is avc1.42001E, 360p, and it decodes with
zero system dependencies); what it does not buy is most of the web, and the trait is what
lets the VA-API/ffmpeg backend drop in later without a caller changing. MEDIA.md calls this
the single most important design decision of the step, and it costs nothing today.

THE ACTUAL WORK WAS A FORMAT MISMATCH THAT FAILS SILENTLY, TWICE. MP4 does not store H.264
the way a decoder eats it. (1) Samples are AVCC — each NAL carries a big-endian LENGTH
prefix — while decoders expect Annex-B 00 00 00 01 START CODES; feed the raw sample and the
length parses as a garbage NAL header, yielding no frame and no readable error. The prefix
width is read from avcC (1, 2 or 4), never assumed to be 4, because assuming 4 against a
2-byte stream desynchronises on the first NAL. (2) The SPS/PPS are not in the samples at
all — in MP4 they live once, out of band, in avcC — so a decoder given only coded frames was
never told the geometry and discards everything until it is.

isTypeSupported STAYS HONEST AND STAYS OFF. can_decode parses profile_idc out of avc1.PPCCLL
and accepts ONLY 66. Answering true for High would be accepted up front and fail mid-stream,
which is strictly worse for a player than a false it can fall back from. __mseCodecs is
UNCHANGED and still empty: M5 decodes a frame, it does not play a video, and flipping
isTypeSupported on a partial pipeline is exactly the black-rectangle failure MEDIA.md warns
about. That flips at M6.

THE GATE'S THREE RED PROBES WERE EXECUTED, NOT ASSERTED (process rule 3):
  · parameter_sets: None (drop SPS/PPS)      -> first-frame test FAILED
  · feed raw AVCC, skip the Annex-B rewrite  -> first-frame test FAILED
  · widen can_decode to any avc1.*           -> High-profile refusal test FAILED
The load-bearing assertion is NON-UNIFORMITY. Dimensions come from two independent sources
(container header vs the decoder's own SPS read), but a correctly-sized flat green field
passes every size check ever written, and a flat field is precisely what a mis-fed decoder
emits.

INPUT BUG PRE-EMPTED: both committed video fixtures are High profile, so this tick would
have failed against its own fixture in a way that reads exactly like a wiring bug. Minted a
Baseline fixture with the SYSTEM ffmpeg BINARY as a dev tool — that authors a test file, it
does not link ffmpeg into the browser. Provenance recorded in tests/data/README.md.

ISOLATION PROVEN BOTH DIRECTIONS, not asserted: `video` is opt-in because openh264 compiles
C, and manuk-js -> manuk-media puts this tree into all ~25 gate binaries. cargo tree finds 0
openh264 in manuk-shell (default AND --no-default-features), manuk-js, and manuk-media
default — and 2 under --features video, so the probe can see it when present.

openh264 pinned to =0.9.0: 0.9.1+ pulls safe_arch 1.0/wide 1.5 requiring rustc 1.89 against
this workspace's 1.88, and the error names two SIMD crates without ever mentioning H.264.

Gate: engine/media/tests/video_decode.rs (3 tests). Full crate 14/14 green.
WIKI: docs/wiki/media-pipeline.md
