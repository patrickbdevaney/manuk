//! **The Cache API** — `caches`, the request/response store the offline web is built on.
//!
//! `localStorage` holds strings and IndexedDB holds structured values; neither can hold a
//! **response**. The Cache API is the only storage in the platform whose unit is an HTTP
//! request/response pair, which is why it — not IndexedDB — is what a PWA's install step fills and
//! what a Service Worker's `fetch` handler reads. Its absence has the same grading shape every
//! storage API before it had: `if ('caches' in window)` does not report a bug, it silently selects
//! the network-only path, and a page that was designed to work offline simply stops being that page.
//!
//! **Why the store is a plain versioned envelope, again.** Same reasoning as `idb`: the difficulty
//! here is not the bytes, it is the **matching semantics** (method filtering, `ignoreSearch`, `Vary`,
//! insertion order) which live in the JS shim. Reusing the serde envelope that the cookie jar,
//! Web Storage and IndexedDB already keep costs no build time and no new dependency, and swapping
//! the backing map later is a contained change behind these functions.
//!
//! **Entries are a list, not a map, and that is deliberate.** `cache.keys()` is specified to return
//! requests **in insertion order**, and `cache.matchAll()` may return several entries for one URL
//! that differ by `Vary`. A `BTreeMap` keyed by URL would quietly make both of those impossible —
//! the second `put` would overwrite a response the spec says must coexist.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// One cached request/response pair.
#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    /// The request URL, already serialised by the shim.
    pub url: String,
    /// The request method. Only `GET` is cacheable per spec; kept so a non-GET `put` can be
    /// rejected in the layer that knows what a `TypeError` is.
    pub method: String,
    pub status: u16,
    pub status_text: String,
    /// Response headers, lowercased names, in insertion order.
    pub headers: Vec<(String, String)>,
    /// The request headers the response's `Vary` names, so a `Vary`-aware match can compare them
    /// without re-fetching. Empty when the response does not vary.
    pub vary: Vec<(String, String)>,
    /// The body. Base64 when `body_b64`, otherwise the literal text — text bodies stay readable in
    /// the profile, which matters the first time someone has to debug a stale cache by hand.
    pub body: String,
    pub body_b64: bool,
}

/// The on-disk envelope, versioned so a later format migrates rather than discarding user data
/// (ADR-009 — the contract the cookie jar, `localStorage` and IndexedDB all keep).
#[derive(Default, Serialize, Deserialize)]
struct Persisted {
    version: u32,
    /// origin → (cache name → entries, in insertion order)
    origins: BTreeMap<String, BTreeMap<String, Vec<Entry>>>,
}

const FORMAT_VERSION: u32 = 1;

/// Per-origin quota. Sized like IndexedDB's rather than Web Storage's: a PWA's install step caches
/// its whole app shell, and a ceiling that a normal install trips is a ceiling that trains pages to
/// avoid the API. Still real — an unbounded store is a disk-exhaustion vector for a hostile page.
const QUOTA_BYTES: usize = 64 * 1024 * 1024;

#[derive(Default)]
struct State {
    origins: BTreeMap<String, BTreeMap<String, Vec<Entry>>>,
    loaded: bool,
    dirty: bool,
}

fn state() -> &'static Mutex<State> {
    static S: std::sync::OnceLock<Mutex<State>> = std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}

/// Where the caches live — beside the cookie jar, `localStorage` and the databases.
pub fn store_path() -> PathBuf {
    crate::cookie_store_path().with_file_name("cachestorage.json")
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
            "caches: unknown format version; leaving the file untouched"
        ),
        Err(e) => tracing::warn!(error = %e, "caches: could not parse; starting empty"),
    }
}

fn used_bytes(caches: &BTreeMap<String, Vec<Entry>>) -> usize {
    caches
        .values()
        .flatten()
        .map(|e| e.url.len() + e.body.len())
        .sum()
}

/// Create the named cache if it does not exist. `caches.open()` is specified to create on demand,
/// so this is idempotent and always succeeds.
pub fn open(origin: &str, name: &str) {
    let Ok(mut s) = state().lock() else {
        return;
    };
    ensure_loaded(&mut s);
    let entry = s
        .origins
        .entry(origin.to_string())
        .or_default()
        .entry(name.to_string());
    if let std::collections::btree_map::Entry::Vacant(v) = entry {
        v.insert(Vec::new());
        s.dirty = true;
    }
}

/// Whether the named cache exists. Distinct from "is non-empty" — an empty cache that was opened
/// still answers `true`, which is what `caches.has()` promises.
pub fn has(origin: &str, name: &str) -> bool {
    let Ok(mut s) = state().lock() else {
        return false;
    };
    ensure_loaded(&mut s);
    s.origins.get(origin).is_some_and(|c| c.contains_key(name))
}

/// The origin's cache names.
pub fn cache_names(origin: &str) -> Vec<String> {
    let Ok(mut s) = state().lock() else {
        return Vec::new();
    };
    ensure_loaded(&mut s);
    s.origins
        .get(origin)
        .map(|c| c.keys().cloned().collect())
        .unwrap_or_default()
}

/// Drop a whole cache. Returns whether it existed, which is `caches.delete()`'s resolution value.
pub fn delete_cache(origin: &str, name: &str) -> bool {
    let Ok(mut s) = state().lock() else {
        return false;
    };
    ensure_loaded(&mut s);
    let existed = s
        .origins
        .get_mut(origin)
        .is_some_and(|c| c.remove(name).is_some());
    if existed {
        s.dirty = true;
    }
    existed
}

/// Every entry in a cache, in insertion order.
pub fn entries(origin: &str, name: &str) -> Vec<Entry> {
    let Ok(mut s) = state().lock() else {
        return Vec::new();
    };
    ensure_loaded(&mut s);
    s.origins
        .get(origin)
        .and_then(|c| c.get(name))
        .cloned()
        .unwrap_or_default()
}

/// The outcome of a `put`, so the shim can raise the right error rather than failing silently.
pub enum PutResult {
    Stored,
    /// The origin is over quota. `cache.put()` rejects with a `QuotaExceededError`.
    QuotaExceeded,
    /// The named cache was never opened.
    NoSuchCache,
}

/// Store a response, replacing any entry with the same URL, method **and** `Vary` key.
///
/// Replacement rather than append is the spec's behaviour and it is the one that matters: a page
/// that re-caches its app shell on every install must not grow the cache without bound. Entries
/// that differ only by `Vary` coexist, which is why the match is on all three fields.
pub fn put(origin: &str, name: &str, entry: Entry) -> PutResult {
    let Ok(mut s) = state().lock() else {
        return PutResult::NoSuchCache;
    };
    ensure_loaded(&mut s);
    let Some(caches) = s.origins.get_mut(origin) else {
        return PutResult::NoSuchCache;
    };
    if !caches.contains_key(name) {
        return PutResult::NoSuchCache;
    }
    let incoming = entry.url.len() + entry.body.len();
    let existing = caches
        .get(name)
        .and_then(|es| {
            es.iter()
                .find(|e| e.url == entry.url && e.method == entry.method && e.vary == entry.vary)
        })
        .map(|e| e.url.len() + e.body.len())
        .unwrap_or(0);
    // Quota is checked against the size AFTER the replacement, so re-caching the same asset at the
    // same size never trips it — otherwise a PWA's second install would fail on a full cache.
    if used_bytes(caches) + incoming - existing > QUOTA_BYTES {
        return PutResult::QuotaExceeded;
    }
    let list = caches.get_mut(name).expect("checked above");
    match list
        .iter_mut()
        .find(|e| e.url == entry.url && e.method == entry.method && e.vary == entry.vary)
    {
        Some(slot) => *slot = entry,
        None => list.push(entry),
    }
    s.dirty = true;
    PutResult::Stored
}

/// Remove every entry for a URL (optionally ignoring the query string). Returns whether anything
/// was removed — `cache.delete()`'s resolution value.
pub fn delete_entry(origin: &str, name: &str, url: &str, ignore_search: bool) -> bool {
    let Ok(mut s) = state().lock() else {
        return false;
    };
    ensure_loaded(&mut s);
    let Some(list) = s.origins.get_mut(origin).and_then(|c| c.get_mut(name)) else {
        return false;
    };
    let before = list.len();
    list.retain(|e| !same_url(&e.url, url, ignore_search));
    let removed = list.len() != before;
    if removed {
        s.dirty = true;
    }
    removed
}

/// URL comparison for matching. With `ignoreSearch`, everything from `?` on is dropped from both
/// sides — the spec's own knob, and the one a cache-busting query string requires.
pub fn same_url(a: &str, b: &str, ignore_search: bool) -> bool {
    if !ignore_search {
        return a == b;
    }
    let strip = |u: &str| u.split('?').next().unwrap_or("").to_string();
    strip(a) == strip(b)
}

/// Flush to disk. Called on transaction-shaped boundaries by the shim and once at shutdown, never
/// per entry — a `save()` per `put` re-serialises the whole envelope and turns an `addAll()` of an
/// app shell into a quadratic write.
pub fn save() {
    let Ok(mut s) = state().lock() else {
        return;
    };
    if !s.loaded || !s.dirty {
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
    match serde_json::to_string(&p) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&path, text) {
                tracing::warn!(error = %e, "caches: could not write the store");
            } else {
                s.dirty = false;
            }
        }
        Err(e) => tracing::warn!(error = %e, "caches: could not serialise the store"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(url: &str, body: &str) -> Entry {
        Entry {
            url: url.to_string(),
            method: "GET".into(),
            status: 200,
            status_text: "OK".into(),
            headers: vec![("content-type".into(), "text/plain".into())],
            vary: Vec::new(),
            body: body.to_string(),
            body_b64: false,
        }
    }

    #[test]
    fn a_put_with_the_same_url_replaces_rather_than_appends() {
        let (o, c) = ("https://replace.test", "v1");
        open(o, c);
        assert!(matches!(put(o, c, entry("/a", "one")), PutResult::Stored));
        assert!(matches!(put(o, c, entry("/a", "two")), PutResult::Stored));
        let es = entries(o, c);
        assert_eq!(es.len(), 1, "re-caching one URL must not grow the cache");
        assert_eq!(es[0].body, "two");
    }

    #[test]
    fn entries_that_differ_only_by_vary_coexist() {
        let (o, c) = ("https://vary.test", "v1");
        open(o, c);
        let mut br = entry("/a", "brotli");
        br.vary = vec![("accept-encoding".into(), "br".into())];
        let mut gz = entry("/a", "gzip");
        gz.vary = vec![("accept-encoding".into(), "gzip".into())];
        put(o, c, br);
        put(o, c, gz);
        assert_eq!(
            entries(o, c).len(),
            2,
            "Vary must not collapse two responses"
        );
    }

    #[test]
    fn keys_come_back_in_insertion_order_not_sorted() {
        let (o, c) = ("https://order.test", "v1");
        open(o, c);
        for u in ["/z", "/a", "/m"] {
            put(o, c, entry(u, "x"));
        }
        let urls: Vec<_> = entries(o, c).into_iter().map(|e| e.url).collect();
        assert_eq!(urls, vec!["/z", "/a", "/m"]);
    }

    #[test]
    fn ignore_search_drops_the_query_on_both_sides() {
        assert!(same_url("/a?v=1", "/a?v=2", true));
        assert!(!same_url("/a?v=1", "/a?v=2", false));
        assert!(same_url("/a", "/a", false));
    }

    #[test]
    fn an_opened_but_empty_cache_still_exists() {
        let (o, c) = ("https://empty.test", "v1");
        open(o, c);
        assert!(
            has(o, c),
            "has() asks whether it was opened, not whether it holds anything"
        );
        assert!(entries(o, c).is_empty());
    }

    #[test]
    fn put_into_a_cache_that_was_never_opened_is_refused() {
        assert!(matches!(
            put("https://never.test", "nope", entry("/a", "x")),
            PutResult::NoSuchCache
        ));
    }

    #[test]
    fn deleting_a_cache_reports_whether_it_existed() {
        let (o, c) = ("https://del.test", "v1");
        open(o, c);
        assert!(delete_cache(o, c));
        assert!(
            !delete_cache(o, c),
            "a second delete resolves false, not true"
        );
    }
}
