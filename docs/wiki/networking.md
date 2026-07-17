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
