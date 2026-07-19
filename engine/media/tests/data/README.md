# Media test fixtures

Two **real** encoded MP4 files, not synthesised ones. That distinction is the point: a fixture
written by our own code proves that our writer and our reader agree, which is a tautology. These
came out of real encoders and carry the box layouts, timescale rounding and sample-flag conventions
that real files have — including the ones that surprised us (see `tests/demux.rs`).

| file | what it exercises |
|---|---|
| `bear-640x360-v-2frames_frag.mp4` | **fragmented** MP4 — `moof`/`traf`/`trun`, the form MSE streams. Video-only, H.264 High (`avc1.64001E`), 640×360, 2 frames at 30000 timescale. |
| `blackwhite_yuv420p.mp4` | **progressive** MP4 — a classic `stbl` sample table with no fragments. H.264, 240×240, 1 sample. |

Both are from the Chromium `media/test/data` corpus, which is BSD-licensed (see the Chromium
`LICENSE`). They are checked in because the tests must run on a clone that has no Chromium tree.
