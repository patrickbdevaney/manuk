# One connection per origin, and a subresource is fetched once per navigation

> Landed PERF.1.1 (partial: see docs/sprint/PERF.md "PERF.1.1 results"). Gate script: `scripts/perf1-gate.py`.

## Mechanism (engine/net/src/lib.rs, engine/page/src/lib.rs)

- **First request to an origin goes alone** (`origin_probe`): the pooled hyper client opens a connection per request that
  arrives before one exists, so an h2 origin got N sockets (Wikipedia 10, now 3). Followers wait for the leader's connect
  (the `TimedConnector` marks the origin known) for at most `PROBE_WAIT` = 1.5 s, so a dead host cannot serialise a page.
- **h2 origins are not capped at six** (`note_http_version`): the first `HTTP/2.0` answer widens that origin's semaphore to 64.
  An explicit `MANUK_MAX_CONNS_PER_HOST` below 6 is honoured (it is the gate's serialising mutant).
- **Repeat requests inside one navigation reuse the first answer** (`nav_recent_*`, 10 s window, cleared by
  `reset_fetch_stats`): css/js/font/image 200s and 404/410s. `no-cache`+ETag sheets used to be revalidated (a 304) 90 ms after
  being fetched; unvalidated ones were fetched twice at full price. JSON/text bodies are never reused (a page may poll them).
- **@font-face sources fetch concurrently** (`FuturesOrdered`, registration order unchanged); `.eot` sources are skipped
  (undecodable). Each face's own `src` ladder stays ordered.
- Trace `req` lines gained a trailing method column so "requested twice" can ignore POST beacons.

## Limits

An HTTP/1.1 origin is still six-wide, and fonts/images still start only after cascade/layout (a serial waterfall Chrome
overlaps with its preload scanner) - that is the remaining subres_fetch gap.

## PERF.1.1.a — the early scan (fonts and images start at parse time)

`load_async` already warmed external CSS before the parse. It now also warms (a) every `<img>` URL
(srcset/`<picture>` chosen by `select_image_url`, http(s) only, first 48 distinct) right after
`manuk_html::parse`, and (b) each stylesheet's `@font-face` first `src` the moment the sheet text lands
(`warm_font_faces`; skips `unicode-range` subsets and `.eot`). Both only call `manuk_net::fetch`, so the
later `fetch_and_apply_*` passes get the bytes from the HTTP cache or park on the single-flight lock:
nothing is requested twice and nothing is applied early. `MANUK_NO_EARLY_SCAN=1` turns it off (the gate's
mutant arm). Measured paired (`scripts/perf1-ab.py`, interleaved per URL/run): Σ subres_fetch 19.90 vs
23.09 s and 22.56 vs 26.55 s (ratio 0.862, 0.850; the node's 0.8x bar was NOT met).
Gotcha: `resolve_url` percent-encodes the `?` of `x.eot?v=1` so an `ends_with(".eot")` test misses it.
