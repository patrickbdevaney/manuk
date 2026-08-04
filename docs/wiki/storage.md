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

## `getAllRecords(options)` returns full records in one call, on the store AND an index (tick 420)

The Interop-2026 addition (`G_INDEXEDDB_GETALLRECORDS`). The old idiom for "read a keyed page WITH its
keys" was two requests — `getAll()` for values, `getAllKeys()` for keys — zipped client-side.
`getAllRecords(options)` returns them already zipped: an array of `{ key, primaryKey, value }` records,
honoring `{ query, count, direction }`. On an **object store** the index key *is* the primary key, so
`key === primaryKey`. On an **index** they differ — `key` is the index key (the value property),
`primaryKey` is the store key — which is exactly the split a cursor exposes, now materialized in one
array. That `key !== primaryKey` difference is the gate's RED-prover: a `getAll` stand-in wearing the
name would report `key === primaryKey` on the index and the gate goes red.

It reuses the existing ordered views rather than adding machinery: the store reads `readAll()` (already
store-key ordered), an index reads `view()`/`matching()` (ascending by *index key then primary key*).
`direction: 'prev'`/`'prevunique'` reverse; `'nextunique'`/`'prevunique'` first keep one record per
distinct index key — the smallest primary key, which is the first occurrence in the ascending order —
so unique-direction dedup and cursor order can never disagree.

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

## A Blob holds BYTES, not String(part) — binary parts and readAsArrayBuffer (tick 422)

The `Blob`/`File` shim (event_loop.rs) stores a blob's contents in `__blobText`. The name is historical:
it is a **binary string** (one character per byte, 0-255), which is why `arrayBuffer()`, `size`,
`slice()`, `stream()` and `FileReader` all read it via `charCodeAt(i) & 0xff`. The constructor bug fixed
at tick 422: a part that was an `ArrayBuffer`, a typed-array view, or a `DataView` went through
`String(p)` — so `new Blob([new Uint8Array([1,2,3])])` stored the text `"1,2,3"` (size 5, wrong bytes).
`new Blob([bytes], {type})` is the most common way binary data enters the platform (decoded media, file
uploads, `canvas.toBlob`, object URLs), so this was silent corruption at a high-traffic seam.

The fix converts binary parts to their raw bytes (a typed-array view uses `byteOffset`/`byteLength`, so a
`subarray` contributes only its window), and leaves STRING parts exactly as before — the several
consumers that read `__blobText` as text (fetch request body, FormData multipart, XHR `send`, clipboard,
`text()`) are unchanged, which the regression suite confirms. It also un-stubbed
`FileReader.readAsArrayBuffer`, previously a `new ArrayBuffer(0)` that dropped every byte.
**Honest limit:** `__blobText` is a UTF-16 JS string used as a byte container, so a code unit above 0xFF
would still truncate under `& 0xff`; string parts are not UTF-8 re-encoded (so `size` of a multibyte
TEXT part is its char count, the pre-existing behaviour, not its UTF-8 byte length).

## A `localStorage` method assignment was ACCEPTED AND DISCARDED (tick 587)

`localStorage` is a `Proxy` over the `__storage` native seam, which gives it the real interface —
indexed access, `length`, enumeration, `delete`. Its `set` trap read:

```js
set: function (t, p, v) {
  if (typeof p === 'string' && !hasOwnProperty.call(t, p)) { __storage('set', area, p, v); }
  return true;                    // ← a METHOD NAME falls through here and is dropped
}
```

So `localStorage.foo = 'bar'` correctly stored an item, and **`localStorage.setItem = fn` was accepted
and thrown away** — the bare `return true` told the assignment it had succeeded. In a browser the methods
live on `Storage.prototype`, so assigning one creates an **own property that shadows it**, and subsequent
calls run the replacement.

### Why it is a capability, not a conformance detail

**Patching storage is one of the commonest things a real page does**, and every one of these installed
silently and then never ran:

- **private-mode / quota fallbacks** — wrap `setItem`, catch `QuotaExceededError`, fall back to an
  in-memory shim. Safari's private mode made this idiom universal.
- **SSR / hydration guards** — replace storage with a no-op so shared code does not touch a missing API
  during server-side or pre-hydration render.
- **session and analytics libraries** — wrap `setItem` to mirror writes, namespace keys, or expire them.
- **a page's own test bundle** — every `spyOn(localStorage, 'setItem')`.

The failure shape is the worst available: **no error, no warning, and the original behaviour continues**,
so the page looks fine until the case the wrapper existed for actually arrives.

### The fix, and the guard that constrains it

A method-name assignment writes to the **target** (shadowing); anything else is a storage write.
`deleteProperty` restores from a `pristine` copy captured before the Proxy is built — the target *is* the
object being shadowed, so there is nothing to restore from otherwise, and comparing against it would
compare a value with itself.

`G_STORAGE_PATCHABLE` RED-proves **both directions**, and the second is the one that matters: make *every*
assignment shadow and `plain:true` fails, because `localStorage.plainkey = 'v'` must still write to
storage. The obvious fix is over-broad, and the gate says so.

> **How it was found: by an instrument that needed the capability.** Tick 586's certificate probe tried to
> wrap storage to record touches and recorded nothing. `wrapStuck=false` here while `indexedDB.open`,
> `fetch` and `IntersectionObserver` all wrapped fine — **two host objects in one engine disagreeing about
> the same idiom**. A tool built to measure the browser found a bug in it, which is the argument for
> building the instrument out of the same primitives the web uses.

## `typeof store.getAll === 'function'` was true, and nobody asks that

Every IndexedDB object in this engine was a plain literal carrying its methods as **own** properties.
Calls worked, `instanceof` worked, and the obvious feature test passed. `idb` — the ecosystem's
dominant IndexedDB wrapper, which Firebase, Workbox and a large share of every PWA depend on — builds
its whole convenience API (`db.get(store, key)`, `db.getAll(store)`, `db.put(...)`) behind a different
question:

```js
  if (!(targetFuncName in (useIndex ? IDBIndex : IDBObjectStore).prototype)) return;
```

Our prototypes were empty, so no convenience method was ever created and the page died on
`this.idb.getAll is not a function`. Harvested from the live corpus on `coinmarketcap.com`, which also
logged `getFromLocalDB TypeError: t.get is not a function` four times in one load. Chrome answers
`true` to all ten `in` tests; we answered `false` to all ten.

**The shape: ask what a library BELIEVES, not what it can detect.** Same law as t862's
`{}.toString.call(div)` returning `[object Object]`, aimed at a different property.

### What the fix must buy beyond the feature test

Moving the closures into a private slot and putting a **dispatcher** on the prototype buys three
things, and a fix that only satisfied the `in` test would buy one:

* the name is on the prototype;
* **patching the prototype takes effect** — with an own method shadowing it, every ad-blocker, error
  tracker and polyfill hook was a silent no-op (`G_PROTOTYPE`'s property, on IndexedDB);
* a foreign receiver throws `TypeError`, as Chrome's *"Illegal invocation"* does.

### Two traps, both hit

**A lazy prototype passes the probe and fails the page.** Populating the prototype as a side effect of
constructing a store works on a probe that opens a database first, and is useless on a return visit:
the schema exists, no `upgradeneeded` fires, no store is constructed, and `db.get(...)` — the call
that needs the prototype — is the first thing `idb` reaches for. Install eagerly.

**`iface()` runs after this prelude**, so `globalThis.IDBObjectStore` is `undefined` when the eager
block runs, and a version that skipped on that skipped all four interfaces in silence. Create the
constructor when one is absent; `iface()` adopts a `globalThis[name]` that is already a function and
only attaches `Symbol.hasInstance`.

### The stub check caught its own author

`stubs=none` walks every published name and requires a real implementation behind it on a live
instance. First run: six failures — `IDBObjectStore.getKey` / `openKeyCursor` (genuinely absent),
`update` (a CURSOR method a regex swept into the store's list), and
`IDBDatabase.createObjectStore` / `deleteObjectStore` (only ever on the versionchange database). A
name on a prototype with nothing behind it is half-presence, which routes callers into a wall instead
of into their fallback — strictly worse than the absence it replaced.

### What it bought, stated honestly

`coinmarketcap.com`: SHAPE **26.2% → 26.3%** on the same 2046 elements — **no movement**. The throws
are gone and the page now fails one layer deeper, at `NotFoundError: no object store named
local-key-val`, because the wrapper finally works. This failure was on a cached-data path, not the
first-paint path. *"The instrument cannot price this"* is the honest report, not *"this bought
nothing"*.
