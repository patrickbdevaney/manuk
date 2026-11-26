# CPU.4.b — w3c navigate under manuk-mcp: where the 'initial images+masks' CPU went — chrome_ver=145.0.7632.116

Live network: images are fetched from www.w3.org, so the page is not frozen. Host shared, load1 ~16 while measuring.

CAUSE: manuk-mcp sets the per-image decoded-pixel cap to 1080p (`set_decode_cap_px(2_073_600)`, DASH.MEM.1.b; a11y-dump does not). The two
1920x1280 unsplash JPEGs exceed it and were downscaled to 1763x1175 with `image::DynamicImage::resize_exact(Triangle)`, ~0.15 cpu-s each on a
tokio blocking worker. Not JPEG decode (21 ms each), not SVG raster (4-9 ms each), not spinning. a11y-dump never hit the resize, which is the "4x".
Isolation: old binary with `MANUK_IMG_MAX_PX=0` (cap off) = 0.275-0.289 cpu-s total (5 runs) vs 0.59-0.68 with the cap.

## The image set (identical in every run of both arms; `ab.tsv` col 6 is one string for all 14 runs)
9 images were decoded (the node note said ~13; the other references were not decoded in these runs, reason not investigated). Per-image decode cpu-s from this thread's
`/proc/thread-self/schedstat` (new `MANUK_IMG_LOG` fields `src_bytes`, `cpu_ms`):

| url (https://www.w3.org...) | src bytes | decoded | cpu ms, cap off | cpu ms, new area-average cap |
|---|---|---|---|---|
| /assets/website-2021/svg/avatar.svg | 464 | 512x512 | 4.9 | 4.7 |
| /assets/website-2021/svg/search.svg | 425 | 512x512 | 5.9 | 5.0 |
| /assets/website-2021/svg/github.svg | 1384 | 496x512 | 4.8 | 5.1 |
| /assets/website-2021/svg/mastodon.svg | 695 | 448x512 | 4.0 | 4.1 |
| /assets/logos/w3c-2025/svg/w3c.svg | 42416 | 480x480 | 9.5 | 9.0 |
| /cms-uploads/banner1.jpg | 93850 | 786x486 | 4.4 | 4.8 |
| /cms-uploads/w30c-pattern.jpeg | 942641 | 1500x1000 | 23.1 | 22.6 |
| /cms-uploads/ryan-quintal-US9Tc9pKNBU-unsplash.jpg | 279145 | 1920x1280 -> 1763x1175 | 21.1 | 70.6 |
| /cms-uploads/marvin-meyer-SYTO3xs06fU-unsplash.jpg | 292700 | 1920x1280 -> 1763x1175 | 21.1 | 69.1 |

(Old Triangle resize is not in this table: the old binary has no cpu_ms field; its cost is the 0.59-0.68 vs 0.28 totals above.)

## Fix
`decode_bitmap_uncapped` (engine/page/src/lib.rs): replace `resize_exact(Triangle)` by `box_downscale`, an alpha-weighted area average, one
pass over the source. Same output dimensions (the cap/aspect rule is unchanged), unit test `cpu4b_box_downscale`.

## Result (`ab.tsv`, 7 interleaved rounds old vs new binary, wait4 cpu-s, w3c navigate+observe)
old median 0.608, new median 0.384 (-37%); tokio worker cpu 0.43 -> 0.21; CPU.4 baseline median 0.63 (cpu4/totals.tsv). Observe text
(port masked) sha identical in all 14 runs. `python3 gate.py` ok; `python3 gate.py --mutant` exits 1.
Not measured: Chrome for this change; image-quality comparison of area-average vs Triangle (both are smoothing filters; no screenshot gate was run).
Remaining 0.38: the box downscale still costs ~48 ms per big photo (vs 21 ms decode); cap-off total is 0.28 — see follow-up CPU.4.b.a.
