# Media test fixtures

Three **real** encoded MP4 files, not synthesised ones. That distinction is the point: a fixture
written by our own code proves that our writer and our reader agree, which is a tautology. These
came out of real encoders and carry the box layouts, timescale rounding and sample-flag conventions
that real files have — including the ones that surprised us (see `tests/demux.rs`).

| file | what it exercises |
|---|---|
| `bear-640x360-v-2frames_frag.mp4` | **fragmented** MP4 — `moof`/`traf`/`trun`, the form MSE streams. Video-only, H.264 High (`avc1.64001E`), 640×360, 2 frames at 30000 timescale. |
| `bear-mpeg2-aac-only_frag.mp4` | **audio**, fragmented — AAC-LC (`mp4a.67`, MPEG-2 AAC object type), 44100 Hz stereo, 119 packets. The M4 decode fixture; its track duration (121856 units at a 44100 timescale) is exactly the PCM frame count a correct decode produces, which is what makes the decode assertable rather than merely runnable. |
| `blackwhite_yuv420p.mp4` | **progressive** MP4 — a classic `stbl` sample table with no fragments. H.264, 240×240, 1 sample. |

All three are from the Chromium `media/test/data` corpus, which is BSD-licensed (see the Chromium
`LICENSE`). They are checked in because the tests must run on a clone that has no Chromium tree.

- `bear-baseline_frag.mp4` — H.264 **Constrained Baseline**, level 3.0, 640x360, fragmented, no
  audio. Transcoded from `bear-640x360-v-2frames_frag.mp4` with the system `ffmpeg` binary used as a
  **dev tool** (authoring a test file, not linking ffmpeg into the browser):
  `ffmpeg -i bear-640x360-v-2frames_frag.mp4 -c:v libx264 -profile:v baseline -level 3.0 \
   -movflags +frag_keyframe+empty_moov -an bear-baseline_frag.mp4`
  Needed because **both** other video fixtures are High profile (`AVCProfileIndication = 100`) and
  the M5 openh264 backend is Constrained Baseline only, so it would fail against its own input.

- `bear-av-baseline_frag.mp4` — the **first multi-track fixture** in this tree: Constrained Baseline
  H.264 (`avc1.42C01E`, 640x360, 3 frames, 30000 timescale) **and** AAC-LC (`mp4a.40.2`, 44100 Hz
  stereo, 4096 PCM frames) in one fragmented file. Muxed from the two single-track fixtures above
  with the system `ffmpeg` binary as a **dev tool** (authoring a test file, not linking ffmpeg into
  the browser):
  `ffmpeg -i bear-baseline_frag.mp4 -i bear-mpeg2-aac-only_frag.mp4 -map 0:v:0 -map 1:a:0 \
   -c:v copy -c:a aac -shortest -movflags +frag_keyframe+empty_moov bear-av-baseline_frag.mp4`
  Needed because every other fixture carries exactly ONE track, so nothing had demuxed a real
  fragment with both a video and an audio `traf` — and A/V sync is unassertable without both on one
  timeline. **Its two tracks deliberately have different durations** (video 0.1001s, audio 0.0929s):
  one stream ending before the other is the realistic case, not a defect, and `av_sync.rs` depends
  on it.

- `four-colors-av1.mp4` — AV1 in MP4 (`av01.*`), the rerun/Chromium four-quadrant test pattern
  (yellow / red / blue / green), copied from `chromium/media/test/data/four-colors-av1.mp4`
  (Chromium, BSD-3 — provenance per the tick-235 observer steer: Chromium test DATA may be copied
  in with attribution; Chromium CODE never). The quadrant colors make a channel swap or a
  scrambled plane read visibly assertable, which is what the av1_decode gate keys on.

- `red-full-range-420-8bpc.avif` / `red-full-range-420-10bpc.avif` — a solid red AVIF still in
  8-bit (decodes) and 10-bit (refused by the `bitdepth_8` build — the graceful-no fixture), from
  `chromium/third_party/blink/web_tests/images/resources/avif/` (Chromium, BSD-3 — test DATA
  with attribution per the tick-235 steer). Solid red makes a channel swap or range error a
  color assert, not a guess.

- `bear-audio-10s-CBR-no-TOC.mp3` — 10 seconds of CBR MPEG-audio with no Xing TOC, and
  `id3_png_test.mp3` — an MP3 whose leading ID3v2 tag embeds a PNG (the tag must be SKIPPED, not
  parsed as sync, and the picture must not be mistaken for audio). Both from
  `chromium/media/test/data/` (Chromium, BSD-3 — test DATA with attribution per the tick-235
  steer). The 10s duration makes the frame-count claim falsifiable against the clock.

- `bear.flac` (raw FLAC), `sfx.ogg` (Ogg/Vorbis mono 44.1k) and `bear-opus.ogg` (Ogg/OPUS — the
  honest-refusal fixture: symphonia has no Opus decoder, so this must fail as a NAMED error, never
  a panic and never a wrong sound), all from `chromium/media/test/data/` (Chromium, BSD-3 — test
  DATA with attribution per the tick-235 steer).

- `red-with-alpha-8bpc.avif` — an AVIF carrying a real alpha AUXILIARY ITEM (8-bit), from the
  Blink AVIF resources (Chromium, BSD-3, same provenance rule). NOTE the trap the first pick hit:
  the `alpha-mask-*` fixtures ARE masks (monochrome primaries, alpha_item=None) — a gate on one
  of those can never see compositing. The alpha gate asserts the A channel actually VARIES and
  the color plane survives compositing.
