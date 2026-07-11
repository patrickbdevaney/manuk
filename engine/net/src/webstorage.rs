//! **Web Storage** (`localStorage` / `sessionStorage`), partitioned by origin.
//!
//! This is not a nice-to-have. The modern web *feature-detects* it, and a browser without it is
//! not merely missing a feature — it is **graded down**. MediaWiki's startup script runs
//!
//! ```js
//! function isCompatible(){ return !!('querySelector' in document && 'localStorage' in window && ...) }
//! if (!isCompatible()) { /* revert `client-js` → `client-nojs`, ship the degraded page */ }
//! ```
//!
//! so on every MediaWiki site in the world — Wikipedia included — the absence of `localStorage`
//! silently switched the whole UI to its no-script fallback: the table of contents never collapsed,
//! and every element on the page below it landed thousands of pixels out of place. It looked like a
//! layout bug for an hour. It was a missing BOM object.
//!
//! **Durability (ADR-009).** `localStorage` is user data: it must outlive the binary. It is written
//! to the profile beside the cookie jar, in a versioned envelope so a future format change can
//! migrate rather than silently discard someone's data. `sessionStorage` is per-session by
//! definition and is never persisted.
//!
//! **Origin partitioning.** Keys are namespaced by origin (`scheme://host[:port]`), so one site can
//! never read another's storage — the same boundary the cookie jar enforces.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use url::Url;

/// Which storage area an operation targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    /// Persisted across sessions.
    Local,
    /// Lives only as long as the browsing session.
    Session,
}

impl Area {
    /// Parse the area name used at the JS boundary.
    pub fn parse(s: &str) -> Option<Area> {
        match s {
            "local" => Some(Area::Local),
            "session" => Some(Area::Session),
            _ => None,
        }
    }
}

/// The on-disk envelope. `version` exists so a later format can be migrated instead of dropped —
/// a browser that loses a user's data on upgrade has broken its contract with them (ADR-009).
#[derive(Default, Serialize, Deserialize)]
struct Persisted {
    version: u32,
    /// origin → (key → value)
    origins: BTreeMap<String, BTreeMap<String, String>>,
}

const FORMAT_VERSION: u32 = 1;

/// Per-origin quota. Real browsers enforce ~5 MiB; an unbounded map is a memory-exhaustion vector
/// for a hostile page, so the limit is real, not decorative.
const QUOTA_BYTES: usize = 5 * 1024 * 1024;

#[derive(Default)]
struct Areas {
    local: BTreeMap<String, BTreeMap<String, String>>,
    session: BTreeMap<String, BTreeMap<String, String>>,
    loaded: bool,
    dirty: bool,
}

fn state() -> &'static Mutex<Areas> {
    static S: std::sync::OnceLock<Mutex<Areas>> = std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}

/// Where `localStorage` lives — beside the cookie jar, in the same profile directory.
pub fn store_path() -> PathBuf {
    crate::cookie_store_path().with_file_name("localstorage.json")
}

/// `scheme://host[:port]` — the storage partition key.
pub fn origin_of(url: &str) -> Option<String> {
    let u = Url::parse(url).ok()?;
    let host = u.host_str()?;
    Some(match u.port() {
        Some(p) => format!("{}://{}:{}", u.scheme(), host, p),
        None => format!("{}://{}", u.scheme(), host),
    })
}

fn ensure_loaded(a: &mut Areas) {
    if a.loaded {
        return;
    }
    a.loaded = true;
    let Ok(text) = std::fs::read_to_string(store_path()) else {
        return;
    };
    match serde_json::from_str::<Persisted>(&text) {
        Ok(p) if p.version == FORMAT_VERSION => a.local = p.origins,
        Ok(p) => {
            // A version we do not understand. Do NOT delete it: keep the file, start empty, and
            // say so. Silently discarding a user's data is the failure mode this envelope exists
            // to prevent.
            tracing::warn!(
                found = p.version,
                expected = FORMAT_VERSION,
                "localStorage: unknown format version; leaving the file untouched"
            );
        }
        Err(e) => tracing::warn!(error = %e, "localStorage: could not parse; starting empty"),
    }
}

/// Read one key. `None` is JS `null` (the key is absent).
pub fn get(area: Area, origin: &str, key: &str) -> Option<String> {
    let mut a = state().lock().ok()?;
    ensure_loaded(&mut a);
    let map = match area {
        Area::Local => &a.local,
        Area::Session => &a.session,
    };
    map.get(origin)?.get(key).cloned()
}

/// Write one key. Returns `false` if the origin's quota would be exceeded (JS throws
/// `QuotaExceededError` on `false`).
pub fn set(area: Area, origin: &str, key: &str, value: &str) -> bool {
    let Ok(mut a) = state().lock() else {
        return false;
    };
    ensure_loaded(&mut a);
    let persist = area == Area::Local;
    let map = match area {
        Area::Local => &mut a.local,
        Area::Session => &mut a.session,
    };
    let entry = map.entry(origin.to_string()).or_default();
    let used: usize = entry
        .iter()
        .filter(|(k, _)| k.as_str() != key)
        .map(|(k, v)| k.len() + v.len())
        .sum();
    if used + key.len() + value.len() > QUOTA_BYTES {
        return false;
    }
    entry.insert(key.to_string(), value.to_string());
    if persist {
        a.dirty = true;
    }
    true
}

/// Delete one key.
pub fn remove(area: Area, origin: &str, key: &str) {
    let Ok(mut a) = state().lock() else {
        return;
    };
    ensure_loaded(&mut a);
    let persist = area == Area::Local;
    let map = match area {
        Area::Local => &mut a.local,
        Area::Session => &mut a.session,
    };
    if let Some(e) = map.get_mut(origin) {
        e.remove(key);
    }
    if persist {
        a.dirty = true;
    }
}

/// Drop every key for this origin.
pub fn clear(area: Area, origin: &str) {
    let Ok(mut a) = state().lock() else {
        return;
    };
    ensure_loaded(&mut a);
    let persist = area == Area::Local;
    let map = match area {
        Area::Local => &mut a.local,
        Area::Session => &mut a.session,
    };
    map.remove(origin);
    if persist {
        a.dirty = true;
    }
}

/// The origin's keys, in insertion-independent (sorted) order — enough for `length`, `key(i)` and
/// `Object.keys(localStorage)`.
pub fn keys(area: Area, origin: &str) -> Vec<String> {
    let Ok(mut a) = state().lock() else {
        return Vec::new();
    };
    ensure_loaded(&mut a);
    let map = match area {
        Area::Local => &a.local,
        Area::Session => &a.session,
    };
    map.get(origin)
        .map(|e| e.keys().cloned().collect())
        .unwrap_or_default()
}

/// Flush `localStorage` to the profile. Call on navigation-commit and on quit — the same points the
/// cookie jar is flushed. Best-effort; a write failure is logged, not fatal.
pub fn save() {
    let Ok(mut a) = state().lock() else {
        return;
    };
    if !a.dirty {
        return;
    }
    let p = Persisted {
        version: FORMAT_VERSION,
        origins: a.local.clone(),
    };
    let path = store_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match serde_json::to_string(&p).map(|t| std::fs::write(&path, t)) {
        Ok(Ok(())) => a.dirty = false,
        Ok(Err(e)) => tracing::warn!(error = %e, "localStorage: write failed"),
        Err(e) => tracing::warn!(error = %e, "localStorage: serialize failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_partitions_by_scheme_host_port() {
        assert_eq!(origin_of("https://a.example/x?y=1").unwrap(), "https://a.example");
        assert_eq!(origin_of("http://a.example:8080/").unwrap(), "http://a.example:8080");
        // Different scheme, different origin — one cannot read the other.
        assert_ne!(origin_of("https://a.example/").unwrap(), origin_of("http://a.example/").unwrap());
    }

    #[test]
    fn session_storage_round_trips_and_is_origin_scoped() {
        let o1 = "https://one.test";
        let o2 = "https://two.test";
        set(Area::Session, o1, "k", "v1");
        set(Area::Session, o2, "k", "v2");
        assert_eq!(get(Area::Session, o1, "k").as_deref(), Some("v1"));
        assert_eq!(get(Area::Session, o2, "k").as_deref(), Some("v2"));
        assert_eq!(keys(Area::Session, o1), vec!["k".to_string()]);
        remove(Area::Session, o1, "k");
        assert_eq!(get(Area::Session, o1, "k"), None);
        // The other origin is untouched.
        assert_eq!(get(Area::Session, o2, "k").as_deref(), Some("v2"));
        clear(Area::Session, o2);
        assert!(keys(Area::Session, o2).is_empty());
    }

    #[test]
    fn quota_is_enforced() {
        let o = "https://quota.test";
        let big = "x".repeat(QUOTA_BYTES);
        assert!(!set(Area::Session, o, "k", &big), "over-quota write must be refused");
        assert!(set(Area::Session, o, "k", "small"));
    }
}
