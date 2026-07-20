//! **G_INDEXEDDB — `indexedDB` must be a real, persistent, transactional store, not a feature flag.**
//!
//! Web Storage is a string map with a 5 MiB ceiling. Everything past a preferences blob uses
//! IndexedDB: offline caches, draft documents, the session layer of the AWS and GCP consoles, every
//! app that claims to work without a network. Before this gate the engine had **no `indexedDB` at
//! all** — `grep -ril indexeddb engine/ shell/` returned nothing.
//!
//! And an absent IDB is not a missing feature the page reports. It is a **grading signal**, the
//! exact shape that cost an hour at the Web Storage tick: a page does
//! `if (!window.indexedDB) { /* degraded path */ }`, takes the lesser branch, throws nothing, and
//! looks like a layout bug for the rest of the session.
//!
//! The claims below are the ones a real app actually depends on, and each is one a plausible-looking
//! shim gets wrong:
//!
//!   * **The async shape is real.** `onsuccess` must not fire before `open()` has returned, or the
//!     page's own `req` variable is still `undefined` inside its handler.
//!   * **`onupgradeneeded` runs, once, with the right versions**, and stores created there are
//!     writable *from inside the handler* — which is where every app seeds its schema.
//!   * **A missing key is `undefined`, never an error.** Pages branch on that to mean "cache miss".
//!   * **Values are structured clones, not JSON.** A `Date` that comes back as a string is a silent
//!     type change, the worst kind: nothing throws and every later comparison is wrong.
//!   * **Key ORDER is IndexedDB's key order.** Naive string keys sort 10 before 9, so `getAll`
//!     returns rows in an order the page never asked for.
//!   * **`add()` refuses an existing key** with a `ConstraintError` rather than overwriting — the
//!     difference between a de-dup check that works and one that silently destroys a record.
//!   * **`abort()` really rolls back.** Firing `onabort` while the write stays applied is worse than
//!     having no transactions at all, because the page believes it undid something.
//!   * **It persists.** A second `open()` of the same origin's database sees the rows — that is the
//!     entire point of choosing IDB over `sessionStorage`.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var r = [];
    var done = function () { document.getElementById('out').textContent = r.join(' '); };

    // The feature detection every app runs first.
    r.push('present:' + (typeof indexedDB === 'object' && typeof indexedDB.open === 'function'));

    var req = indexedDB.open('shop', 1);
    // ASYNC SHAPE: a synchronous shim would already have fired by now and `sawUpgrade` would be
    // set before this line runs.
    var firedEarly = (typeof sawUpgrade !== 'undefined');
    var sawUpgrade = false, oldV = -1, newV = -1;

    req.onupgradeneeded = function (e) {
        sawUpgrade = true; oldV = e.oldVersion; newV = e.newVersion;
        var db = e.target.result;
        var st = db.createObjectStore('items', { keyPath: 'id' });
        // Seeding INSIDE the upgrade handler is what every real app does.
        st.put({ id: 2, name: 'b', when: new Date(86400000) });
        st.put({ id: 10, name: 'j' });
        st.put({ id: 9, name: 'i' });
    };

    req.onsuccess = function () {
        var db = req.result;
        r.push('early:' + firedEarly);          // must be false — callbacks are NOT synchronous
        r.push('upgrade:' + sawUpgrade + '/' + oldV + '/' + newV);
        r.push('names:' + db.objectStoreNames.contains('items'));

        var tx = db.transaction('items', 'readwrite');
        var st = tx.objectStore('items');

        // A missing key is `undefined`, not an error.
        st.get(999).onsuccess = function (e) { r.push('miss:' + (e.target.result === undefined)); };

        // A Date must come back as a Date, with its time intact.
        st.get(2).onsuccess = function (e) {
            var v = e.target.result;
            r.push('clone:' + (v.when instanceof Date) + '/' + (v.when.getTime() === 86400000));
        };

        // KEY ORDER: 2, 9, 10 — not the lexicographic 10, 2, 9.
        st.getAllKeys().onsuccess = function (e) { r.push('order:' + e.target.result.join(',')); };

        // add() must refuse a live key.
        var dup = st.add({ id: 2, name: 'clash' });
        dup.onerror = function (e) {
            r.push('dup:' + (e.target.error && e.target.error.name));
            // The failed request aborted this transaction; do the rest in a fresh one.
            e.stopPropagation && e.stopPropagation();
        };

        tx.onabort = function () {
            // ROLLBACK, tested against a write that ACTUALLY LANDED.
            //
            // The first version of this claim read record 2 after the failed `add()` above and
            // asserted it was still 'b'. That passed against a build whose `abort()` rolled back
            // NOTHING — because a rejected `add()` never wrote anything, so there was nothing to
            // undo and the claim was vacuous. It is written the hard way now: overwrite the record
            // with a put that SUCCEEDS, abort the transaction from inside its own success handler,
            // and require the old value back.
            var tx2 = db.transaction('items', 'readwrite');
            var st2 = tx2.objectStore('items');
            st2.put({ id: 2, name: 'OVERWRITTEN' }).onsuccess = function () { tx2.abort(); };
            tx2.onabort = function () {
                db.transaction('items').objectStore('items').get(2).onsuccess = function (e) {
                    r.push('rollback:' + e.target.result.name);
                };
            };

            // CURSOR: walk every row in key order.
            var tx3 = db.transaction('items', 'readonly');
            var seen = [];
            tx3.objectStore('items').openCursor().onsuccess = function (e) {
                var c = e.target.result;
                if (c) { seen.push(c.key); c.continue(); return; }
                r.push('cursor:' + seen.join(','));

                // PERSISTENCE: re-open the SAME database. No upgrade this time, and the rows are
                // still there — this is the whole reason a page picks IDB over sessionStorage.
                var again = indexedDB.open('shop', 1);
                var reUpgraded = false;
                again.onupgradeneeded = function () { reUpgraded = true; };
                again.onsuccess = function () {
                    r.push('reopen:' + again.result.version + '/' + reUpgraded);
                    again.result.transaction('items').objectStore('items').count().onsuccess = function (ev) {
                        r.push('persist:' + ev.target.result);
                        // Clean up after ourselves: the store is persisted to the REAL profile, so
                        // a gate that left its database behind would grow that file on every run
                        // forever. This also exercises `deleteDatabase`.
                        indexedDB.deleteDatabase('shop').onsuccess = function () {
                            var gone = indexedDB.open('shop', 1);
                            gone.onupgradeneeded = function () { r.push('deleted:true'); };
                            gone.onsuccess = function () {
                                gone.result.close();
                                indexedDB.deleteDatabase('shop');
                                done();
                            };
                        };
                    };
                };
            };
        };
    };
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn indexeddb_is_a_real_transactional_persistent_store() {
    let fonts = FontContext::new();
    // A unique origin per run: the store is origin-partitioned and persisted, so sharing an origin
    // with a previous run would let stale rows answer the persistence claim.
    let origin = format!(
        "https://idb-gate-{}.test/",
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
        "present:true",        // the feature detection every app runs first
        "early:false",         // callbacks are delivered on a microtask, never inline
        "upgrade:true/0/1",    // onupgradeneeded ran once, with the right versions
        "names:true",          // the store created in the handler is on the database
        "miss:true",           // a missing key is `undefined`, never an error
        "clone:true/true",     // a Date round-trips as a Date — structured clone, not JSON
        "order:2,9,10",        // IndexedDB key order, not lexicographic ("10,2,9")
        "dup:ConstraintError", // add() refuses a live key instead of overwriting
        "rollback:b",          // abort() really undid the write, it did not just fire an event
        "cursor:2,9,10",       // the cursor walks every row, in key order
        "reopen:1/false",      // re-opening at the same version does NOT re-upgrade
        "persist:3",           // and the rows are still there
        "deleted:true",        // deleteDatabase really removes it (the re-open upgrades from 0)
    ] {
        assert!(
            got.contains(claim),
            "G_INDEXEDDB: expected {claim} in {got:?}\n  \
             `indexedDB` must be a real transactional, structured, PERSISTENT store. Its absence is \
             not a reported failure — pages feature-detect it and silently take a degraded path, \
             which is why this is gated on observable behaviour rather than on the symbol existing."
        );
    }
}
