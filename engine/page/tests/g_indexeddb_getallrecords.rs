//! **G_INDEXEDDB_GETALLRECORDS — `getAllRecords(options)` must return full `{ key, primaryKey,
//! value }` records in one call, on both the store and an index (Interop 2026).**
//!
//! `getAllRecords()` is the Interop-2026 IndexedDB addition (20%-weight focus set). It collapses the
//! old two-call idiom — `getAll()` for values, `getAllKeys()` for keys, then a client-side zip — into
//! a single request that returns records: `{ key, primaryKey, value }`. On an OBJECT STORE the index
//! key IS the primary key, so `key === primaryKey`. On an INDEX they differ: `key` is the index key
//! (the value property) and `primaryKey` is the store key — the exact pair a cursor exposes, but
//! materialized in one array. Dexie 4, `idb`, and the Firebase/Cognito offline layers reach for it
//! wherever they page a keyed range, and its absence is the same silent-degrade shape as a missing
//! index: `store.getAllRecords` is `undefined`, `undefined(...)` throws inside the SDK's own promise,
//! and the app "just doesn't load".
//!
//! The claims are checked on OBSERVABLE record shape and order — the `key !== primaryKey` split on the
//! index is what proves this is `getAllRecords`, not `getAll` wearing its name:
//!
//!   * **Store `getAllRecords()`** returns `{ key, primaryKey, value }` in store-key order, `key ==
//!     primaryKey`.
//!   * **`{ query }`** filters to an `IDBKeyRange`; **`{ count }`** caps the result; **`{ direction:
//!     'prev' }`** reverses it.
//!   * **Index `getAllRecords()`** returns records in INDEX-key order with `key` = index key and
//!     `primaryKey` = store key (they differ — the RED-prover against a `getAll` stand-in).
//!   * **`{ direction: 'nextunique' }`** on an index with a duplicate index key returns one record per
//!     distinct index key — the one with the smallest primary key.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var r = [];
    var done = function () { document.getElementById('out').textContent = r.join(' '); };
    // key/primaryKey/value.age of each record, joined — the shape that separates getAllRecords from getAll.
    var shape = function (recs) {
        return recs.map(function (x) { return x.key + '/' + x.primaryKey + '/' + x.value.age; }).join(',');
    };

    var req = indexedDB.open('recs', 1);
    req.onupgradeneeded = function (e) {
        var st = e.target.result.createObjectStore('users', { keyPath: 'id' });
        st.createIndex('by_age', 'age');
        st.put({ id: 1, age: 30 });
        st.put({ id: 2, age: 20 });
        st.put({ id: 3, age: 40 });
        st.put({ id: 4, age: 20 }); // a DUPLICATE index key (age 20) — the nextunique prover
    };

    req.onsuccess = function () {
        var db = req.result;
        var st = db.transaction('users', 'readonly').objectStore('users');
        var byAge = st.index('by_age');

        // STORE: records in store-key order; key === primaryKey === id.
        st.getAllRecords().onsuccess = function (e) { r.push('store:' + shape(e.target.result)); };
        // STORE + query: bound(2,3) keeps ids 2,3.
        st.getAllRecords({ query: IDBKeyRange.bound(2, 3) }).onsuccess = function (e) { r.push('sq:' + shape(e.target.result)); };
        // STORE + count: first 2 in store-key order → ids 1,2.
        st.getAllRecords({ count: 2 }).onsuccess = function (e) { r.push('sc:' + shape(e.target.result)); };
        // STORE + direction prev: reversed store-key order → 4,3,2,1.
        st.getAllRecords({ direction: 'prev' }).onsuccess = function (e) { r.push('sp:' + shape(e.target.result)); };

        // INDEX: index-key order (age 20,20,30,40). key = age, primaryKey = id — THEY DIFFER.
        byAge.getAllRecords().onsuccess = function (e) { r.push('idx:' + shape(e.target.result)); };
        // INDEX + range: bound(25,45) spans age 30 and 40 → ids 1,3.
        byAge.getAllRecords({ query: IDBKeyRange.bound(25, 45) }).onsuccess = function (e) { r.push('ir:' + shape(e.target.result)); };
        // INDEX + nextunique: one record per distinct age; age 20 → smallest primary key (id 2).
        byAge.getAllRecords({ direction: 'nextunique' }).onsuccess = function (e) { r.push('iu:' + shape(e.target.result)); };

        db.transaction('users').oncomplete = function () {
            db.close();
            indexedDB.deleteDatabase('recs');
            done();
        };
    };
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn getallrecords_returns_full_records_on_store_and_index() {
    let fonts = FontContext::new();
    // A unique origin per run: the store is origin-partitioned and persisted, so sharing an origin
    // with a previous run would let stale rows answer the shape claims.
    let origin = format!(
        "https://idb-getallrecords-gate-{}.test/",
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
        "store:1/1/30,2/2/20,3/3/40,4/4/20", // store record: key == primaryKey, in store-key order
        "sq:2/2/20,3/3/40",                  // { query } filters to the IDBKeyRange
        "sc:1/1/30,2/2/20",                  // { count } caps the result
        "sp:4/4/20,3/3/40,2/2/20,1/1/30",    // { direction:'prev' } reverses it
        "idx:20/2/20,20/4/20,30/1/30,40/3/40", // INDEX: key(age) != primaryKey(id), in index-key order
        "ir:30/1/30,40/3/40",                  // index { query } range spans age 30 and 40
        "iu:20/2/20,30/1/30,40/3/40", // nextunique: one per distinct age; age 20 → smallest pk (id 2)
    ] {
        assert!(
            got.contains(claim),
            "G_INDEXEDDB_GETALLRECORDS: expected {claim} in {got:?}\n  \
             `getAllRecords(options)` must return full `{{ key, primaryKey, value }}` records in one \
             call on both the store (key == primaryKey) and an index (key = index key, primaryKey = \
             store key). Its absence throws inside the SDK's own promise and the app silently fails."
        );
    }
}
