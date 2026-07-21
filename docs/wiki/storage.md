# STORAGE — the persistence layers a page can reach, and what each one really guarantees

Cookies and their SameSite/prefix enforcement live in `networking.md`; this file covers the
**page-visible storage APIs**: Web Storage and IndexedDB.

## The absence of a storage API is a GRADING signal, not a reported failure (ticks ~60, 278)

This has now cost real time twice, in the same shape, four hundred ticks apart.

`localStorage` was missing, and MediaWiki's startup script runs
`isCompatible() { return !!('querySelector' in document && 'localStorage' in window && …) }`. Failing
it, MediaWiki reverted **every Wikipedia page in the world** to its no-script fallback: the table of
contents never collapsed and everything below it landed thousands of pixels out of place. It looked
like a layout bug for an hour. It was a missing BOM object.

`indexedDB` was missing in exactly the same way, and the shape of the damage is identical: apps write
`if (!window.indexedDB) { /* degraded path */ }`, take the lesser branch, **throw nothing**, and look
like some other bug for as long as you are willing to keep looking. Offline caches, draft documents,
the session layer of the AWS and GCP consoles, and every PWA that claims to work on a plane are all
behind it.

**The general rule:** a feature the web *feature-detects* is not scored by whether pages crash without
it. It is scored by whether pages **silently grade you down** — and that failure emits no signal at
all. Those APIs must be gated on observable behaviour, never on the symbol existing, which is why
`G_INDEXEDDB` asserts key order and rollback rather than `typeof indexedDB`.

## The host-native + prelude-shim pattern, and where the line falls (tick 278)

Both storage APIs use the same two-layer split, and the split is deliberate:

| Layer | Owns | Why here |
|---|---|---|
| **Rust** (`manuk_net::webstorage`, `manuk_net::idb`) | the origin partition, the bytes, the quota, the on-disk envelope | one native entry point per API — every extra one is another place a `*mut JSObject` can outlive a GC |
| **JS boot shim** (`dom_bindings.rs`) | the *interface*: requests, transactions, upgrades, cursors, key/value encoding | this is where the specification's difficulty actually is |

The seam is one string-in/string-out function (`__storage`, `__idb`). **The store is not where
IndexedDB is hard.** Getting a `BTreeMap` to hold bytes is nothing; getting the request/transaction
lifetime right is the entire job.

### Why `idb` is a serde envelope and not redb

The board's standing advice is *borrow redb/heed*, and for a large durable KV that is correct. It was
declined here on scope, not on principle: the JSON envelope is what Web Storage already uses, costs
zero build time and zero new dependency surface, and kept the tick atomic. The `idb` API is written
so the backing map is **not** part of its contract — `get`/`put`/`records` take an origin and return
owned values, so swapping in redb is contained behind those functions. **Do that when a real workload
puts megabytes through it, not before.** Recorded here so it is an upgrade path rather than a
rediscovery.

## Key encoding carries the spec's TYPE ORDER, or `getAll` lies (tick 278)

The store sorts by an opaque key string and never interprets it. That means IndexedDB's key ordering
— **number < date < string < array** — has to be built into the encoding's *prefix*, and numbers must
be offset and zero-padded so lexicographic comparison agrees with numeric comparison.

Skip the padding and key `10` sorts before key `9`. Nothing throws. `getAll()`, `getAllKeys()` and
every cursor walk simply return rows in an order the page never asked for, which surfaces later as a
list rendered wrong or a "latest record" that is not the latest. The gate pins this with
`order:2,9,10`, and the unpadded RED probe produces exactly `order:10,2,9`.

## Indexes must PERSIST across a reopen, so their metadata cannot live in the shim (tick 329)

`store.index('by_email').get(addr)` — look a record up by a value property, not its primary key — is
the query the Firebase/Cognito/Dexie/`idb` auth layers are built on. `createIndex`, `store.index()`,
`IDBKeyRange` and `multiEntry`/`unique` landed at tick 329 (`G_INDEXEDDB_INDEX`).

The decision that matters is **where the index metadata lives.** An index is declared once, in
`onupgradeneeded`, and on a returning visit the page opens the database at the *same* version — so no
`versionchange` fires and `createIndex` never runs again. Yet `store.index('by_email')` must still
resolve. Therefore the index set is persisted **with the store** in `manuk_net::idb`
(`ObjectStore.indexes`), serialized out on `open` and re-applied (add *and* remove) on every
`upgrade`. A shim that kept indexes in a JS map would pass a single-session gate and silently break
every second visit — so the gate opens, indexes, closes the connection, reopens with **no upgrade**,
and requires the index to still resolve records.

Everything else follows from the store's existing encoded-key order: an index builds its ordered view
by drawing `pathGet(value, keyPath)` from each record (an array key with `multiEntry` expands to one
entry per element), sorts by *encoded index key then primary key*, and `IDBKeyRange` compares in that
same encoded space — so an index's "between" and the store's "in order" can never disagree. A `unique`
index enforces on `put` by scanning for another record with the same index key before the write lands,
so a violation leaves nothing behind. **Honest limit:** a compound (array) keyPath round-trips as its
JSON text through the store's single `key_path` string; there is no locale collation.

## IndexedDB stores STRUCTURED CLONES — `JSON.stringify` is a silent type change

`JSON.stringify` turns a `Date` into a string and a `Uint8Array` into an object with numeric keys. The
page writes one type and reads back another, nothing throws, and every later comparison is quietly
wrong. `Date`, `ArrayBuffer` and typed-array views are therefore **tagged** in the encoding, and a
plain object that itself carries the tag key is wrapped — otherwise decoding would mistake the page's
own data for a tag.

**Honest limit:** `Map`, `Set`, `RegExp`, `Blob` and `File` are not yet tagged and degrade to plain
objects. That is a known gap, written down rather than discovered.

## Async shape is a CORRECTNESS property, not politeness (tick 278)

Every IDB callback is delivered on a microtask, never inline — even though the store underneath is
synchronous. This is not decoration:

```js
var req = indexedDB.open('db', 1);
req.onsuccess = function () { req.result … };   // `req` is still undefined if onsuccess fired inline
```

A shim that settles synchronously fires `onsuccess` **before `open()` has returned**, so the page's
own `req` variable is `undefined` inside its handler. Replacing `micro()` with a direct call does not
merely fail a claim in the gate — it takes the whole script down, which is precisely what it does to
real page code.

The same reasoning governs transaction completion: a request settles on one microtask and the
completion check runs on **another**. The dominant real-world pattern is issuing the next request from
inside `onsuccess`, and a transaction that completed the instant its pending count hit zero would
finish before that follow-up was ever queued.

## `abort()` must roll back, and a vacuous rollback test will tell you it does (tick 278)

Writes are applied eagerly under the seam, so each one pushes an **undo closure** onto the
transaction. `abort()` replays them in reverse. Firing `onabort` while the data stays changed is worse
than having no transactions at all, because the page believes it undid something.

**This is where tick 278 caught itself.** The first rollback claim read a record after a *failed*
`add()` and asserted the old value survived — and it **passed against a build whose `abort()` rolled
back nothing**, because a rejected `add()` never wrote anything. There was nothing to undo, so the
claim measured nothing. It is now written the hard way: a `put` that **succeeds**, `abort()` called
from inside its own success handler, and the old value required back. The disabled-undo probe then
produces exactly `rollback:OVERWRITTEN`.

This is the third recorded instance of the class (see `conformance-and-oracles.md`): **a green claim
is worthless until a probe has made that specific claim go red.** Not the gate — *the claim*. A gate
with twelve assertions where eleven are load-bearing and one is vacuous reports green either way.

## A quota that is never enforced is not a quota

Both stores enforce a real per-origin byte limit (Web Storage 5 MiB, IndexedDB 64 MiB — larger
because that 5 MiB ceiling is *why* pages reach for IDB). The IDB check runs **after** the insert and
rolls it back on failure, so the write that crosses the line is the one refused and the store never
keeps a record it reported as rejected. Reporting a rejection and keeping the data is worse than
either outcome on its own.

---

## The Cache API — `caches` (tick 279)

The third storage API, and the only one whose unit is a **request/response pair**. `localStorage`
holds strings, IndexedDB holds structured values, and neither can hold a *response* — which is why
the Cache API, not IndexedDB, is what a PWA's install step fills and what a Service Worker's `fetch`
handler reads on every navigation afterwards.

Same architecture as IndexedDB, deliberately: a store in `manuk_net::cachestorage` behind **one**
native seam (`__caches(opJson)`), with the promise plumbing and the matching rules in the boot shim.
Every extra native entry point is another place a `*mut JSObject` can outlive a GC.

### Bodies are stored as bytes, not as text

This is the whole care in the implementation. A cache holds fonts, images and wasm as readily as it
holds HTML. Round-tripping those through a UTF-8 `text()` **inflates every byte above `0x7F` into
two** — the same defect that once made a 260-byte media segment arrive as 407 bytes and defeated
every demuxer downstream. Bodies therefore travel and persist as a **latin-1 byte string** (one char
per byte, lossless in both directions), which is the same `raw` channel `__makeResponse` already
takes for exactly this reason. Text is recovered with `TextDecoder` on read.

The gate proves this rather than asserting it: a 6-byte body containing `0x80`, `0xC3` and `0xFF`
must come back as 6 identical bytes. With bodies stored as text the claim reads `bytes:false/9`.

### Entries are a list, not a map

`cache.keys()` is specified to return requests **in insertion order**, and responses that differ by
`Vary` must coexist for one URL. A `BTreeMap` keyed by URL makes both impossible and does so
silently — the second `put` would overwrite a response the spec says to keep. So `put` replaces on
the triple **(url, method, vary)** and appends otherwise.

Replacement matters for a reason that is easy to miss: a PWA re-runs its install step on every
version. With append semantics the cache grows without bound *and the stale first response is the
one served forever after* — the gate shows exactly that as `replaced:CODE` instead of `CODE2`.

### `Response` and `Request` became constructible

Both were on the inert interface-surface list, so `typeof Response === 'function'` was true while
`new Response('x')` produced an object with no `status`, no `headers` and no `clone()`. That is the
worst shape a failure takes — the feature detection passes and the first real use fails somewhere
else entirely. Nothing can be put *into* a cache without a constructed response, so this had to
become real; they are now built on `__makeResponse`, which means a constructed response and a
fetched one are the same shape and nothing downstream cares which it got.

### A miss resolves `undefined` — it does not reject

Every cache-first handler on the web is `caches.match(e.request).then(r => r || fetch(e.request))`.
A shim that rejects on a miss turns the entire offline path into an unhandled rejection, and the
gate demonstrates it: with `match` rejecting, the probe output stops dead at `THREW:NotFoundError`
and every later claim disappears with it.

### Honest limits

`add()`/`addAll()` fetch from the network and are **not** gated — a gate that needs a live server
false-REDs on a quiet box. Their one load-bearing rule (refuse a non-`ok` response, or a PWA ships
an install that "succeeded" and serves a 404 page forever) is implemented and read by inspection.
There is no `Vary: *` handling beyond declining to match, and `ignoreVary` is not implemented.

**The Service Worker is still absent.** What tick 279 built is its *store*; what remains is
registration, lifecycle and `fetch` interception. The constellation row was split rather than
flipped, because one row cannot honestly say "half of this works".
