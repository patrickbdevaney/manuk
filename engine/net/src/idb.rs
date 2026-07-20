//! **IndexedDB** — the structured, origin-partitioned store the app web assumes exists.
//!
//! `localStorage` is a string map with a 5 MiB ceiling and a synchronous API. Everything past a
//! preferences blob uses **IndexedDB**: offline caches, draft documents, the auth/session layers of
//! the AWS and GCP consoles, every "works on a plane" PWA. And like `localStorage` before it, its
//! absence is not a missing feature — it is a **grading signal**. A page that does
//! `if (!window.indexedDB) { /* degraded path */ }` does not report a bug; it silently becomes a
//! lesser page, which is the hardest class of failure to see from the outside.
//!
//! **Why this file is a plain versioned envelope and not redb.** The board says *borrow redb/heed*,
//! and for a large durable KV that is right. But the store is not where IndexedDB is hard — the
//! hard part is the **request/transaction/upgrade semantics**, which live in the JS shim above this.
//! Using the same serde envelope Web Storage already uses costs zero build time, zero new
//! dependency surface, and keeps this tick atomic. When a real workload puts megabytes through it,
//! swapping the backing map for redb is a contained change *behind these functions* — the shape of
//! this API does not assume a `BTreeMap`. That upgrade path is recorded in `docs/wiki/storage.md`
//! rather than left as a discovery.
//!
//! **Key encoding is the JS layer's job.** Records carry an opaque, *sortable* key string plus the
//! original key's JSON. The store never interprets either: it sorts by the encoded key (so
//! `getAll`/`count`/cursor order is IndexedDB's key order) and hands the JSON back untouched, so a
//! numeric key round-trips as a number and never as `"3"`. A store that decides key types would
//! have to re-derive the spec's type ordering here, in the wrong layer, twice.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// One object store: its creation options plus its records.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ObjectStore {
    /// The store's `keyPath` (in-line keys), or empty for out-of-line keys.
    pub key_path: String,
    /// Whether the store mints keys itself.
    pub auto_increment: bool,
    /// The next key `autoIncrement` will mint. Per spec this only ever moves forward, including
    /// across an explicit key write that is larger — a store that reused a key after a delete
    /// would silently overwrite a live record.
    pub next_key: i64,
    /// encoded-sortable-key → (original key JSON, value JSON)
    pub records: BTreeMap<String, Record>,
}

/// A stored record. The key is kept in its original JSON form so `IDBRequest.result` and
/// `cursor.key` return the type the page wrote, not a stringified shadow of it.
#[derive(Clone, Serialize, Deserialize)]
pub struct Record {
    /// The key as JSON — `3`, `"abc"`, `[1,2]`.
    pub key: String,
    /// The value as the shim's tagged-JSON encoding.
    pub value: String,
}

/// One database: a version and its stores.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Database {
    pub version: u64,
    pub stores: BTreeMap<String, ObjectStore>,
}

/// The on-disk envelope, versioned so a later format migrates rather than discarding user data
/// (ADR-009 — the same contract the cookie jar and `localStorage` keep).
#[derive(Default, Serialize, Deserialize)]
struct Persisted {
    version: u32,
    /// origin → (database name → database)
    origins: BTreeMap<String, BTreeMap<String, Database>>,
}

const FORMAT_VERSION: u32 = 1;

/// Per-origin quota. Larger than Web Storage's 5 MiB because that ceiling is exactly why pages
/// reach for IndexedDB — but still real: an unbounded map is a memory-exhaustion vector for a
/// hostile page, and a quota that is never enforced is not a quota.
const QUOTA_BYTES: usize = 64 * 1024 * 1024;

#[derive(Default)]
struct State {
    origins: BTreeMap<String, BTreeMap<String, Database>>,
    loaded: bool,
    dirty: bool,
}

fn state() -> &'static Mutex<State> {
    static S: std::sync::OnceLock<Mutex<State>> = std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}

/// Where the databases live — beside the cookie jar and `localStorage`, in the profile.
pub fn store_path() -> PathBuf {
    crate::cookie_store_path().with_file_name("indexeddb.json")
}

fn ensure_loaded(s: &mut State) {
    if s.loaded {
        return;
    }
    s.loaded = true;
    let Ok(text) = std::fs::read_to_string(store_path()) else {
        return;
    };
    match serde_json::from_str::<Persisted>(&text) {
        Ok(p) if p.version == FORMAT_VERSION => s.origins = p.origins,
        Ok(p) => tracing::warn!(
            found = p.version,
            expected = FORMAT_VERSION,
            "indexedDB: unknown format version; leaving the file untouched"
        ),
        Err(e) => tracing::warn!(error = %e, "indexedDB: could not parse; starting empty"),
    }
}

fn used_bytes(dbs: &BTreeMap<String, Database>) -> usize {
    dbs.values()
        .flat_map(|d| d.stores.values())
        .flat_map(|st| st.records.values())
        .map(|r| r.key.len() + r.value.len())
        .sum()
}

/// The database's current version and store definitions, creating an empty version-0 entry if the
/// origin has never opened it. Version 0 is how the shim knows to fire `upgradeneeded`.
pub fn open(origin: &str, db: &str) -> Database {
    let Ok(mut s) = state().lock() else {
        return Database::default();
    };
    ensure_loaded(&mut s);
    s.origins
        .entry(origin.to_string())
        .or_default()
        .entry(db.to_string())
        .or_default()
        .clone()
}

/// Commit an upgrade transaction: the new version plus the stores as they stand after the page's
/// `onupgradeneeded` handler ran. Stores absent from `keep` were deleted by the handler and their
/// records go with them.
pub fn commit_upgrade(origin: &str, db: &str, version: u64, stores: Vec<(String, String, bool)>) {
    let Ok(mut s) = state().lock() else {
        return;
    };
    ensure_loaded(&mut s);
    let entry = s
        .origins
        .entry(origin.to_string())
        .or_default()
        .entry(db.to_string())
        .or_default();
    entry.version = version;
    let keep: Vec<&String> = stores.iter().map(|(n, _, _)| n).collect();
    entry.stores.retain(|name, _| keep.contains(&name));
    for (name, key_path, auto_increment) in &stores {
        let st = entry.stores.entry(name.clone()).or_default();
        // A store that already existed keeps its records and its counter; only a NEW store takes
        // its options from this call. Re-applying options to a live store would silently rewrite
        // its keyPath out from under records already keyed by the old one.
        if st.next_key == 0 && st.records.is_empty() {
            st.key_path = key_path.clone();
            st.auto_increment = *auto_increment;
            st.next_key = 1;
        }
    }
    s.dirty = true;
}

/// Drop a database entirely.
pub fn delete_database(origin: &str, db: &str) {
    let Ok(mut s) = state().lock() else {
        return;
    };
    ensure_loaded(&mut s);
    if let Some(dbs) = s.origins.get_mut(origin) {
        dbs.remove(db);
    }
    s.dirty = true;
}

/// Reserve and return the next `autoIncrement` key for a store.
pub fn next_auto_key(origin: &str, db: &str, store: &str) -> i64 {
    let Ok(mut s) = state().lock() else {
        return 1;
    };
    ensure_loaded(&mut s);
    let Some(st) = s
        .origins
        .get_mut(origin)
        .and_then(|d| d.get_mut(db))
        .and_then(|d| d.stores.get_mut(store))
    else {
        return 1;
    };
    let k = st.next_key.max(1);
    st.next_key = k + 1;
    s.dirty = true;
    k
}

/// Read one record. `None` is "no such key" — which the shim turns into `undefined`, the value
/// IndexedDB reports for a missing key (never an error).
pub fn get(origin: &str, db: &str, store: &str, enc_key: &str) -> Option<Record> {
    let Ok(mut s) = state().lock() else {
        return None;
    };
    ensure_loaded(&mut s);
    s.origins
        .get(origin)?
        .get(db)?
        .stores
        .get(store)?
        .records
        .get(enc_key)
        .cloned()
}

/// The outcome of a write, so the shim can raise the right `DOMException`.
#[derive(PartialEq, Eq, Debug)]
pub enum PutResult {
    Ok,
    /// `add()` against a key that already exists.
    ConstraintError,
    /// The origin is over quota.
    QuotaExceeded,
    /// No such store — the transaction named a store that does not exist.
    NotFound,
}

/// Write one record. `no_overwrite` is `add()` semantics.
pub fn put(
    origin: &str,
    db: &str,
    store: &str,
    enc_key: &str,
    key_json: &str,
    value: &str,
    no_overwrite: bool,
) -> PutResult {
    let Ok(mut s) = state().lock() else {
        return PutResult::NotFound;
    };
    ensure_loaded(&mut s);
    let Some(dbs) = s.origins.get_mut(origin) else {
        return PutResult::NotFound;
    };
    let Some(st) = dbs.get_mut(db).and_then(|d| d.stores.get_mut(store)) else {
        return PutResult::NotFound;
    };
    if no_overwrite && st.records.contains_key(enc_key) {
        return PutResult::ConstraintError;
    }
    st.records.insert(
        enc_key.to_string(),
        Record {
            key: key_json.to_string(),
            value: value.to_string(),
        },
    );
    // Quota is checked AFTER the insert against the whole origin, then rolled back — the write
    // that crosses the line is the one refused, and the store never keeps a record it reported as
    // rejected. (A pre-check would have to model the delta of an overwrite.)
    if used_bytes(dbs) > QUOTA_BYTES {
        if let Some(st) = dbs.get_mut(db).and_then(|d| d.stores.get_mut(store)) {
            st.records.remove(enc_key);
        }
        return PutResult::QuotaExceeded;
    }
    s.dirty = true;
    PutResult::Ok
}

/// Delete one record, returning what was there (so a transaction can undo it on `abort()`).
pub fn delete(origin: &str, db: &str, store: &str, enc_key: &str) -> Option<Record> {
    let Ok(mut s) = state().lock() else {
        return None;
    };
    ensure_loaded(&mut s);
    let prev = s
        .origins
        .get_mut(origin)
        .and_then(|d| d.get_mut(db))
        .and_then(|d| d.stores.get_mut(store))
        .and_then(|st| st.records.remove(enc_key));
    s.dirty = true;
    prev
}

/// Every record in a store, **in IndexedDB key order** (the `BTreeMap` is ordered by the shim's
/// sortable encoding, which is the whole reason the encoding exists).
pub fn records(origin: &str, db: &str, store: &str) -> Vec<(String, Record)> {
    let Ok(mut s) = state().lock() else {
        return Vec::new();
    };
    ensure_loaded(&mut s);
    s.origins
        .get(origin)
        .and_then(|d| d.get(db))
        .and_then(|d| d.stores.get(store))
        .map(|st| {
            st.records
                .iter()
                .map(|(k, r)| (k.clone(), r.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// Empty a store, returning what it held (so `abort()` can restore it).
pub fn clear(origin: &str, db: &str, store: &str) -> Vec<(String, Record)> {
    let prev = records(origin, db, store);
    let Ok(mut s) = state().lock() else {
        return prev;
    };
    if let Some(st) = s
        .origins
        .get_mut(origin)
        .and_then(|d| d.get_mut(db))
        .and_then(|d| d.stores.get_mut(store))
    {
        st.records.clear();
    }
    s.dirty = true;
    prev
}

/// The origin's database names — `indexedDB.databases()`.
pub fn database_names(origin: &str) -> Vec<(String, u64)> {
    let Ok(mut s) = state().lock() else {
        return Vec::new();
    };
    ensure_loaded(&mut s);
    s.origins
        .get(origin)
        .map(|dbs| {
            dbs.iter()
                .filter(|(_, d)| d.version > 0)
                .map(|(n, d)| (n.clone(), d.version))
                .collect()
        })
        .unwrap_or_default()
}

/// Flush to the profile. Called at the same points the cookie jar and `localStorage` flush.
pub fn save() {
    let Ok(mut s) = state().lock() else {
        return;
    };
    if !s.dirty {
        return;
    }
    let p = Persisted {
        version: FORMAT_VERSION,
        origins: s.origins.clone(),
    };
    let path = store_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match serde_json::to_string(&p).map(|t| std::fs::write(&path, t)) {
        Ok(Ok(())) => s.dirty = false,
        Ok(Err(e)) => tracing::warn!(error = %e, "indexedDB: write failed"),
        Err(e) => tracing::warn!(error = %e, "indexedDB: serialize failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_creates_stores_and_records_survive_a_later_upgrade() {
        let o = "https://idb-one.test";
        commit_upgrade(o, "app", 1, vec![("notes".into(), "id".into(), true)]);
        assert_eq!(open(o, "app").version, 1);

        assert_eq!(
            put(
                o,
                "app",
                "notes",
                "n0000000001",
                "1",
                "{\"t\":\"hi\"}",
                false
            ),
            PutResult::Ok
        );
        // A second upgrade that keeps the store must NOT wipe its records or reset its counter —
        // that would silently delete user data on a routine version bump.
        let k = next_auto_key(o, "app", "notes");
        commit_upgrade(
            o,
            "app",
            2,
            vec![
                ("notes".into(), "id".into(), true),
                ("tags".into(), String::new(), false),
            ],
        );
        assert_eq!(open(o, "app").version, 2);
        assert_eq!(records(o, "app", "notes").len(), 1);
        assert!(next_auto_key(o, "app", "notes") > k, "counter must advance");

        // A store dropped from the upgrade is really gone.
        commit_upgrade(o, "app", 3, vec![("tags".into(), String::new(), false)]);
        assert!(open(o, "app").stores.get("notes").is_none());
    }

    #[test]
    fn add_refuses_an_existing_key_and_origins_cannot_see_each_other() {
        let a = "https://idb-a.test";
        let b = "https://idb-b.test";
        commit_upgrade(a, "d", 1, vec![("s".into(), String::new(), false)]);
        commit_upgrade(b, "d", 1, vec![("s".into(), String::new(), false)]);
        assert_eq!(put(a, "d", "s", "sk", "\"k\"", "1", true), PutResult::Ok);
        assert_eq!(
            put(a, "d", "s", "sk", "\"k\"", "2", true),
            PutResult::ConstraintError,
            "add() against a live key must be a ConstraintError, not a silent overwrite"
        );
        // put() may overwrite.
        assert_eq!(put(a, "d", "s", "sk", "\"k\"", "2", false), PutResult::Ok);
        assert_eq!(get(a, "d", "s", "sk").unwrap().value, "2");
        // The other origin never saw any of it.
        assert!(get(b, "d", "s", "sk").is_none());
    }

    #[test]
    fn records_come_back_in_encoded_key_order() {
        let o = "https://idb-order.test";
        commit_upgrade(o, "d", 1, vec![("s".into(), String::new(), false)]);
        for k in ["n003", "n001", "n002"] {
            put(o, "d", "s", k, "0", "v", false);
        }
        let got: Vec<String> = records(o, "d", "s").into_iter().map(|(k, _)| k).collect();
        assert_eq!(got, vec!["n001", "n002", "n003"]);
    }

    #[test]
    fn a_write_over_quota_is_refused_and_leaves_nothing_behind() {
        let o = "https://idb-quota.test";
        commit_upgrade(o, "d", 1, vec![("s".into(), String::new(), false)]);
        let big = "x".repeat(QUOTA_BYTES + 1);
        assert_eq!(
            put(o, "d", "s", "k", "0", &big, false),
            PutResult::QuotaExceeded
        );
        assert!(
            get(o, "d", "s", "k").is_none(),
            "a refused write must not be readable — reporting a rejection and keeping the record \
             is worse than either outcome alone"
        );
    }
}
