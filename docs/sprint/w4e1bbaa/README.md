# W4.E1.b.b.a.a — true wire bytes of the lean image-size probe (chrome_ver=145.0.7632.116)

Question: W4.E1.b.b.a's one-request probe (128 KB range, early stop) counted "bytes read"; the server may have sent more. Measured two ways, same HEAD tree,
only `lean_image_dims_uncached` differs between arms (prefix-head = old 2 KB/16 KB/full probe, oneshot = 128 KB single range, fixed = this node).
- Real interface rx (/proc/net/dev eno1 delta per run, interleaved, 3 runs; `real-rx.txt`, `real-rx32.txt`). Medians vs prefix-head:
  oneshot 128 KB: mobile.bg 1.343, puentedemando 1.307 (RED, > 1.10). 2 KB + 128 KB continuation: 0.987 / 1.069. 2 KB + 32 KB continuation (landed): ~1.00 / ~1.00 (real-rx32.txt; site content varied between sessions, arms interleaved).
- Loopback HTTP/1.1 server logging bytes it wrote (`scripts/w4e1bbaa-wire-check.py`, 40 images, 25% with a >16 KB EXIF block; `loopback.txt`): oneshot 1.859, 2K+128K 1.363, 2K+32K 1.079 (<= 1.10); prefix-head reproduces its own count (2078020 twice).
Fix: first request 2 KB range; only if no size in it ONE continuation, 32 KB range read in 2 KB steps with early stop; still no size -> the old full-fetch fallback. Trade: images whose header is past 2 KB again cost up to 2 requests (was <=3 before W4.E1.b.b.a, 1 after); duplicates are no longer 0 there.
Not measured: HTTP/2 on loopback (no TLS test server); real-site rx covers h2.
