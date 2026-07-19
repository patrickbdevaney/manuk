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
