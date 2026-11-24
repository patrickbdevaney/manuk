# DASH.MEM.1.b — bounding decoded-image memory of manuk-mcp (full profile)

chrome_ver=145.0.7632.116 (n/a: no Chrome in the loop). Driver: `scripts/dash-mem-trace.py`, dashdish-5, under
`systemd-run --user --scope -p MemoryMax=6G -p MemorySwapMax=0`.

## What the 4.5 GB actually was (measured, not assumed)
`MANUK_IMG_LOG=1` (decode_bitmap) names the images: the dashdish home loads **147 images**, ~40 of them
`/_next/image?url=...&w=3840` photos decoded at 3840x2160 (33 MB each; largest 3840x2880 = 44 MB), 1747 MB of RGBA in total.
Three multipliers sat on top of that, each measured by an A/B on the same page:
1. **A second copy in JS.** `publish_image_sources` copied every `<img>` into a premultiplied tiny-skia Pixmap
   (`canvas::publish_source`), for pages that never touch a canvas. Now the page's `Rc<DecodedImage>` is shared and the
   Pixmap is built on first `drawImage` (`RAW_SOURCES`/`source_pixmap`). Load events and sizes are unchanged.
2. **glibc arenas hoarding freed rasters.** Decodes run on blocking-pool threads; per-thread arenas keep every freed
   25-45 MB raster resident. With `MALLOC_ARENA_MAX=1` the page peaked 1349 MB, with `MALLOC_MMAP_THRESHOLD_=262144` 1227 MB
   (vs ~2.9 GB default). manuk-mcp now calls `mallopt(M_MMAP_THRESHOLD=256K, M_ARENA_MAX=2)` at startup (Linux).
3. **Per-image pixel cap.** One decoded image holds at most 1920x1080 px (2 073 600; raster downscaled aspect-kept before the
   RGBA conversion, SVG rasterised at scale). Retained decoded pixels 1747 -> 873 MB. Set by manuk-mcp only; the engine
   default is unbounded (0), `MANUK_IMG_MAX_PX` overrides. Concurrent decodes are bounded to 4 (`MANUK_DECODE_SLOTS`).
Lean profile and images that fit under the cap are untouched (lean never reaches decode_bitmap before a screenshot).

## Gate: start navigate peak RSS, 3 runs (files here)
| arm | files | max RSS |
|---|---|---|
| before (DASH.MEM.1, pre-change binary) | ../dash-mem1/new-dashdish-5-r{1,2,3}.tsv | 4502 / 4569 / 4513 MB |
| **bounded (this change)** | bounded-dashdish-5-r{1,2,3}.tsv | **1272 / 1265 / 1206 MB** (< 1.5 G) |
| mutant: `MANUK_MEM_BOUND=0 MANUK_DECODE_SLOTS=0` (cap + mallopt + slots off, same binary) | mutant-dashdish-5-r{1,2,3}.tsv | 2873 / 2782 / 2848 MB |
| intermediate: cap only, before the other two fixes | capped-dashdish-5-r{1,2,3}.tsv | 4397 / 3944 / 3499 MB |

The gate goes red: the pre-change binary is 4.5 G and the in-binary mutant 2.8 G, both far above 1.5 G. The env mutant
does not reach 4 G because the shared-pixels change (1.) has no switch; the >4 G red arm is the pre-change binary.
Navigate times (bounded 14.5/13.8/14.4 s vs 23.6/23.3/23.5 s before) were not run interleaved or under quiet load: no speed claim.
Screenshots: images that fit under the cap decode as before; images over 1080p-pixels are drawn downscaled-from-1080p instead of
from the full raster (a visible difference only above 1920x1080 source resolution).
