# NETWORKING — how real sites actually load

## Count the WIRE, not the call

`NET_REQUESTS`/`NET_DUPES` must be incremented where the byte leaves the process, not where the fetch
function is entered. A dedupe layer that counts its own *calls* reports a perfect cache-hit rate while
hammering the network.

## The three mechanisms that actually cut load time, and what each one is for

| Mechanism | Fixes |
|---|---|
| **URL-keyed cache** | the same URL fetched twice by two different subsystems |
| **`INFLIGHT` single-flight coalescing** | the same URL fetched twice *concurrently* — a cache alone never sees the second one, because the first has not landed yet |
| **Per-navigation negative cache (`FAILED`)** | **the one everybody forgets.** A skip-list built from *successes* never remembers a *failure*, so a 404'd asset is retried by every subsystem that wants it. nytimes: **813 fetches, 507 of them duplicates.** |

## The page's own `fetch()`/XHR must actually be performed

They were queued and never made outside the shell — the oracle, `boxes`, and the agent all silently
dropped them. Likely a large share of the oracle's "missing nodes": a page that fetches its content and
never receives it renders its skeleton, which is exactly what a hydration failure looks like.

## A page's `fetch`/XHR **request headers** must reach the wire — dropping them is a silent 401

`fetch(url, {headers})` and `xhr.setRequestHeader(...)` were both no-ops: the JS surface collected no
headers, the pending-request string carried none, and the host hard-coded `Content-Type:
application/json` for every non-GET while sending **nothing** else. So an authenticated request
(`Authorization: Bearer …`) left as an anonymous one and came back 401 — and the page's `.catch`/`onerror`
made it look like a network fault, not a dropped header. Every token-auth SPA read, every OAuth token
exchange, every `application/x-www-form-urlencoded` form-POST failed this way, invisibly.

The fix threads headers end-to-end. JS `__encHeaders` flattens the three shapes a page passes (plain
object, `[name,value]` array, `forEach(value,name)` Headers-like) into `name\x02value\x02…`, appended to
the pending string as `id\x01kind\x01method\x01url\x01headers\x01body` (**body stays the greedy tail** so
it may still contain `\x01`). `drain_pending` parses it back to `Vec<(String,String)>`; the host replays
it onto `manuk_net::request`, defaulting `Content-Type` **only when the page did not set one** (overriding
an explicit form encoding is its own bug). A GET *with* headers is routed through `request` too, not the
cache-carrying `fetch` path — an `Authorization`-bearing GET is not safely shareable across auth contexts.
Response headers are still a stub (`headers.get() -> null`); that is the next half.

## A load budget must be a HARD deadline, not a between-phases courtesy

Checking the clock only *between* phases lets a phase start with a millisecond left and then run for its
full per-request timeout — a 12s budget delivered **21.6s**. Wrap the whole sequence.

Cancelling mid-phase is safe **by construction** only if each phase fetches everything it needs and
*then* applies it to the DOM: a dropped future loses that phase's *enhancement*, never a half-mutated
document.

## Speed is only real if it comes from doing the same work BETTER

*"Fast because we never loaded the images"* and *"fast because we never ran the script"* are two lies
already told and caught here. **A speed claim is only admissible next to a coverage number** — which is
why `crawl-report.sh` prints coverage first and has **no flag to print speed alone.**

---
# Backfill — mechanisms recovered from ticks 1–42 (pre-wiki)

## A browser with NO network timeout is hostage to every dead tracker

`grep -i timeout` over the net layer returned **zero lines**. One subresource that completed its TCP
handshake and then went **silent** stalled the page's `join_all` until the **kernel** gave up — *minutes*,
with the tab frozen. **This is the ordinary condition of the real web**: ad hosts, trackers, beacons and
geoblocked CDNs blackhole connections constantly.

**The contract, not the number:** *the document is what the user came for; a subresource is an
enhancement, and an enhancement that does not arrive in time is dropped.*

**Three deliberately-DIFFERENT deadlines:**

| Deadline | Why |
|---|---|
| **per-request (~8s)** | no single fetch is unbounded |
| **per-page (~12s, HARD)** | the phases are **serial by necessity** (a stylesheet can add an image; a script can add a stylesheet), so six phases × one timeout each is still a **~64s frozen tab** |
| **~30s for the DOCUMENT** | ⚠ **the asymmetry is the point.** Bounding it at the subresource deadline trades *"some sites hang"* for *"some sites are UNREACHABLE"* — a slow-but-healthy origin simply fails to open. **That is a second bug, not a fix.** |

Measured: w3schools **37,786ms → 15,062ms** (Chromium 15.2s), **and structural coverage went UP, 95.7% →
100%** — *the stalls were losing elements too.*

**A budget must be a HARD deadline, not a between-phases courtesy.** Checking the clock only *between*
phases lets a phase start with a millisecond left and run for its full per-request timeout: a 12s budget
delivered **21.6s**. And **a budget that covers one of two identical phases is decorative** —
`finish_loading` ran under the budget while `load_async`, which runs *the same two phases*, had **none at
all**.

## Classic scripts require ordered EXECUTION, not ordered FETCHING

A `for` loop awaiting each `<script src>` before starting the next cost bbc.co.uk **9.3 seconds of pure
waiting**. **Browsers have fetched these concurrently since the 2000s.** Fetch in parallel; execute in
order.

## 74–79% of a page's fetches were the same URL fetched again

Re-running subresource phases after each round of dynamic scripts re-fetched everything from scratch:
**apple.com 282 fetches for 58 distinct URLs (79% duplicate); bbc 484 for ~124 (74%)**. Per navigation,
bbc.co.uk also ran **9 full-document layouts and 4 full cascades**.

**The HTTP cache made them *cheap*, which is exactly why nobody noticed** — *cheap is not free*, and each
still costs a body clone of a multi-megabyte script.

**Four traps in one area:**

1. **Keying the image cache by `(node, url)`** still fetches one sprite **nine times** if nine elements
   name it. **Key by URL.**
2. **A skip-list built from SUCCESSES never remembers a FAILURE** — every blocked tracker and 404 is
   re-fetched on every round. **A news front page is MADE of images that fail:** nytimes **813 fetches, 507
   duplicate**; theguardian **431 of 576 (75%)**. **Remember the ATTEMPT, not the win.**
3. **The preload scanner and the loader race for the same stylesheet** — you need **single-flight
   coalescing**, because a cache alone never sees the second request (the first has not landed yet).
4. **Count the WIRE, not the call.** A dedupe layer that counts its own *calls* reports a perfect
   cache-hit rate while hammering the network. *A repeat call served from cache costs nothing; a repeat
   network request costs the user money on a metered link.*

Result: theguardian **19,175ms → 3,110ms**; nytimes 863 calls but only **335 network requests, 4 duplicate**.

## Keep-alive connections DIE with the Tokio runtime that spawned them

A process-global pooled `hyper` client's idle connections are driven by the runtime that created them.
**Building a fresh multi-thread runtime per navigation killed every warm connection**, so **every
navigation re-did DNS + TCP + TLS from cold.** *One runtime for the process lifetime.*

This is also the prerequisite for parallel subresource fetching: **without a shared pool, parallelism just
opens more cold connections.**

## Preconnect only pays off if it warms the SAME rustls session cache the navigation uses

A preconnect is a **TCP+TLS handshake only, no HTTP request**. The mechanism that makes it worth anything:
the connector is a **shared `OnceLock` cloned into BOTH the pooled client and the preconnector**, so the
speculative handshake populates **the same rustls session cache** the real navigation resumes from.

Constraints from Chrome's own experience: it **reaps idle preconnected sockets at ~10s** and warns that
**over-preconnecting saturates the pool**. And because preconnect **leaks navigation intent** (DNS+TCP+TLS),
it must be **user-initiated and same-origin only**.

## Stream hyper bodies with `BodyExt::frame()`, never `collect()`

`collect()` **buffers the whole body and destroys streaming**. Feed each `Frame`'s `Bytes` straight to
`parser.process(chunk)`, with `async-compression` wrapping the chunk stream **before** the parser for
`Content-Encoding`.

## `SameSite` is ASYMMETRIC — and getting it backwards ships the exact CSRF it prevents

- A **subresource** is judged cross-site against the **current top-level site**.
- A **top-level navigation** must be judged against the **INITIATOR** — because *the destination BECOMES
  the top level.*

**Comparing a navigation against the current top-level site would ship `SameSite=Strict` cookies on a
cross-site link click — precisely the case `Strict` exists to prevent.**

**Cookie rules that carry real security weight:** `Domain` **may not be a public suffix** (`Domain=com` —
the classic supercookie hole; **an unknown suffix must FAIL CLOSED**); **an insecure origin may not SET a
`Secure` cookie** ("Leave Secure Cookies Alone" — otherwise plaintext HTTP overwrites the HTTPS session);
`Max-Age` beats `Expires`; send-order is longest-path-then-oldest; **path matching respects segment
boundaries** (`/application` does **not** match `Path=/app`).

**And cookies must be attached INSIDE the redirect loop**, not only at the initial send site — `Set-Cookie`
must be stored before the next hop and the `Cookie` header recomputed per new host, **or a login that
redirects loses its session.**

**The live jar path enforces this only where it has request context — the page's own `fetch`/XHR.** For
a long time the asymmetric logic above lived in `storage.rs` with **zero callers**: the network's real
cookie attachment (`send_once`) called `jar.cookie_header(url)`, which judges by host alone and so shipped
**every** cookie — `Lax` and `Strict` included — on a *cross-site* `fetch()`. A page on `evil.example`
doing `fetch("https://bank.example/api")` got the bank's session cookie attached **and** read the
response: the exact CSRF/credential-leak `SameSite` exists to stop. The fix threads the **initiator** (the
page's document URL — available at the `finish_loading` fetch chokepoint as `self.final_url`) down to
`send_once`; a script-initiated request is never a top-level navigation, so `cookie_header_subresource`
withholds `Lax` **and** `Strict` cross-site and sends only `SameSite=None`. Same-site (by registrable
domain, so `app.bank.example` → `bank.example` is same-site) is unchanged. Document navigations and
subresource loads still pass `initiator = None` (the flat jar) — wiring their context is the follow-on;
this tick closes the readable-response `fetch`/XHR vector, which is the one that leaks data to script.

## `__Host-`/`__Secure-` name prefixes are a promise the client must KEEP, or it is worse than useless

A server that names its session cookie `__Host-sid` is opting into an integrity contract: *"reject this
unless it could not have been planted by a sibling-subdomain or plaintext-HTTP attacker."* A client that
stores a prefixed cookie whose preconditions do not hold has silently defeated the exact defence the name
requests — so the enforcement is not optional polish, it is the whole point of the feature (RFC 6265bis
§5.5). Enforce at `parse_set_cookie`, the ONE chokepoint both the network `Set-Cookie` path and
`document.cookie` writes funnel through:

- **`__Secure-`** → require the `Secure` attribute.
- **`__Host-`** → require `Secure` **and** host-only (**no `Domain` attribute**) **and** resolved
  `Path` exactly `/`. The Domain ban is what makes it un-forgeable from a sibling subdomain; the `Path=/`
  makes it un-shadowable by a narrower-path cookie.

Two traps: the prefix match is **case-insensitive** (`__hOsT-` still binds), and the `Path=/` test is
against the **resolved** path — a prefixed cookie with no explicit `Path` set from a deep URL inherits a
non-root default-path (`/app`) and must be dropped, not silently accepted.

## Two proxy details that are the difference between privacy and a leak

1. **Hand the target host to SOCKS5 as a DOMAIN NAME, never resolved locally** — so DNS happens at the
   proxy and does not leak the destination.
2. **A dead proxy must ERROR, never silently fall back to a direct connection** — a silent fallback leaks
   the user's IP, *which is the exact thing the proxy was for.*

## Charset sniffing is a definitive reuse

**`encoding_rs` is Gecko's** WHATWG-Encoding implementation (used in Firefox since 56); **`chardetng` is
Firefox's** fallback detector. Only the **orchestration** is hand-rolled, in WHATWG order: **BOM (peek 3
bytes) → HTTP `Content-Type` charset → `<meta>` prescan of the first 1024 bytes → chardetng → default.**
Content-decoding (gzip/br/deflate) **precedes** charset decoding.

## A download STREAMS to disk; only a document is buffered

**A document is bounded; a download is not — and they must not share a memory model or a deadline.** The
navigation entry point `fetch_document_or_download(url, dir)` decides between them **from the response
headers, before touching the body** (`is_attachment` on `Content-Disposition` / a non-renderable
`Content-Type`). A document is buffered (correct — HTML is small) and cached; a download is streamed
**decoded, chunk-by-chunk (64 KiB), straight into a `<name>.part` file** that is atomically renamed on
completion, so the file **never exists whole in RAM** and a half-written download never appears under its
real name.

**Two deadlines, not one.** The header/connect phase and the *document* body read share one `timeout_at`
deadline (`document_timeout`, ~30s) — a slow-but-alive server must not hang the tab, which is the Bar-0
reason that deadline exists. The **download body is deliberately let out from under it**: a multi-GB
transfer taking minutes is *correct*, not a hang. The old path applied the 30s document deadline to the
whole download and buffered it in a `Vec<u8>` — so a large file OOM'd or was killed mid-transfer and
surfaced as a network fault. That is the defect this closes.

**What the streaming path must NOT drop, because the buffering path did it for free.** Splitting header
detection from body consumption means re-doing, not skipping, everything `send_once`/`fetch_inner` gave the
buffered fetch: the **HTTP cache** get/put, the **wire-request accounting** (`NET_REQUESTS` + the dedup set
`G_DEDUP` reads), and **cookie carry + `Set-Cookie` storage**. The last is the trap — `send_raw` (unconsumed
body) has *no* cookie behaviour, so the streaming path uses `send_raw_with_cookies`, which attaches the
jar's `Cookie:` header and stores any `Set-Cookie` (login persistence) exactly as `send_once` does, but
leaves the body unconsumed to stream. Miss it and a logged-in navigation drops its session cookie.

## HTTP/2's win is gated on SUBRESOURCES, not on the document fetch

Advertising `h2` over ALPN is cheap and correct — but **multiplexing's payoff is real only with many
same-origin subresources.** A top-level-document-only fetch gains little over HTTP/1.1 keep-alive.

## The bot-wall fingerprint surface is known — and matching it IS the evasion

CDNs key on **TLS ClientHello (JA3/JA4)**, the **HTTP/2 fingerprint** (SETTINGS / pseudo-header / priority
order), **header order and casing**, `User-Agent`, and JS-environment probes. Tooling that matches these is
**by definition bot-evasion**.

**Two facts make an honest line drawable:** `Mozilla/5.0` is a **universal legacy token every browser
sends** (a "modern browser" marker, not impersonation), and **Cloudflare DOCUMENTS the
`cf-mitigated: challenge` response header** specifically so legitimate clients can *detect* a challenge — a
first-party signal, so **honest detection is the opposite of reverse-engineering.**

> **The strategic argument, not merely the ethical one: impersonation substitutes for the very capability
> that closes the coverage gap.**

## Native `<form method=post>` submission — POST navigation, and POST→redirect→GET

A GET form builds a query URL (`forms::submission_url`); a **POST form builds a request body** and
navigates by POSTing it — the classic login/signup/checkout that isn't JS-intercepted. Before T4 this
was a logged no-op ("method=post is not implemented — nothing was sent"), so those forms did nothing.

**The three pieces, all pre-existing but unwired:** `forms::urlencoded_submission` encodes the
successful controls as `application/x-www-form-urlencoded` into the **body** (never the URL — a
password in a query leaks into history and `Referer`); `net::post_document` POSTs under the document
deadline; `page::prefetch_document_post` runs the same off-thread subresource prep as a GET nav so the
page swaps in identically.

**POST→redirect→GET is the load-bearing detail.** Server login flows almost universally answer a
successful POST with a `303`/`302` to a dashboard (the PRG pattern — so a browser Back doesn't
re-submit). `post_document` therefore follows a `3xx` as a **GET of its `Location`**, and the
redirected page is what renders. Miss this and every login "works" but shows a blank 303 body.

**Cookies must flow across the redirect.** A POST navigation is **top-level**, so it uses the flat
cookie jar (`initiator=None`): the `Set-Cookie` the login response set is stored and then attached to
the followed GET — which is the entire point, or the user lands logged *out*. (Cross-site
POST-navigation `SameSite` — withholding `Strict` — is the follow-on; a same-site login is correct.)

**A file-input form is refused here, LOUD.** `urlencoded_submission` cannot carry file bytes, so a
`multipart/form-data` upload goes through the OS file-picker path (`forms::multipart_submission`), not
a urlencoded POST that would silently drop the files.

## Cross-site POST navigation withholds Lax/Strict (the form-POST CSRF defence)

A native `<form method=post>` navigation is a **top-level navigation with an *unsafe* method**, so
its `SameSite` rule matches a subresource's, not a top-level GET's: a **cross-site** POST withholds
both `SameSite=Lax` and `SameSite=Strict` (only `None` crosses). This is the CSRF defence — it stops
`evil.example` auto-submitting a form POST to `bank.example` with the victim's session cookie.

`post_document(url, ct, body, initiator)` threads the **submitting page's URL** as the initiator
(captured before the URL bar repoints) and hands it to `send_once`, which applies
`cookie_header_subresource` — same-site (incl. subdomains) sends everything so the ordinary login
lands logged in; cross-site sends only `SameSite=None`. The POST→redirect→GET follow stays flat-jar:
a top-level GET is `Lax`-eligible, so the dashboard the login redirects to is logged in even when the
action host differs from the form host. (Lax is sent on *safe* top-level navigations; withheld on the
unsafe POST — that asymmetry is the whole point.)

## CORS is a READ barrier, and a missing check leaks every cross-origin body

`SameSite` decides which *cookies* ride a request; **CORS decides whether the page may READ the
response** to a cross-origin `fetch()`/XHR. They are different halves of the same-origin policy and
both must hold. For a long time only the first existed here: the shell's `pump_fetches` performed the
request and handed the body straight back to the page regardless of origin, so
`fetch("https://api.other.example/data")` from `https://app.example/` **always** resolved with the
body — a cross-origin read the server never opted into. Chromium blocks exactly this.

`net::cors::fetch_response_readable(page_origin, request_url, response_headers, with_credentials)` is
the decision, and it is a **pure function** (the whole gate is unit tests, no socket): same-origin is
always readable; cross-origin is readable only when the response's `Access-Control-Allow-Origin`
(ACAO) opts in — `*` for an **uncredentialed** request (the wildcard may **not** carry credentials),
or a byte-exact echo of `page_origin`; a **credentialed** cross-origin read additionally needs
`Access-Control-Allow-Credentials: true`; a **missing/blank ACAO blocks** (silence is not consent).
Origins are compared as serialized tuples (`scheme://host[:port]`, default port omitted), so a
scheme, host, **or** port difference is cross-origin, and an opaque origin (`data:`) fails closed.

**A blocked read is a network failure, not an empty success.** `pump_fetches` settles the request
with `status 0`, which the JS glue turns into a rejected `fetch()` Promise (`TypeError: Failed to
fetch`) — the same shape Chromium produces — so page error-handling runs instead of the page seeing a
silently-empty body. A cross-origin request also now carries an `Origin` header (as every browser
does), so a server doing *reflective* ACAO can echo it and the read is allowed. The check fires
**only in `pump_fetches`** (script-issued subresources); top-level document navigation is
cross-origin by nature (that is what a link is) and is untouched. Default `fetch()` credentials mode
is `same-origin`, so a cross-origin request is modelled as **uncredentialed** (`with_credentials =
false`); a per-call credentials mode and the CORS **preflight** (the `OPTIONS` round-trip a non-simple
request must pass *before* it is sent) are documented follow-ons — this is the response read barrier,
the part whose absence leaked every body.

## Response headers are readable — `headers.get()` is not hard-coded to null

A page reads a `fetch()`/XHR response's status and body, and it reads the response's **headers**:
`response.headers.get('content-type')` to branch on the payload shape, `Link` for pagination,
`X-RateLimit-Remaining` before the next call, `ETag` for a conditional re-fetch. For a long time the
JS `Response` was built as `{ …, headers: { get: () => null, has: () => false, forEach: () => {} } }`
and XHR's `getResponseHeader`/`getAllResponseHeaders` were `=> null` / `=> ""` — the server's headers
never reached the page. This is the read-side twin of the tick-148 gap where the page's *request*
headers were dropped: there the request reached the server stripped, here the response reached the
page stripped.

**The fix threads the real header list from the wire to the page.** `manuk_net::request` already
returns `resp.headers: Vec<(String, String)>`; both fetch pumps (the shell's `pump_fetches` and the
page's own `finish_loading`) now carry it into `Page::resolve_fetch(id, status, body, headers, …)` →
`manuk_js::resolve_fetch` → `event_loop::deliver`, which serializes the pairs as a JS array literal
and calls `__deliver(id, status, body, headers)`. `__makeResponse` builds a real `Headers` over that
list and XHR stores it as `_respHeaders`.

**The `Headers` semantics are the Fetch standard's, not a lookup table.** `get(name)` matches the
name **case-insensitively** (HTTP field names are case-insensitive — a page asking for
`'Content-Type'` finds a server's `content-type`) and comma-joins repeated fields; `has(name)` is the
same match; `getAllResponseHeaders()` emits one `name: value\r\n` line per field with the name
lower-cased. A header the server did not send is `null`, not `""` — the distinction a page's
`if (r.headers.get('x-foo'))` depends on. An **empty** header slice yields a `Headers` whose `get`
returns null, so the mock-fetcher event loop (which delivers with no headers) and every pre-existing
caller keep working unchanged — the plumbing is additive.

**Bound: cross-origin exposure is left to the CORS read barrier.** The barrier already blocks an
unreadable cross-origin *body* wholesale (settling `status 0`), so a page cannot read headers off a
response it cannot read at all. The finer per-header `Access-Control-Expose-Headers` safelist — which
in Chromium hides non-safelisted headers even on a readable cross-origin response — is a documented
follow-on; same-origin (the common case for header inspection) exposes the full list, which is
correct.

## `fetch(url, {signal})` honours AbortController — cancellation is not a no-op

`AbortController`/`AbortSignal` existed as globals (so `new AbortController()` didn't throw), but
`fetch` never read `opts.signal` — so `controller.abort()` did **nothing** to the request. This is the
single most common way modern code cancels a fetch: every React `useEffect(() => { const c = new
AbortController(); fetch(url, {signal: c.signal})…; return () => c.abort(); }, […])` depends on it, and
React 18 StrictMode's deliberate double-mount *relies* on the first request being cancelled — without
it the component sets state after unmount (the "can't update state on an unmounted component" race) and
StrictMode's effect-cleanup contract is silently broken.

**`fetch` now honours the signal, in three cases:**
- **Already aborted** when `fetch` is called → returns a **synchronously rejected** Promise and queues
  **no** network request at all (the host never sees it).
- **Aborted in flight** → the Promise rejects with the signal's `reason`, and the pending callback is
  **dropped from `__fetchCb`** so a later host delivery (`__deliverFetch(id, …)`) finds no callback and
  is a no-op — the response body can never resolve an already-cancelled fetch.
- **Never aborted** → unchanged; the request queues and resolves normally (an empty/absent signal adds
  nothing).

**The reject reason is a `DOMException` named `AbortError`,** per the Fetch/DOM standard — code
everywhere branches on `err.name === 'AbortError'` to tell a *cancel* apart from a real network
failure (a cancel is expected and swallowed; a failure is surfaced). The default abort reason in both
`AbortController.prototype.abort()` and the static `AbortSignal.abort()` was `new Error('AbortError')`
(whose `.name` is `'Error'`, not `'AbortError'`) — now a `DOMException(…, 'AbortError')`. Residue:
`XMLHttpRequest.prototype.abort()` is still a no-op (the XHR cancel path is a separate, rarer lever —
`fetch` is what the frameworks use), and `AbortSignal.timeout()` marks the signal aborted but does not
yet reject an in-flight fetch bound to it (needs the timer to route through the same drop path).

## `fetch(FormData)` sends multipart/form-data — a File is uploaded, not dropped

A `FormData` body has exactly one correct wire encoding: **`multipart/form-data`**. It is the only
encoding that can carry a file, which is the entire reason `FormData` exists. But `fetch(url, {body:
fd})` did `String(fd)`, and `FormData.toString()` is **urlencoded** — so a File part became
`String(file)` = `"[object File]"`. Every uploaded avatar, attachment, and document was **silently
dropped** and replaced with that placeholder; the server received a text field named after the input
whose value was the string `[object File]`, and the upload "succeeded" with no file. This is the
read/write mirror of the other silent-drop bugs (tick 148's dropped request headers, tick 167's
dropped multi-select values): the request reached the server *structurally intact but missing its
payload*.

**The fix encodes a FormData body as multipart, for both `fetch` and `XMLHttpRequest.send`.** A new
`FormData.prototype.__multipart(boundary)` walks the parts: a value that is a Blob/File (detected by
its `__blobText`) is emitted with `Content-Disposition: form-data; name="…"; filename="…"`, its own
`Content-Type`, and its content; a plain value is a simple text part. `fetch`/`send` generate the
boundary (`__multipartBoundary()` — `Math.random`-based, which is fine because a boundary only needs
to not occur in the body, not to be unguessable) and set `Content-Type: multipart/form-data;
boundary=…`, **replacing** any page-set Content-Type — because only the browser knows the boundary, a
page that set its own Content-Type for a FormData body would produce an unparseable request, so every
browser overrides it (`__withContentType` strips the old one first). `FormData.toString()` stays
urlencoded for the code that stringifies a FormData directly (`new URLSearchParams(fd)`); only the
request-body path changed.

**Bounds/residue.** A File's content is the Blob's `__blobText` (a JS string; the engine has no
byte-accurate binary body path yet — the same lossy-UTF-8 limitation the Blob layer already has), so a
text/JSON upload is exact and a true-binary upload rides the existing one-char-per-byte convention.
`FormData` detection is a duck-typed `__isFormData` flag rather than `instanceof FormData` (the
constructor can be shadowed). The native `<form enctype="multipart/form-data">` submission path (not
the JS `fetch`/XHR path) is a separate mechanism.

## `XMLHttpRequest.abort()` honours the cancellation — a late response no longer fires `onload`

`abort()` was `function() {}` — a literal no-op. So a cancelled XHR still fired `onload` with its full
response the moment the host delivered it: a search-as-you-type box that fires a request per keystroke
and `abort()`s the stale one would **apply the old response over the new** (the classic stale-result
race), and every request library's XHR cancel path did nothing. This is the XHR twin of the tick-172
`fetch`/AbortSignal gap.

**The fix drops the pending callback and fires the abort events.** `abort()` now `delete`s the request
from `__xhrObj`, so a later `__deliverXhr(id, …)` finds no object and is a **no-op** — the response
cannot resolve a cancelled request (the same drop-the-callback mechanism `fetch`'s abort uses). It
resets `status`/`responseText` to the cancelled state and fires `readystatechange` → `abort` →
`loadend` (the XHR standard's abort() event order), leaving `readyState` at `UNSENT`. Residue: an
`AbortSignal` passed to an XHR (rare) is still not wired; `abort()` on an already-delivered request is
a no-op (correct).

## `response.body` is a real `ReadableStream` — a streamed answer renders at all

The canonical streaming read on the modern web is three lines:

```js
const reader = (await fetch(url)).body.getReader();
for (;;) { const {done, value} = await reader.read(); if (done) break; render(decode(value)); }
```

Until tick 196 the first of those lines threw. `__makeResponse` hardcoded **`body: null`**, and
`ReadableStream` was one of the `__inertNames` stubs — a *named, empty constructor* with no
`getReader` on its prototype. So `res.body.getReader()` raised a `TypeError` **inside the response
handler**, which took the rest of the handler with it.

**The symptom is not "the answer streams in slowly" — it is that the answer never appears.** Every AI
chat (claude.ai, ChatGPT, Gemini), every cloud-console live-log tail and every inference token stream
ships this exact loop, so the entire class rendered blank. This was named the **#1 unlock** by the
Phase-0 edge audit, and it was one file.

**`typeof` would have lied.** `typeof ReadableStream === 'function'` was ALREADY true against the
inert stub, and `'body' in res` was already true against the `null`. The gate therefore asserts a
reader that actually **reads** — the `g_globals` lesson, restated: assert behaviour, never a name.

**What was built.** A real `ReadableStream` — a chunk queue plus a list of `read()` calls parked on an
empty queue, which is the entire mechanism; `enqueue`/`close`/`error` settle parked readers.
`getReader()` (locking, with `ReadableStreamDefaultReader`: `read`/`releaseLock`/`cancel`/`closed`),
`locked`, `cancel()`, `tee()` (an AI SDK forks the token stream — one branch to the UI, one to a log)
and `Symbol.asyncIterator`, so `for await (const chunk of res.body)` works as the short spelling.
`Response` gained a **lazy** `body` (constructing it eagerly would allocate a byte copy for every
response a page only calls `.json()` on), an honest accessor-backed `bodyUsed` that flips on *any*
consumption route, plus `arrayBuffer()`, `bytes()` and `blob()`.

Defining it here, ahead of the inert sweep that runs **last**, is what suppresses the stub — the same
ordering mechanism `AbortSignal` uses.

**The honest boundary, stated out loud.** The body reaches JS **fully buffered**: the host path
`manuk_net::request` → `NavEvent::PageFetch` → `event_loop::deliver` carries one `String` embedded as
a JS string literal, so this stream yields its chunks from memory, not off the wire. The *page's* code
path is entirely real — the pump loop, `done`, `TextDecoder` and SSE framing all execute exactly as
written, and the answer renders. But incremental wire-level delivery needs a per-chunk channel through
shell → page → js that does not exist below `manuk_net::fetch_streaming` (wired only to the document
loader), and that is a **subsystem, not a tick**. It is residue, and it is not claimed here. Until it
lands, a long answer appears in one go rather than token by token.

Residue: incremental wire-level chunking (above); `EventSource`/SSE remains an honest stub, so a page
using `new EventSource()` rather than fetch-with-streaming is still unserved; a double `text()` is
permissive rather than rejecting on `bodyUsed`; no BYOB readers, no backpressure (`desiredSize` is a
constant), no `WritableStream`/`TransformStream`/`pipeThrough`.

## Incremental delivery — the answer TYPES ITSELF OUT (`FetchStreamEvent`)

Tick 196 gave the page a real `response.body` to read from; it could still only be *fed* the whole
body at once, because `Page::resolve_fetch` settles a request with one complete `String`. So a
streamed answer appeared in a single lump the moment the server finished. This is the other half.

**`manuk_js::FetchStreamEvent`** is the shape the buffered path cannot express:

| step | meaning |
|---|---|
| `Head { status, headers }` | **where the page's `fetch()` promise resolves** — body still arriving |
| `Chunk(Vec<u8>)` | raw body bytes, as they come off the wire |
| `End` | the pump loop sees `{done: true}` |

One entry point per layer carries it: `Page::deliver_fetch_stream` → `manuk_js::deliver_fetch_stream`
→ `PageContext::deliver_fetch_stream` → `event_loop::{deliver_head, deliver_chunk, deliver_end}`.

**Resolving at the HEADERS is the load-bearing detail.** A real `fetch()` promise settles when the
response headers arrive, not when the body ends — that is precisely what lets a page take a reader
and pump while the rest is still in flight. Resolving at the end instead would make `response.body`
a stream that is always already complete, which is the buffered behaviour wearing a stream's costume.

**Each step runs the page's reactions before returning,** and `Page::deliver_fetch_stream`
re-cascades + re-lays-out afterwards, guarded on the dirty bit. That guard is what makes the answer
render *between* chunks rather than only at the end, at no cost for a chunk the page ignores.

**Bytes stay bytes across the Rust↔JS boundary.** `js_bytes_literal` emits one `\u00NN` escape per
byte and `__bytesFromLatin1` reads it back with `charCodeAt(i) & 0xff`. **Not**
`String::from_utf8_lossy`: a chunk boundary lands wherever the wire put it, which is routinely in the
middle of a multi-byte sequence, and lossy decoding would substitute U+FFFD and silently corrupt the
text.

**`TextDecoder` gained `{stream: true}`,** which the same fact makes mandatory. It now holds an
incomplete trailing sequence back (walk back over the `10xxxxxx` continuation bytes to the lead byte;
if the run is shorter than the length that lead byte announces, keep it) and prepends it to the next
call. Every streaming client on the web passes this flag; without it the whole `response.body` path
mangles any non-ASCII answer — "café" split after `0xC3` becomes a replacement character.

A streaming `Response` keeps a **buffered mirror** so `text()`/`json()` still work, but **drops it the
moment the page takes a reader** — otherwise an SSE stream that never ends would accumulate a copy of
every token forever. A page that streams does not also buffer. `clone()` on a still-streaming
response throws; `body.tee()` is the honest way to fork it.

Residue: the host side still calls the buffered `resolve_fetch` — `shell/src/gui.rs::pump_fetches`
uses `manuk_net::request`, and `manuk_net::fetch_streaming` is GET-only with no request headers, so
wiring the two (plus a `NavEvent` per step) is the next tick. Until then this path is exercised by
the engine and its gate, not by live navigation. `EventSource`/SSE and XHR `readyState 3` are still
stubs and should ride this same spine.

## The wire is connected — `request_streaming` + `PageFetchStream` (finish-line lever 1, done)

Tick 197 built the engine spine but the host still called the buffered `resolve_fetch`, so nothing
streamed during real navigation. This closes it.

**`manuk_net::request_streaming(method, url, headers, body, on_head, on_chunk)`** is to a page's
`fetch()` what `fetch_streaming` is to the document. It adds the three things the document-loader
version cannot do — an arbitrary **method**, request **headers** (an API call without its
`Authorization` is a 401) and a request **body** — and one it does not do: **`on_head` fires with the
response metadata before the body starts arriving.** Returning `ResponseMeta` at the end, as
`fetch_streaming` does, cannot express "headers now, body later", and handing the page its headers
only once the body finished would give it a stream that is already complete.

Redirects follow the browser rule: 301/302/303 rewrite to a bodiless `GET`, 307/308 replay method and
body as-is.

**`NavEvent::PageFetch` became `NavEvent::PageFetchStream { gen, id, event }`** — one event per step
instead of one per response. The worker sends `Head` the instant headers land, a `Chunk` per piece off
the socket, then `End`; the `gen` guard still drops a response for a page the user has navigated away
from.

**The CORS read barrier moved to the headers, and is strictly stronger there.** The buffered path
read the entire cross-origin body and *then* decided it was unreadable. Now a response the server did
not opt into sharing is refused before a single body byte is forwarded, with the chunk callback
dropping the rest on the floor. Still surfaced as Chromium does — `status 0`, rejecting the page's
promise with a `TypeError`.

**Failure has two shapes and they are not the same.** A failure *before* the headers must reject the
promise (`Head { status: 0 }`); one *after* them can only truncate the body, so it sends `End` — a
page whose reader never sees `done` spins forever waiting for an answer that is not coming.

**On the UI thread**, the follow-on work (re-pump the fetch queue, history ops, messages, persist
cookies/storage) runs **only on `End`** — doing it per chunk would re-drain the queue and re-save
cookies on every token — while `rerender()` runs on **every** step, which is the visible half of
streaming.

The gate is a timing claim, because that is the only kind buffering cannot fake: a raw-TCP server
sends the headers, half the body, then holds the rest back for 250ms. The first chunk must be
delivered at least 200ms before the last. Proven RED by making the implementation collect the body
and hand it over at the end — `chunks=1, first=last=253ms`, exactly the failure the claim names.

## WebSocket transport — borrowed, not hand-rolled (tick 200)

Phase-0 finish-line lever 3. The page-facing `WebSocket` constructor has existed for a while as an
**honest stub**: it constructs, then reports failure, so a live-news site's live-blog silently never
updated rather than throwing a ReferenceError. `manuk_net::websocket::WebSocketConn` is the transport
that makes it real.

**Borrowed from `tokio-tungstenite`.** RFC 6455 framing, client-side masking, the close handshake,
continuation frames and ping/pong are exactly the wheel that should not be reinvented — and getting
masking or the close handshake subtly wrong yields a connection that works against one server and
hangs against another.

**But the TLS is ours, and that is load-bearing.** `tokio-tungstenite`'s TLS features pull an
unpinned `tokio-rustls`, and cargo's feature **union** would re-enable the `aws-lc` backend across the
whole dependency graph — the exact failure already documented in `engine/net/Cargo.toml`, which once
broke the Windows build outright (`link.exe: exit code 1104`). So the crate is taken with
`default-features = false, features = ["handshake"]`, we connect the socket and run TLS with the
ring-pinned connector (`proxy::tls_connect`, now `pub(crate)` for exactly this), and hand tungstenite
a ready stream via `client_async`.

**Subprotocols are negotiated, not assumed.** The handshake request is built by hand so
`Sec-WebSocket-Protocol` can carry the page's offered list, and `protocol()` reports what the
**server** chose. A client that offered `["chat.v1", "chat.v0"]` and got `""` back must not then
speak chat.v1 at it.

Ping/pong are consumed rather than surfaced: they are keepalive, not page data, and the `WebSocket`
API does not expose them either.

**The close handshake is a real trap, and the gate caught it.** The first version of the gate's
*server* returned as soon as it read a `Close` frame. tungstenite replies to a close from inside
`next()`, so bailing out on the first Close drops the socket before the reply is flushed — and the
client correctly reported `Connection reset without closing handshake`. That is not a client bug: a
server that drops the socket is indistinguishable from a crashed one, and a browser is right to
surface it as an unclean close. The fix is to keep polling and let the loop end on its own.

Gated against a **real server** (tungstenite's accept side, not a mock of our own client): handshake
completes, subprotocol negotiated, text and binary round-trip intact, **the server pushes a message
the client never asked for** — the capability polling cannot express and the whole reason this
transport exists — and a clean close is observed as end-of-stream so a page's `onclose` fires instead
of hanging.

Residue: the page-facing JS `WebSocket` is **still the stub** — wiring this transport to it (plus the
shell's event pump, a per-connection id, `onopen`/`onmessage`/`onclose`/`onerror`, `bufferedAmount`
and `binaryType`) is the next tick, and finishes lever 3. No permessage-deflate (offered by many
servers, optional by spec), no auto-reconnect (correctly the page's job), no `Blob` binaryType yet.

## The page-facing `WebSocket` connects (tick 201) — lever 3's other half

Tick 200 built the transport; the JS `WebSocket` was still the honest stub. It now queues ops for the
host and receives events back — the same shape `fetch` uses, because the host owns the socket.

**Page → host: `WsOp`** — `Connect { url, protocols }`, `Send { data, binary }`, `Close { code,
reason }`, drained via `Page::take_ws_ops()`.
**Host → page: `WsEvent`** — `Open { protocol, extensions }`, `Message { data, binary }`,
`Sent { bytes }`, `Error`, `Close { code, reason, clean }`, delivered via
`Page::deliver_ws_event()`, which runs the page's handlers and re-renders if they changed the DOM.

**What the stub got wrong beyond not connecting.** It pre-filled `socket.protocol` with the client's
*first offered* subprotocol. `protocol` is what the **server** selects, and it is empty until it
does — pre-filling it tells the page a negotiation happened when none has.

**`send()` before OPEN still throws `InvalidStateError`.** That is the spec and clients are written
for it; what is new is that a socket can actually *be* open. `send()` after CLOSING drops the frame
rather than throwing, also per spec.

**`close()` moves to CLOSING(2), not straight to CLOSED(3)** — the closing handshake is not instant,
and a page that watches `readyState` sees the real intermediate state.

**Bytes stay bytes, again.** Frames cross as one char per byte, and the Rust side decodes with
`c as u32 & 0xff` rather than `as_bytes()` — the latter would UTF-8-encode 0x80..0xFF into two bytes
each and corrupt every binary frame. `binaryType` then decides the page-visible shape
(`arraybuffer` → `ArrayBuffer`, otherwise `Blob`); a client that set one and got the other breaks on
the first byte it reads.

**The `error` event carries no detail to the page,** deliberately — the spec withholds it because it
would be a cross-origin information leak. The message rides along for our logs only.

Gated by `g_websocket`: the connect op carries URL + offered protocols; `send()` before open throws
`InvalidStateError`; `onopen` reports the *server's* protocol and `readyState 1`; a frame sent from
`onopen` reaches the host queue; **an unprompted server push lands in `onmessage` and mutates the
DOM**, twice, appending; a binary frame preserves `0xFF`; `onclose` reports code, `wasClean` and
`readyState 3`. Proven RED by making `deliver_ws_event` not reach the page — `onopen` never fires and
the status stays at the pre-connect value.

Residue: **the shell is not wired yet** — nothing calls `take_ws_ops`/`deliver_ws_event` from
`gui.rs`, so this is engine-reachable but not yet live during browsing. That is the next tick and the
true end of lever 3; it needs a per-connection task holding the `WebSocketConn` plus an mpsc from the
UI thread for sends (bidirectional, unlike fetch). `bufferedAmount` decrements via `Sent` but nothing
emits it yet; no `Blob` binaryType read path; no permessage-deflate.

## WebSocket is LIVE in the browser (tick 202) — lever 3 complete

`gui.rs::pump_websockets` is the last piece: the page's ops now reach a real socket during ordinary
browsing.

**Why it is not shaped like `pump_fetches`.** A fetch is one request and one response, so its worker
can be a fire-and-forget task. **A socket stays open and is written to long after it was opened**, so
each connection gets a task that owns the `WebSocketConn` plus an `mpsc::UnboundedSender` the UI
thread queues frames onto (`App::ws_send`, keyed by socket id). The task `select!`s between "the page
wants to send" and "the server said something" — the only way to service both without one starving
the other, and the reason a polling loop would not do.

**Dropping the sender IS the close signal.** `WsOp::Close` removes the entry; the task's `rx.recv()`
returns `None`, completes the closing handshake and reports the real close back. So the page's
`onclose` reflects what actually happened rather than an optimistic local guess.

**Navigation closes every socket** — `ws_send.clear()` beside the `nav_gen` bump. A live-chat socket
must not keep streaming into a document the user has left, and the `gen` guard drops any frame
already in flight.

`WsEvent::Sent { bytes }` is emitted once a frame is actually on the wire, which is what makes
`bufferedAmount` fall — a client polling it to avoid flooding a slow socket now gets a real answer.
A failed connect sends `error` then `close(1006, wasClean: false)`, which is what a reconnect loop
backs off on.

**Gated by composition, because the shell itself cannot be.** `gui.rs` has no UI harness (the same
honest limitation as T6.1 agent-click and the tick-198 fetch wiring). But the *composition* is the
part that can silently disagree, so `g_websocket_live` does exactly what `pump_websockets` does, in
the same order, with a **real server** in the middle: drain the page's ops, connect a real
`WebSocketConn`, resolve the page's relative `'/live'` against the document URL, put the page's own
frame on the wire, pump the replies back, and assert the DOM reads
`offline[pong:ping][push](closed 1000)`. If the two halves disagreed about the op encoding, the
one-char-per-byte convention, the subprotocol or the close semantics, that gate fails where the unit
gates pass.

Residue: no `Blob` binaryType read path; no permessage-deflate; no auto-reconnect (correctly the
page's job); the close CODE from the server is not yet threaded through `WebSocketConn::recv` (a
clean close is reported as 1000 regardless of what the peer sent). Finish-line levers 4
(scroll-anchoring) and 5 (forced reflow) remain.

## `EventSource` (SSE) connects — built on our own fetch (tick 205)

The last piece of finish-line lever 1's stated scope ("ReadableStream + real SSE + readyState-3
XHR"). `EventSource` used to construct and then report that it could not connect — honest, and far
better than throwing, but it left every live-updates page dead: score tickers, CI/deploy log tails,
notification streams, dashboard metrics, and the many AI chats that use SSE rather than
fetch-streaming.

**It is implemented on top of our own `fetch`, and that is why it is small.** Ticks 196-198 made
`response.body` a real `ReadableStream` fed incrementally off the wire, and SSE is precisely *a text
stream cut into frames on blank lines*. So this needed **no new Rust plumbing at all** — the same
route a polyfill takes, except our fetch is real. It is also the first proof that the streaming spine
carries a second consumer.

The frame parser is where the correctness lives:

- **A frame ends at a blank line, not at a chunk boundary.** The trailing partial frame stays
  buffered. Dispatching per chunk delivers half a message — the gate falsifies to exactly that
  (`[first\npar/1]`).
- **CRLF/CR are normalised first.** A server sending `\r\n` would otherwise never appear to
  terminate a frame at all.
- **Multiple `data:` lines join with `\n` as ONE message**, not several.
- **One leading space after the colon is stripped** (exactly one, per spec).
- **A comment line (`: keepalive`) dispatches nothing** — it is the standard idle heartbeat.
- **A named `event:` goes to its own listener and NOT to `onmessage`.**
- `id:` persists as `lastEventId` across subsequent frames.
- `{stream: true}` decoding, because a chunk boundary can split a multi-byte character.

Residue: **no automatic reconnection.** A real `EventSource` reconnects when the stream ends, honouring
the server's `retry:` interval and resending `Last-Event-ID`; we parse `retry:` but ignore it, and a
finished stream fires `error` and stays closed. That is the one substantial gap and it is what makes
SSE resilient in practice, so it is worth closing. No `withCredentials` enforcement beyond recording
the flag.

## XHR `readyState 3` — progress instead of nothing-then-done (tick 206)

The last item in finish-line lever 1's stated scope. The streaming delivery path from ticks 197-198
only knew about `fetch`: `__deliverHead` bailed out on an XHR id, a documented residue. So an XHR
still received its whole body in one delivery — `readyState` went 1 → 4, `onprogress` never fired,
and `responseText` was empty right up until it was complete. **A download progress bar showed nothing
and then 100%**: the transfer appeared to take zero time.

The three delivery entry points now branch on which kind of request the id belongs to:

- `__deliverHead` → `readyState 2` (HEADERS_RECEIVED), status and headers readable, body still empty.
- `__deliverChunk` → append to `responseText`, `readyState 3` (LOADING), fire `readystatechange` and
  `onprogress` with `loaded`.
- `__deliverEnd` → `readyState 4` (DONE), parse `responseType: "json"` at this point (not before —
  partial JSON does not parse), then `onload`/`onerror` and `onloadend`.

Decoding uses `{stream: true}` for the same reason everything else on this path does: a chunk
boundary can split a multi-byte character.

The buffered `__deliverXhr` remains for the non-streaming path (the headless loader and the
mock-fetcher event loop), so `readyState` still goes straight to DONE there — correct, because that
path genuinely has the whole body at once.

Gated by `g_xhr_progress`: the lifecycle is `2 → 3 → 3 → 4` rather than `1 → 4`; at `readyState 3` the
page reads a **partial** `responseText` and `onprogress` reports `loaded`; the body **grows** across
chunks; `onload` has not fired while the body is unfinished, and fires once with the complete body at
the end. Proven RED by never reporting LOADING — the state string collapses from `23` to `22`.

**Finish-line lever 1 is now complete in full**: ReadableStream + `response.body` (196), the
incremental spine (197), the wire (198), real SSE (205), and XHR `readyState 3` (206).
