//! Playwright-compatible storage-state import/export (W3.4), plus the per-origin grant that copies
//! one origin's cookies out of the human's profile into an ephemeral session.
//!
//! Shape: `{"cookies":[{name,value,domain,path,expires,httpOnly,secure,sameSite}],
//! "origins":[{"origin":"https://x","localStorage":[{name,value}]}]}`; `expires` is unix seconds,
//! `-1` for a session cookie.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::cookies::SameSite;
use crate::webstorage::{self, Area};

fn jar() -> Result<std::sync::MutexGuard<'static, crate::cookies::CookieJar>> {
    crate::cookie_jar()
        .lock()
        .map_err(|_| anyhow!("cookie jar poisoned"))
}

fn ss_str(s: SameSite) -> &'static str {
    match s {
        SameSite::Strict => "Strict",
        SameSite::Lax => "Lax",
        SameSite::None => "None",
    }
}

fn ss_parse(s: &str) -> SameSite {
    match s.to_ascii_lowercase().as_str() {
        "strict" => SameSite::Strict,
        "none" => SameSite::None,
        _ => SameSite::Lax,
    }
}

/// Snapshot the live cookie jar and localStorage as Playwright storage state.
pub fn export() -> Result<Value> {
    let now = SystemTime::now();
    let cookies: Vec<Value> = jar()?
        .cookies()
        .filter(|c| c.expires.is_none_or(|e| e > now))
        .map(|c| {
            let exp = c
                .expires
                .and_then(|e| e.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(-1.0);
            let domain = if c.host_only {
                c.domain.clone()
            } else {
                format!(".{}", c.domain)
            };
            json!({"name": c.name, "value": c.value, "domain": domain, "path": c.path,
                   "expires": exp, "httpOnly": c.http_only, "secure": c.secure,
                   "sameSite": ss_str(c.same_site)})
        })
        .collect();
    let origins: Vec<Value> = webstorage::origins(Area::Local)
        .into_iter()
        .map(|o| {
            let ls: Vec<Value> = webstorage::keys(Area::Local, &o)
                .into_iter()
                .filter_map(|k| {
                    webstorage::get(Area::Local, &o, &k).map(|v| json!({"name": k, "value": v}))
                })
                .collect();
            json!({"origin": o, "localStorage": ls})
        })
        .collect();
    Ok(json!({"cookies": cookies, "origins": origins}))
}

/// Merge a Playwright storage state into the live session. Returns `(cookies, localStorage keys)`.
pub fn import(state: &Value) -> Result<(usize, usize)> {
    let mut nc = 0;
    let mut nl = 0;
    if let Some(cs) = state.get("cookies").and_then(Value::as_array) {
        let mut j = jar()?;
        for c in cs {
            let s = |k: &str| c.get(k).and_then(Value::as_str);
            let (Some(name), Some(value), Some(domain)) = (s("name"), s("value"), s("domain"))
            else {
                continue;
            };
            let expires = c
                .get("expires")
                .and_then(Value::as_f64)
                .filter(|e| *e > 0.0)
                .map(|e| UNIX_EPOCH + Duration::from_secs_f64(e));
            j.import(
                name,
                value,
                domain,
                s("path").unwrap_or("/"),
                c.get("secure").and_then(Value::as_bool).unwrap_or(false),
                c.get("httpOnly").and_then(Value::as_bool).unwrap_or(false),
                ss_parse(s("sameSite").unwrap_or("Lax")),
                expires,
            );
            nc += 1;
        }
    }
    if let Some(os) = state.get("origins").and_then(Value::as_array) {
        for o in os {
            let Some(origin) = o.get("origin").and_then(Value::as_str) else {
                continue;
            };
            for kv in o
                .get("localStorage")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let (Some(k), Some(v)) = (
                    kv.get("name").and_then(Value::as_str),
                    kv.get("value").and_then(Value::as_str),
                ) {
                    if webstorage::set(Area::Local, origin, k, v) {
                        nl += 1;
                    }
                }
            }
        }
    }
    Ok((nc, nl))
}

/// Per-origin grant: copy the cookies the human's profile (`cookies.json` beside `profile_state`)
/// holds for `origin`'s host into this session. Returns how many were copied. The caller logs it.
pub fn grant_from_profile(profile_cookies: &std::path::Path, origin: &str) -> Result<usize> {
    let host = url::Url::parse(origin)?
        .host_str()
        .ok_or_else(|| anyhow!("grant origin has no host: {origin}"))?
        .to_ascii_lowercase();
    let src = crate::cookies::CookieJar::load_from(profile_cookies);
    let mut j = jar()?;
    let mut n = 0;
    for c in src.cookies() {
        let applies =
            host == c.domain || (!c.host_only && host.ends_with(&format!(".{}", c.domain)));
        if applies {
            let domain = if c.host_only {
                c.domain.clone()
            } else {
                format!(".{}", c.domain)
            };
            j.import(
                &c.name,
                &c.value,
                &domain,
                &c.path,
                c.secure,
                c.http_only,
                c.same_site,
                c.expires,
            );
            n += 1;
        }
    }
    Ok(n)
}
