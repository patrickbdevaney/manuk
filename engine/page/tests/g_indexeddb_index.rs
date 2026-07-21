//! **G_INDEXEDDB_INDEX — `store.index()` / `createIndex` / `IDBKeyRange` must be a real secondary
//! key, not an absent method.**
//!
//! The base `indexedDB` store (G_INDEXEDDB) keys records by their primary key. Everything that reads
//! records by a *value property* — `store.index('by_email').get(addr)` — goes through an INDEX, and
//! the SDKs that define the app web build on exactly that: the Firebase and Cognito auth layers,
//! Dexie, `idb`, and every "query by a field that isn't the id" a page ever writes. Before this gate
//! `store.index` was `undefined` and `store.indexNames` was permanently empty.
//!
//! An absent index is the silent-degrade shape this project keeps naming: the SDK calls
//! `store.index(...)`, `undefined.get` throws *inside the SDK's own promise*, the app "just doesn't
//! load", and nothing the page surfaces says why. So the claims are checked on OBSERVABLE behaviour:
//!
//!   * **`createIndex` inside `onupgradeneeded`** registers a secondary key, and `indexNames` lists it.
//!   * **`index.get(key)`** finds the record whose index-key matches — a different lookup than `get`.
//!   * **`index.getAll(IDBKeyRange.bound(...))`** returns records across a key SPAN, in index order.
//!   * **Index order is the index key's order**, not primary-key order and not insertion order.
//!   * **`multiEntry`** puts a record into the index once per element of an array-valued key.
//!   * **A `unique` index refuses** a second record with the same index key — a `ConstraintError`,
//!     the difference between "email is unique" enforced and merely hoped for.
//!   * **Indexes PERSIST.** A reopen at the same version fires no upgrade, so `createIndex` never
//!     re-runs — yet `store.index(...)` must still resolve, or every returning visit is broken.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var r = [];
    var done = function () { document.getElementById('out').textContent = r.join(' '); };

    var req = indexedDB.open('people', 1);
    req.onupgradeneeded = function (e) {
        var db = e.target.result;
        var st = db.createObjectStore('users', { keyPath: 'id' });
        st.createIndex('by_email', 'email', { unique: true });
        st.createIndex('by_age', 'age');
        st.createIndex('tags', 'tags', { multiEntry: true });
        r.push('idxnames:' + st.indexNames.contains('by_email') + '/' + st.indexNames.contains('by_age'));
        st.put({ id: 1, email: 'ann@x', age: 30, tags: ['red', 'blue'] });
        st.put({ id: 2, email: 'bob@x', age: 20, tags: ['red'] });
        st.put({ id: 3, email: 'cal@x', age: 40, tags: ['green'] });
    };

    req.onsuccess = function () {
        var db = req.result;
        var tx = db.transaction('users', 'readonly');
        var st = tx.objectStore('users');
        var byEmail = st.index('by_email');
        var byAge = st.index('by_age');
        var tags = st.index('tags');

        // A lookup by a NON-primary key.
        byEmail.get('bob@x').onsuccess = function (e) { r.push('get:' + (e.target.result && e.target.result.id)); };
        byEmail.getKey('cal@x').onsuccess = function (e) { r.push('getkey:' + e.target.result); };

        // A RANGE query, returned in index order. bound(25,45) spans ann(30) and cal(40), not bob(20).
        byAge.getAll(IDBKeyRange.bound(25, 45)).onsuccess = function (e) {
            r.push('range:' + e.target.result.map(function (u) { return u.id; }).join(','));
        };
        // Index order is by the AGE key: 20,30,40 → ids 2,1,3 (neither primary nor insertion order).
        byAge.getAllKeys().onsuccess = function (e) { r.push('order:' + e.target.result.join(',')); };
        byAge.count().onsuccess = function (e) { r.push('count:' + e.target.result); };

        // multiEntry: 'red' is in ann.tags AND bob.tags, so it matches both; first by primary key is 1.
        tags.get('red').onsuccess = function (e) { r.push('multi:' + (e.target.result && e.target.result.id)); };

        // A cursor over the index yields (primaryKey @ indexKey) in index order.
        var seen = [];
        byAge.openCursor().onsuccess = function (e) {
            var c = e.target.result;
            if (c) { seen.push(c.primaryKey + '@' + c.key); c.continue(); return; }
            r.push('cursor:' + seen.join(','));
        };

        tx.oncomplete = function () {
            // UNIQUE: a second record with an existing index key is refused, not written.
            var tx2 = db.transaction('users', 'readwrite');
            var dup = tx2.objectStore('users').add({ id: 9, email: 'ann@x', age: 99 });
            dup.onerror = function (e) {
                r.push('unique:' + (e.target.error && e.target.error.name));
                e.stopPropagation && e.stopPropagation();
            };
            tx2.onabort = function () {
                // PERSISTENCE: reopen at the SAME version. No upgrade fires, so `createIndex` does
                // not run again — yet the index must still be there, or a returning visit is broken.
                var again = indexedDB.open('people', 1);
                var reUp = false;
                again.onupgradeneeded = function () { reUp = true; };
                again.onsuccess = function () {
                    var db2 = again.result;
                    r.push('reopen:' + reUp);
                    var st2 = db2.transaction('users').objectStore('users');
                    r.push('persistnames:' + st2.indexNames.contains('by_email'));
                    st2.index('by_email').get('cal@x').onsuccess = function (e) {
                        r.push('persistget:' + (e.target.result && e.target.result.id));
                        db2.close();
                        indexedDB.deleteDatabase('people');
                        done();
                    };
                };
            };
        };
    };
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn indexeddb_indexes_are_a_real_persistent_secondary_key() {
    let fonts = FontContext::new();
    // A unique origin per run: the store is origin-partitioned and persisted, so sharing an origin
    // with a previous run would let stale rows answer the persistence claim.
    let origin = format!(
        "https://idb-index-gate-{}.test/",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let page = manuk_page::Page::load(HTML, &origin, &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "idxnames:true/true",    // createIndex registered the index; indexNames lists it
        "get:2",                 // index.get finds by a NON-primary key (bob, by email)
        "getkey:3",              // index.getKey returns the primary key for an index key (cal)
        "range:1,3",             // IDBKeyRange.bound spans ann(30) and cal(40), in index order
        "order:2,1,3",           // index order is the AGE key's order: 20,30,40 → ids 2,1,3
        "count:3",               // three records are in the by_age index
        "multi:1",               // multiEntry: 'red' matches ann and bob; first by primary key is 1
        "cursor:2@20,1@30,3@40", // the index cursor walks in index-key order
        "unique:ConstraintError", // a unique index refuses a duplicate index key
        "reopen:false",          // re-opening at the same version does NOT re-upgrade
        "persistnames:true",     // and the index survived — it is persisted, not held in JS
        "persistget:3",          // and still resolves records after the reopen
    ] {
        assert!(
            got.contains(claim),
            "G_INDEXEDDB_INDEX: expected {claim} in {got:?}\n  \
             `store.index()` / `createIndex` / `IDBKeyRange` must be a real, ordered, PERSISTENT \
             secondary key. Its absence is not a reported failure — the auth SDKs that depend on it \
             throw inside their own promises and the app silently fails to load."
        );
    }
}
