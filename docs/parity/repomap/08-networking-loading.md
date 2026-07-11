# REPOMAP 08 — Networking, Resource Loading & Caching

How production browser engines fetch documents and subresources, cache them, reuse
connections, and — the big latency lever — *discover subresources early* via a
speculative preload scanner, and what a lean from-scratch Rust browser (**Manuk**)
should fold in. All paths are absolute under `/home/patrickd/manuk/`.

The user's complaint is **latency / snappiness**. Networking is where most of a
cold page-load's wall-clock lives, so this doc is organized around the four levers
that production engines pull, in rough order of payoff:

1. **Early discovery** — a *preload scanner* races ahead of the main parser and
   fires subresource fetches before the DOM exists (a full RTT+ saved per resource).
2. **Parallelism + prioritization** — fetch many things at once, but in the *right
   order* (CSS/fonts before images), bounded by socket/stream limits.
3. **Connection reuse** — pooled keep-alive sockets, HTTP/2 multiplexing, HTTP/3,
   and speculative preconnect so the TLS handshake is already paid for.
4. **Caching** — an HTTP cache (memory + disk) so the second visit skips the network
   entirely, plus conditional revalidation (304) so a stale entry costs one small RTT.

---

## 1. Scope & sources

| Engine | Paths |
|---|---|
| **Chromium / Blink** | `chromium/net/http` (HTTP cache, socket pools), `chromium/net/base/scheduler`, `chromium/third_party/blink/renderer/core/html/parser` (**preload scanner**), `.../core/loader` (document/resource loader), `.../platform/loader/fetch` (ResourceFetcher, priorities) |
| **Gecko (necko)** | `firefox/netwerk/protocol/http/nsHttpChannel.cpp`, `firefox/netwerk/cache2` (disk+memory cache, index) |
| **Servo** | `servo/components/net` (fetch spec in Rust — Manuk's closest peer: `fetch/methods.rs`, `http_loader.rs`, `http_cache.rs`, `connector.rs`) |
| **WebKit** | `WebKit/Source/WebCore/loader/cache` (CachedResource memory cache), `.../html/parser/HTMLPreloadScanner.cpp`, `.../platform/network/CacheValidation.cpp` |
| **Ladybird** | `ladybird/Libraries/LibWeb/Loader/ResourceLoader.cpp`, `ladybird/Services/RequestServer` (out-of-process cURL-backed network service) |
| **Manuk (this repo)** | `engine/net/src/lib.rs` (fetch + streaming + preconnect + pooling), `engine/page/src/lib.rs` (subresource fetch: images/fonts/stylesheets) |

---

## 2. Per-engine approach

### 2.1 Chromium / Blink — the reference architecture

Blink runs the most sophisticated loading pipeline in existence; every latency
optimization below originated or was popularized here.

**The preload scanner (the headline innovation).** As raw HTML bytes arrive — even
while the main parser is *blocked* on a synchronous `<script>` — a lightweight
tokenizer scans ahead looking only for fetchable URLs and fires their requests
speculatively.
- `TokenPreloadScanner::Scan()` walks tokens and, per start-tag, a `StartTagScanner`
  decides whether to emit a `PreloadRequest`
  (`chromium/third_party/blink/renderer/core/html/parser/html_preload_scanner.h:111`,
  `.cc:270` `CreatePreloadRequest`). It understands `<img srcset>`/`<picture>`
  (`HandlePictureSourceURL` `:257`, `BestFitSourceForSrcsetAttribute` `:504`),
  `<link rel=preload/preconnect/modulepreload>` (`LinkRelAttribute` `:530-537`),
  `media=` queries (evaluated against `MediaValuesCached` `:156`), and the
  `fetchpriority` attribute (`:446`).
- A nested `CSSPreloadScanner` (`css_preload_scanner.cc`) scans inline/linked CSS for
  `@import` and `url()` so even CSS-referenced assets start early.
- It runs **off the main thread**: `HTMLDocumentParser` owns a `background_scanner_`
  on a dedicated `preload_scanner_thread`
  (`html_document_parser.cc:143`), and `ScanAndPreload()` (`:841`, `:994`) also fires
  a foreground `preload_scanner_` when the parser is paused on a script
  (`:835-841`). Requests flow through `HTMLResourcePreloader`
  (`html_resource_preloader.cc`) → `ResourceFetcher`.
- Payoff: subresource fetches begin ~one full parse-and-execute cycle earlier —
  frequently the single largest cold-load win on script-heavy pages.

**Prioritization.** Every resource gets a `ResourceLoadPriority`
(`platform/loader/fetch/resource_load_priority.h`, 5 levels kVeryLow…kVeryHigh).
`ResourceFetcher::ComputeLoadPriority` (`resource_fetcher.cc:573`) starts from a
type table `TypeToPriority` (`:177`):
- **kVeryHigh**: CSS stylesheets, fonts (render-blocking).
- **kHigh**: scripts, raw/XHR; visible images bumped here.
- **kMedium**: preload-scanner-discovered late scripts/styles, manifest.
- **kLow**: images, media, SVG.
- **kVeryLow**: `rel=prefetch`, speculation-rules, dictionaries.
Then it adjusts for viewport visibility, `<script async/defer>`, `fetchpriority`
hints, and late-body position (`:588-683`). Priority feeds the socket pool and the
HTTP/2 stream-dependency tree.

**HTTP cache** (`chromium/net/http/http_cache.cc`). Backed by a pluggable
`disk_cache::Backend` (`http_cache.h:117`), memory *and* disk. The load-bearing
detail is the **partitioned (double-keyed) cache key**: the key is
`NetworkIsolationKey` (top-frame site + frame site) concatenated with the URL —
`GenerateCacheKeyForRequest` (`http_cache.cc:897`) → `GenerateCacheKeyForRequestWithAlternateURL`
which prepends `network_isolation_key.ToCacheKeyString()` with a `kDoubleKeySeparator`
(`:836-897`) and further tags subframe-document resources (`:861`). This closed a
class of cross-site cache-probing timing attacks; every from-scratch cache must
partition by top-frame origin. Transient/opaque isolation keys are non-cacheable
(`CanGenerateCacheKeyForRequest` `:822`).

**Connection reuse / socket pools.** HTTP/1 is limited to **6 sockets per
destination** via layered socket pools; HTTP/2 collapses all same-origin requests
onto one multiplexed connection (so the 6-limit and request prioritization become a
single stream-dependency tree); HTTP/3 (QUIC) removes head-of-line blocking. A
`ResourceScheduler` / priority task-queue system throttles delayable requests
(`chromium/net/base/scheduler/net_task_scheduler.cc`, priorities in
`net/base/request_priority.h`). Preconnect is driven speculatively by the
`LoadingPredictor` (learned per-origin subresource origins) plus page-declared
`<link rel=preconnect>`.

### 2.2 Gecko / necko — nsHttpChannel + cache2

`nsHttpChannel` is the per-request state machine (`firefox/netwerk/protocol/http/
nsHttpChannel.cpp`): it opens a cache entry (`OpenCacheEntry` `:1496`), and on a
cached-but-stale hit issues a conditional request and handles `304`
(`ProcessNotModified` `:3481`), else dooms the entry on error (`AsyncDoom`).

**cache2** (`firefox/netwerk/cache2`) is a from-scratch (2014) unified disk+memory
cache. Architecture worth studying:
- `CacheStorageService` (`CacheStorageService.h:87`) is the front door; it
  distinguishes **memory-only** vs disk entries (`:65`) and walks them separately
  (`WalkMemoryCacheRunnable` / `WalkDiskCacheRunnable` `:92-93`).
- `CacheIndex` (`CacheIndex.h`) keeps an in-memory hash index (SHA1 of the key,
  `#include "mozilla/SHA1.h"` `:19`) so lookups and eviction (frecency-ranked)
  don't touch disk; `CacheFileIOManager` does async I/O off the main thread on a
  dedicated `CacheIOThread`.
- Each `CacheFile` stores metadata + chunked body (`CacheFileChunk`,
  `CacheFileMetadata`), enabling range reads and partial-content resumption.

### 2.3 Servo — the Rust peer (read this one closely)

Servo implements the **WHATWG Fetch spec** directly in async Rust over the *same
hyper stack Manuk uses*, so it is the single most transferable design.
- **Fetch pipeline**: `fetch()` → `main_fetch()` → `http_fetch()` /
  `scheme_fetch()` (`servo/components/net/fetch/methods.rs:179,400,588,1069`),
  mirroring the spec's algorithm names; a `cors_cache` handles preflight caching
  (`fetch/cors_cache.rs`).
- **Connector** (`connector.rs`): a `hyper_util` legacy `Client` over a
  `hyper-rustls` HTTPS connector with ALPN `h2` (`ALPN_H2` `:41`) — i.e. pooling +
  HTTP/2 for free, exactly Manuk's stack.
- **HTTP cache** (`http_cache.rs`): an **in-memory** cache (`quick_cache`) — Servo
  ships no disk cache. Key is just the URL (`CacheKey` `:39`, `CacheKey::new` `:45`).
  It implements the RFC 9111 machinery Manuk lacks entirely:
  - freshness from `Cache-Control: max-age`/`s-maxage` and `Expires`, with
    **heuristic freshness** from `Last-Modified` for cacheable status codes
    (`:221-273`);
  - `stale-while-revalidate` parsed by hand (the `headers` crate doesn't) with a
    single-flight background-revalidation guard (`revalidation_guard`,
    `revalidate_in_background` `:73-115,283`);
  - `Vary` handling: a `Vary: *` response is treated as uncacheable, otherwise the
    stored request headers must match (`construct_response` `:616-643`);
  - request-side `no-cache`/`max-age=0` force revalidation (`request_demands_revalidation`
    `:308`).
  - `http_loader.rs` wires it in: on a `304` it updates the stored entry
    (`revalidating_flag`, status `NOT_MODIFIED` `:1580`).

### 2.4 WebKit — memory cache + preload scanner, no big disk cache in WebCore

WebKit's WebCore layer keeps a typed **in-memory** `CachedResource` cache
(`WebCore/loader/cache/CachedResource.h`, one subclass per type: `CachedImage`,
`CachedCSSStyleSheet`, `CachedFont`, `CachedScript`, …). `CachedResourceLoader`
(`CachedResourceLoader.cpp`) dedups in-flight requests and *reuses speculative
preloads*: `preload()` (`:1838`) parks resources in `m_preloads` (`:1856`), and a
later real request adopts the already-started preload (`:1501,1544`). Unused preloads
are reaped after DOMContentLoaded (`m_unusedPreloadsTimer` `:1419`). WebKit also has
an `HTMLPreloadScanner` (`WebCore/html/parser/HTMLPreloadScanner.cpp`) + a
`CSSPreloadScanner`, same idea as Blink. HTTP-cache validation logic (freshness,
conditional headers) lives in `platform/network/CacheValidation.cpp`; the actual
disk cache lives in the network process (`WebKit/NetworkProcess/cache`, not WebCore).

### 2.5 Ladybird — out-of-process cURL network service

Ladybird splits networking into a separate **RequestServer** process
(`ladybird/Services/RequestServer`) backed by libcurl (`CURL.cpp`), talking to
`LibWeb` over IPC. `ResourceLoader::load()`
(`ladybird/Libraries/LibWeb/Loader/ResourceLoader.cpp:386`) sends the request with a
`cache_mode` and unbuffered streaming callbacks (`set_unbuffered_request_callbacks`
`:492`, including an `OnCachedBodyAvailable` path for cache hits). It exposes an
explicit **connection cache warmer**: `ResourceLoader::preconnect()`
(`ResourceLoader.h:47`) → `ensure_connection(url, CacheLevel::ResolveOnly |
CreateConnection)` (`ResourceLoader.cpp:93,108`) — a two-tier preconnect (DNS-only
vs full socket). Caching/pooling largely delegate to curl. Ladybird has a
`rel=preload` link path but no full speculative preload scanner (the parser-level
one Blink/WebKit have) in the surveyed tree.

---

## 3. Manuk today (honest assessment)

**What exists** (`engine/net/src/lib.rs`):
- A process-global pooled `hyper_util` `Client` over a `hyper-rustls` connector with
  ALPN `h2,http/1.1` (`connector()` `:88`, `client()` `:103`). This gives, for free:
  **connection pooling** (~90s keep-alive → sequential same-origin fetches skip the
  TLS handshake), **HTTP/2 multiplexing**, and Happy-Eyeballs dual-stack connect.
- `fetch()` (`:264`) GET-with-redirects (buffers whole body); `fetch_streaming()`
  (`:309`) delivers `Content-Encoding`-decoded chunks to a sink as they arrive off the
  socket — the enabler for feeding `manuk_html::StreamParser` incrementally.
- `Preconnector` (`:369`): speculative TCP+TLS warming on link hover, **same-origin +
  user-gesture only** for privacy (`classify` `:394`), bounded by an in-flight cap
  and a 10s idle window; shares the connector so the warmed TLS session resumes.
- Content-encoding decode (gzip/br/deflate) and WHATWG charset sniffing.

**What page-level loading does** (`engine/page/src/lib.rs`):
- `fetch_streaming_page()` (`:928`) streams the *main HTML document* into an
  incremental parser — good.
- BUT subresources are fetched **after** the document parse, in **serial `await`
  loops**: `fetch_and_apply_stylesheets` (`:660`) awaits each external CSS one at a
  time (`:672`), then each `@font-face` source one at a time (`:687`);
  `fetch_images` (`:81`) awaits each `<img>` one at a time (`:99-100`). Each
  `manuk_net::fetch` also **buffers the full body** before returning.

**Gaps, ranked by latency impact:**
1. **No preload scanner.** Subresources are discovered only after the DOM is built,
   not during byte arrival. On a page with N stylesheets/fonts/images this adds
   roughly one document-parse of latency before *any* subresource request leaves.
2. **Serial subresource fetching.** The `await` loops fetch one resource at a time;
   with pooling+H2 the connection can carry many in parallel, but the code never
   issues them concurrently. This is the biggest *immediately fixable* win.
3. **No HTTP cache at all.** Every navigation re-fetches everything over the network
   — no memory cache, no disk cache, no conditional revalidation (304), no
   `Cache-Control`/`Expires`/`ETag`/`Last-Modified` handling. Repeat visits and
   back/forward pay full cost.
4. **No request prioritization.** CSS/fonts (render-blocking) race images with no
   ordering; on H2 they share a connection with no stream priority.
5. **Preconnect is narrow.** Only same-origin, user-gesture; page-*declared*
   `<link rel=preconnect>`/`rel=preload` to subresource origins aren't acted on (a
   solicited, privacy-safe path Manuk could honor).
6. No HTTP/3 (QUIC) — lowest priority; H2 covers the common case.

---

## 4. Fold-in recommendations (ranked by latency leverage)

**What reqwest/hyper already gives us — do NOT rebuild:** connection pooling,
keep-alive, HTTP/2 multiplexing, Happy Eyeballs, TLS session resumption, chunked
streaming, content-encoding. Servo proves this exact stack (`hyper_util` +
`hyper-rustls`) is production-viable. **Bloat to avoid:** a custom HTTP/1+2 stack, a
bespoke socket-pool layer, and — initially — a full disk-cache infrastructure
(mmap'd chunk files, an on-disk index, frecency eviction à la necko cache2). Those
are large surface areas for small marginal gains on a lean browser.

**Ranked plan:**

1. **Parallelize subresource fetching (highest leverage, smallest change).** Replace
   the serial `await` loops in `fetch_and_apply_stylesheets` (`page/src/lib.rs:667`)
   and `fetch_images` (`:99`) with concurrent issue — collect the URLs, spawn one
   `manuk_net::fetch` future per URL, and `futures::future::join_all` (or a bounded
   `buffer_unordered(6)` to respect per-host etiquette). The pooled H2 client already
   multiplexes them onto one connection. This alone should turn N serial RTTs into ~1.

2. **Build a preload scanner (highest *structural* win).** Manuk already streams
   bytes into `StreamParser`. Add a cheap tokenizer pass (or hook the existing
   streaming tokenizer) that, as chunks arrive, extracts `<link rel=stylesheet>`,
   `<script src>`, `<img src/srcset>`, `@import`/`url()` in CSS, and
   `<link rel=preload/preconnect>`, and *immediately* kicks off (parallel) fetches
   into a small in-memory pending map keyed by URL. When the real DOM node is reached,
   adopt the in-flight/completed fetch instead of starting a new one (WebKit's
   `m_preloads` adoption model, `CachedResourceLoader.cpp:1501`, is the pattern). Model
   on `html_preload_scanner.cc` but keep it minimal — no need for the off-main-thread
   background thread initially; a synchronous scan of each arriving chunk on the fetch
   task is enough for a single-page load.

3. **Add an in-memory HTTP cache (skip disk first).** Port Servo's `http_cache.rs`
   design nearly verbatim: a `quick_cache`/`HashMap` keyed by URL, storing body +
   headers + computed freshness. Implement RFC 9111 essentials — `max-age`/`Expires`
   freshness, heuristic freshness from `Last-Modified`, `ETag`/`Last-Modified`
   conditional revalidation (`If-None-Match`/`If-Modified-Since` → handle `304`),
   `Vary` matching, and `no-cache`/`no-store`. **Partition the key by top-frame
   origin from day one** (Chromium's double-key, `http_cache.cc:897`) — retrofitting
   partitioning later is painful and it's a real privacy/security property. This makes
   repeat visits and back/forward nearly free. A disk tier can come much later (or
   never, for a lean browser).

4. **Basic prioritization.** Even without H2 stream-priority plumbing, issue
   render-blocking resources (CSS, fonts) *before* images from the preload scanner's
   queue — a two-bucket "blocking vs non-blocking" split captures ~80% of Blink's
   `TypeToPriority` benefit (`resource_fetcher.cc:177`) for near-zero complexity.

5. **Honor page-declared preconnect/preload.** Extend the existing `Preconnector`
   to act on `<link rel=preconnect>` found by the preload scanner (this is
   *solicited* by the page, so the same-origin privacy restriction that guards
   *speculative* hover-preconnect can be relaxed for declared hints). Cheap, and
   overlaps DNS+TLS with parsing.

6. **(Later) HTTP/3.** `hyper` doesn't do QUIC; would need `quinh`/`h3` or switching
   to `reqwest` with the `http3` feature. Defer — H2 + pooling covers the common case
   and this is a large dependency for a marginal tail-latency gain.

---

## 5. Open questions for frontier research

- **Preload scanner vs a lean streaming parser: how much duplication?** Blink runs a
  *separate* speculative tokenizer because its main parser is heavyweight. Manuk's
  `StreamParser` is already incremental and cheap — could a single pass emit
  fetch-intents inline (parse-once) rather than maintaining a second scanner, without
  the correctness hazards (document.write, `<base>` retargeting, CSP nonces) that
  forced Blink to keep them separate? See the CSP-nonce caveat at
  `html_preload_scanner.cc:230`.
- **Cache partitioning granularity.** Double-key (top-frame + resource) is the
  Chromium standard, but triple-keying and `no-vary-search` are evolving. What's the
  right default for a privacy-forward lean browser — and does partitioning interact
  badly with a purely in-memory cache's low hit-rate?
- **Speculative preconnect without a learned predictor.** Chromium's `LoadingPredictor`
  learns per-origin subresource origins from history. Without that ML/history layer,
  how much preconnect value comes purely from parsing declared hints + same-origin
  warming — and is a tiny per-origin frequency table worth the privacy cost of
  persisting it?
- **Prioritization on a single H2 connection.** With everything multiplexed on one
  socket, does issue-order alone suffice, or is explicit H2 stream-dependency/weight
  plumbing (which `hyper` doesn't expose ergonomically) worth the effort for
  render-blocking-first ordering?
- **Streaming into layout, not just parse.** Manuk streams into the parser and does a
  head-complete first-layout checkpoint; how early can *subresource-dependent* layout
  (image intrinsic sizes, font metrics) begin without thrashing — i.e. the tradeoff
  between more incremental layouts and fewer, later, cheaper ones?
