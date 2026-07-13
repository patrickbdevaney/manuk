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
