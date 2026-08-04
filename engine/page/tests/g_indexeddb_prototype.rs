//! **G_INDEXEDDB_PROTOTYPE — an IndexedDB object's methods live on its INTERFACE PROTOTYPE.**
//!
//! Every IndexedDB object in this engine was a plain literal carrying its methods as OWN properties.
//! Calls worked. `instanceof` worked. `typeof store.getAll === 'function'` was true the whole time —
//! and **nobody asks that**.
//!
//! `idb`, the ecosystem's dominant IndexedDB wrapper (Firebase, Workbox, and a large share of every
//! PWA depend on it), builds its entire convenience API — `db.get(store, key)`, `db.getAll(store)`,
//! `db.put(store, value)` — behind one test:
//!
//! ```js
//!   if (!(targetFuncName in (useIndex ? IDBIndex : IDBObjectStore).prototype)) return;
//! ```
//!
//! Our prototypes were **empty**, so the test failed, no convenience method was ever created, and
//! the page died on `this.idb.getAll is not a function`. Harvested from the live corpus on
//! `coinmarketcap.com`, which logged `getFromLocalDB TypeError: t.get is not a function` **four
//! times in one load** alongside it. Chrome answers `true` to all ten of the `in` tests below; we
//! answered `false` to all ten.
//!
//! **This is t862's law about a different library — *ask what a library BELIEVES, not what it can
//! detect.*** The capability was complete; the shape it was published in was not one the ecosystem
//! reads.
//!
//! ## What the fix has to buy beyond the feature test, or it is theatre
//!
//!   * **The name is on the prototype BEFORE a database is opened.** The first draft populated the
//!     prototype as a side effect of constructing a store, which passes a probe that opens a
//!     database first and fails the case that matters: on a RETURN visit the schema already exists,
//!     no `upgradeneeded` fires, no store is ever constructed — and `db.get(...)`, the call that
//!     needs the prototype, is the first thing `idb` reaches for. Asserted at script top level.
//!   * **Patching the prototype takes effect.** With the methods as own properties it was a silent
//!     no-op — the exact defect `G_PROTOTYPE` exists for, and how every error tracker, ad-blocker and
//!     polyfill hooks a platform object.
//!   * **A call on a foreign receiver throws**, as Chrome's "Illegal invocation" does, instead of
//!     doing something.
//!   * **NO PUBLISHED NAME IS A STUB.** A name on a prototype with nothing behind it is t772-775's
//!     *half-presence routes into a wall*, which is strictly worse than the absence it replaces — so
//!     the last claim walks every published name and requires a real implementation behind it.
//!
//! Ground truth: `chromium --dump-dom` over `http://127.0.0.1` (a `file://` origin has no IndexedDB
//! at all, which is its own trap — the first run of this fixture returned an empty `#out` from
//! Chrome and said nothing about why).
//!
//! ⚠ **AND ONLY THE FIRST NINE CLAIMS ARE CHROME-VERIFIED, WHICH IS SAID HERE RATHER THAN IMPLIED.**
//! Headless Chrome under `--virtual-time-budget` never settles the `open` request, so its `#out`
//! carries the synchronous block and nothing after it. Those nine — including `idbGate`, the
//! library's own condition — came back `true` from Chrome, byte for byte. The rest (`own=false`,
//! `patch=1`, `foreign=TypeError`, `stubs=none`) are properties Chrome satisfies *by construction*,
//! because in Chrome the prototype method IS the implementation; they are asserted from the spec, not
//! from a reading, and this note exists so a later audit does not mistake them for measured.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
  var r = [];
  function has(o, n) { try { return n in o; } catch (e) { return 'THREW'; } }
  // ── BEFORE a database exists. This is the ordering `idb` actually hits on a return visit.
  r.push('store.get=' + has(IDBObjectStore.prototype, 'get'));
  r.push('store.getAll=' + has(IDBObjectStore.prototype, 'getAll'));
  r.push('store.put=' + has(IDBObjectStore.prototype, 'put'));
  r.push('store.count=' + has(IDBObjectStore.prototype, 'count'));
  r.push('index.getAll=' + has(IDBIndex.prototype, 'getAll'));
  r.push('index.getKey=' + has(IDBIndex.prototype, 'getKey'));
  r.push('db.transaction=' + has(IDBDatabase.prototype, 'transaction'));
  r.push('tx.objectStore=' + has(IDBTransaction.prototype, 'objectStore'));
  // ── `idb`'s own gate, reproduced verbatim: the wrapper creates `db.getAll` only if BOTH hold.
  r.push('idbGate=' + (('getAll' in IDBObjectStore.prototype) && !('getAll' in IDBDatabase.prototype)));
  document.getElementById('out').textContent = r.join(' ');
  var open = indexedDB.open('g_idb_proto', 1);
  open.onupgradeneeded = function (e) {
    var d = e.target.result;
    var s = d.createObjectStore('s', { keyPath: 'id' });
    s.createIndex('byv', 'v');
  };
  open.onsuccess = function (e) {
    var db = e.target.result, r2 = [];
    r2.push('dbIs=' + (db instanceof IDBDatabase));
    var tx = db.transaction('s', 'readwrite'), st = tx.objectStore('s');
    r2.push('stIs=' + (st instanceof IDBObjectStore));
    // The method is INHERITED, not own — which is what makes a prototype patch visible.
    r2.push('own=' + Object.prototype.hasOwnProperty.call(st, 'put'));
    st.put({ id: 1, v: 'a' });
    var g = st.get(1);
    g.onsuccess = function () {
      r2.push('roundtrip=' + (g.result && g.result.v));
      // A prototype patch must be OBSERVED by a real call.
      var orig = IDBObjectStore.prototype.count, hit = 0;
      IDBObjectStore.prototype.count = function () { hit++; return orig.apply(this, arguments); };
      db.transaction('s', 'readonly').objectStore('s').count();
      IDBObjectStore.prototype.count = orig;
      r2.push('patch=' + hit);
      // A foreign receiver throws, as Chrome's "Illegal invocation" does.
      try { IDBObjectStore.prototype.getAll.call({}); r2.push('foreign=none'); }
      catch (err) { r2.push('foreign=' + err.name); }
      // ── NO PUBLISHED NAME IS A STUB: every inherited method must resolve to a real
      // implementation on a live instance. This is the claim that keeps the fix from degrading
      // into a list of names.
      var tx3 = db.transaction('s', 'readonly'), s3 = tx3.objectStore('s'), i3 = s3.index('byv');
      var missing = [];
      [[IDBObjectStore, s3, 'store'], [IDBIndex, i3, 'index'],
       [IDBDatabase, db, 'db'], [IDBTransaction, tx3, 'tx']].forEach(function (t) {
        Object.getOwnPropertyNames(t[0].prototype).forEach(function (k) {
          if (k === 'constructor' || typeof t[0].prototype[k] !== 'function') { return; }
          if (typeof t[1][k] !== 'function' || !t[1].__idbImpl || typeof t[1].__idbImpl[k] !== 'function') {
            missing.push(t[2] + '.' + k);
          }
        });
      });
      r2.push('stubs=' + (missing.length ? missing.join(',') : 'none'));
      document.getElementById('out').textContent = r.join(' ') + ' || ' + r2.join(' ');
    };
  };
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn an_indexeddb_objects_methods_are_on_its_interface_prototype() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://idbproto.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("IDB PROTOTYPE: {got}");

    for (claim, why) in [
        (
            "store.getAll=true",
            "**THE DEFECT**, and the one the corpus named: `'getAll' in IDBObjectStore.prototype` \
             was FALSE, so `idb` never created `db.getAll` and coinmarketcap died on \
             `this.idb.getAll is not a function`. Chrome says true",
        ),
        ("store.get=true", "…and `get` — `t.get is not a function`, four times in one load"),
        ("store.put=true", "…and the write half"),
        ("store.count=true", "…and `count`"),
        ("index.getAll=true", "`IDBIndex.prototype` is the other half of `idb`'s gate"),
        ("index.getKey=true", "…including the key-only read"),
        ("db.transaction=true", "the database's own methods were own properties too"),
        ("tx.objectStore=true", "and the transaction's"),
        (
            "idbGate=true",
            "**`idb`'s OWN CONDITION, reproduced verbatim**: it creates `db.getAll` only when the \
             name IS on `IDBObjectStore.prototype` and is NOT already on the database. Asserting the \
             library's actual test, rather than our model of it, is the difference between fixing \
             this and fixing something adjacent to it",
        ),
        (
            "dbIs=true",
            "**THE GUARD**: moving methods to prototypes must not break `instanceof`, which the \
             `iface()` `Symbol.hasInstance` provides and a naive `setPrototypeOf` could have lost",
        ),
        ("stIs=true", "…for the store as well"),
        (
            "own=false",
            "**THE PROPERTY THAT MAKES THE PATCH WORK**: the method is INHERITED. An own method \
             shadows the prototype, so the feature test would pass while every prototype patch \
             stayed a silent no-op — a fix that satisfies the library and nothing else",
        ),
        (
            "roundtrip=a",
            "…and the call still reaches the real implementation through the dispatcher. A \
             prototype full of names that no longer store anything is the worse bug",
        ),
        (
            "patch=1",
            "**`G_PROTOTYPE`'s property, on IndexedDB**: patching `IDBObjectStore.prototype.count` \
             is OBSERVED by a real call. This is how every error tracker, ad-blocker and polyfill \
             hooks a platform object",
        ),
        (
            "foreign=TypeError",
            "a call on a foreign receiver throws, as Chrome's \"Illegal invocation\" does, rather \
             than silently doing something to an object that is not a store",
        ),
        (
            "stubs=none",
            "**NO PUBLISHED NAME IS A STUB.** Every function on the four prototypes must resolve to \
             a real implementation on a live instance. A name with nothing behind it is \
             half-presence, which routes callers into a wall instead of into their fallback — \
             strictly worse than the absence it replaced (t772-775)",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_INDEXEDDB_PROTOTYPE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
