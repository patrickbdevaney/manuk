//! W3.13: a snapshot of the client side of the world: cookies, localStorage, sessionStorage and
//! CacheStorage. Service-worker registrations are part of the contract but the engine has no
//! service-worker store yet, so that list is always empty and says so. None of this reaches the
//! server: see [`crate::ledger`] for what a restore cannot undo.

use serde_json::{json, Value};

use crate::webstorage::{self, Area};
use crate::{cachestorage, storage_state};

pub fn snapshot() -> anyhow::Result<Value> {
    let session: Vec<Value> = webstorage::origins(Area::Session)
        .into_iter()
        .map(|o| {
            let kv: Vec<Value> = webstorage::keys(Area::Session, &o)
                .into_iter()
                .filter_map(|k| {
                    webstorage::get(Area::Session, &o, &k).map(|v| json!({"name": k, "value": v}))
                })
                .collect();
            json!({"origin": o, "sessionStorage": kv})
        })
        .collect();
    Ok(json!({
        "state": storage_state::export()?,
        "session": session,
        "caches": cachestorage::dump(),
        "service_workers": [],
    }))
}

/// Make the partition equal `snap`: everything not in it is dropped first.
pub fn restore(snap: &Value) -> anyhow::Result<()> {
    if let Ok(mut j) = crate::cookie_jar().lock() {
        j.clear();
    }
    webstorage::reset_in_memory();
    if let Some(st) = snap.get("state") {
        storage_state::import(st)?;
    }
    for o in snap["session"].as_array().into_iter().flatten() {
        let Some(origin) = o["origin"].as_str() else {
            continue;
        };
        for kv in o["sessionStorage"].as_array().into_iter().flatten() {
            if let (Some(k), Some(v)) = (kv["name"].as_str(), kv["value"].as_str()) {
                webstorage::set(Area::Session, origin, k, v);
            }
        }
    }
    cachestorage::load_dump(&snap["caches"]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_returns_cookies_session_storage_and_caches_to_the_snapshot() {
        crate::set_ephemeral(true);
        let url = url::Url::parse("https://shop.test/").unwrap();
        crate::cookie_jar()
            .lock()
            .unwrap()
            .store(&url, "sid=one; Path=/");
        webstorage::set(Area::Session, "https://shop.test", "k", "one");
        cachestorage::open("https://shop.test", "shell");
        let snap = snapshot().unwrap();

        // the world moves on: new cookie value, new session key, a second cache
        crate::cookie_jar()
            .lock()
            .unwrap()
            .store(&url, "sid=two; Path=/");
        crate::cookie_jar()
            .lock()
            .unwrap()
            .store(&url, "extra=1; Path=/");
        webstorage::set(Area::Session, "https://shop.test", "k", "two");
        cachestorage::open("https://shop.test", "late");
        assert_eq!(cachestorage::cache_names("https://shop.test").len(), 2);

        restore(&snap).unwrap();
        let h = crate::cookie_jar()
            .lock()
            .unwrap()
            .cookie_header(&url)
            .unwrap();
        assert_eq!(h, "sid=one", "cookies not rewound");
        assert_eq!(
            webstorage::get(Area::Session, "https://shop.test", "k").as_deref(),
            Some("one")
        );
        assert_eq!(
            cachestorage::cache_names("https://shop.test"),
            vec!["shell".to_string()]
        );
        assert_eq!(snap["service_workers"], serde_json::json!([]));
    }
}
