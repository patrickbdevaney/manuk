feat(loop tick 240): a decoded video frame reaches the SCREEN — MEDIA.md tick 1

Ticks 234/235/236 built the media pipeline to within one step of the display — demux (re_mp4),
AAC->PCM (symphonia), H.264->RGBA (openh264) — and every one of them stopped at a value in
memory. decode_first_frame returned a correct picture that nothing could show. A decoded frame
that cannot be displayed is not video.

THE STRUCTURAL CLAIM IN MEDIA.md WAS TRUE, AND THAT IS THE FINDING. "A video frame IS a
DecodedImage, and playing a video is swapping the Rc in the map the poster already occupies."
A <video> was ALREADY a replaced element, and <video poster> ALREADY decoded and painted
through the identical route as <img> — Page::images keyed by the video's own NodeId, blitted
into the content box by blit_image. So displaying a frame needs NO video path in the painter,
NO new display item, and NO relayout. Page::set_video_frame overwrites one map entry.

THE SEAM TAKES RAW RGBA, NOT manuk_media::video::Frame, AND THAT IS LOAD-BEARING. Naming the
media type in manuk-page's signature would drag manuk-media's decoder features into manuk-page,
and openh264 compiles C into the ~25 gate binaries that link it — the isolation tick 236 proved
both directions with cargo tree. Bytes keep the page DECODER-AGNOSTIC: openh264 today,
re_rav1d or a VA-API backend later, and this signature does not move. Same principle as tick
236's trait VideoDecoder — the boundary is the deliverable, not the backend behind it.

NO RELAYOUT IS A CORRECTNESS PROPERTY, NOT AN OPTIMISATION. A <video>'s box comes from its
attributes or CSS, never from the frame currently on screen. Deriving the box from the frame
reflows the page on frame one and again whenever an adaptive stream changes resolution
mid-playback — which is what adaptive streaming does BY DESIGN. Asserted: a 5x-wider frame moves
the box by less than half a pixel.

THE GATE'S BASELINE ASSERTION FOUND A REAL BUG BEFORE THE FEATURE WAS WRITTEN. g_video_frame
asserts the poster paints BEFORE any frame is handed over, because "the poster's red is gone" is
vacuous if nothing ever painted. That baseline FAILED — the box was white. Cause:
decode_inline_images matched <img> only, while the async pass (fetch_images_owned) matched
<img src> AND <video poster>. So a NETWORK poster rendered and an INLINE data: poster silently
did not — on Page::load, in every gate, and in the WPT runner. Two functions doing the same job
on different transports had drifted. Fixed by making the inline pass select its source attribute
exactly as the async pass does.

A SECOND TRAP, CAUGHT THE SAME WAY: after that fix the baseline STILL failed, because my
hand-written 8x8 red PNG had a valid header and a TRUNCATED IDAT. A corrupt fixture and a
missing feature produce the identical symptom — a white box — which is precisely the "input bug
wearing a wiring bug's clothes" the observer warned about for the H.264 fixtures at tick 235.
Decoding the base64 in python named it in one command.

THREE RED PROBES EXECUTED, NOT ASSERTED (process rule 3):
  · set_video_frame made a no-op     -> left/right claims FAIL, and fail showing the POSTER's
                                        red, which is the correct failure mode rather than blank
  · frame made a uniform green field -> the right-half claim FAILS (a flat field is what a
                                        mis-fed decoder emits, and it passes every size check)
  · inline-poster fix reverted       -> the BASELINE fails; this one ran for real BEFORE the fix,
                                        which is how the bug was found at all

Gate: engine/page/tests/g_video_frame.rs — 2 tests, asserting RASTERIZED pixels (not the images
map), both halves of an asymmetric green|blue frame, and that the poster's red is GONE, because
"a frame painted" and "the frame painted OVER the poster" are different claims and only the
second one is what playback means. Neighbours green: g_inline_image_size, g_replaced_ratio,
g_canvas_image, g_first_paint (the first two matter — the inline fix touches natural sizing).

WHAT THIS UNLOCKS IS A CLASS, AND IT IS NOT YOUTUBE: the hero video, the background loop, the
product demo, the GIF-replacement clip — <video> as moving decoration, a large fraction of all
<video> on the open web, none of which needs MSE, ABR or a codec negotiation.

RESIDUE, STATED RATHER THAN IMPLIED: nothing DRIVES frames yet — no decode thread, no clock, no
play(). This is one frame on demand: MEDIA.md tick 1, not tick 2. isTypeSupported is UNCHANGED
and still answers false, because a <video> that shows one frame is not a <video> that plays.
g_video_frame is NOT in the verify wall (tick 239; docs/loop/GATE-COVERAGE.md).

WIKI: docs/wiki/media-pipeline.md (+ INDEX.md)
