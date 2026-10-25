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
