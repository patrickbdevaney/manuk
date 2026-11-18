# W4.E1.b.b.a — lean probe: one request per image URL (chrome_ver=145.0.7632.116)

Cause (from the W4.E1.b.b traces): `lean_image_dims` asked for a 2 KB range, then (header not in 2 KB) a second request for 16 KB,
then (SVG / header past 16 KB) a third request for the whole body. Same URL, up to 3 requests (SVG: 3, large-EXIF JPEG/WebP: 2).
Fix: `manuk_net::fetch_prefix_until` = ONE ranged request (128 KB cap) read in 2 KB steps, stopped as soon as the header gives a size;
if the whole body arrived (< cap) and has no readable size (SVG) it is decoded from the bytes in hand.

Paired A/B, both arms `MANUK_AGENT_PROFILE=lean`, A = pre-fix binary (/tmp/a11y-dump.w4e1bb, built at HEAD of W4.E1.b.b), B = fixed, 3 runs
each, mobile.bg + puentedemando (`urls.txt`, `run.log`, `paired.jsonl`, `traces/`). Checker: `scripts/w4e1bba-dup-check.py` (`verdict.txt`).

| URL | A req/unique | B req/unique | image bytes read A / B (median) |
|---|---|---|---|
| mobile.bg | 44/30 | 30/30 | 75 422 / 74 872 |
| puentedemando | 70/54 | 54/54 | 377 809 / 187 111 |

Bytes: A = declared length of 206 responses + 200 bodies (A read all of those); B = bytes actually read (new `prefix` trace line) + 200 bodies.
Caveat: B stops reading early; on HTTP/2 the server may already have sent more than we read before the stream reset reached it, so B's
true wire bytes can exceed the "read" figure. The one-sided 10% bound (B <= 1.10 x A) holds on read bytes; a wire-level check is follow-up W4.E1.b.b.a.a.
Red proof: arm B := pre-fix binary (`--arm-a manuk --arm-b manuk`) -> exit 1 "fixed arm still duplicates"; `--self-test` OK.
