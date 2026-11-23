//! R3.1.a — network record / replay for `--deterministic`.
//!
//! **Record** (`MANUK_RECORD=<dir>`): every response the net layer hands to a caller (document,
//! subresource, bodied `fetch`/XHR, POST navigation, image-header probe) is appended to a recording
//! directory: `index.jsonl` (one JSON line per response: method, url, request-body hash, status,
//! headers, final url) plus `bodies/<sha256>` (decoded body, content-addressed).
//! **Replay** (`MANUK_REPLAY=<dir>`): the same calls are answered from the recording and the
//! network is never touched; a request the recording does not hold fails loudly (and is counted in
//! [`misses`]) rather than falling through to the wire. A live site thereby becomes a fixture.
//!
//! Secrets (W3.16): every header value and every UTF-8 body passes through `secrets::redact`, and
//! `Set-Cookie` values are reduced to `name=•••` — a recording never holds a bound secret or a
//! session cookie. Request bodies are stored only as a hash (key), never in the clear.
use crate::{HttpVersion, Response};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    method: String,
    url: String,
    req: String,
    status: u16,
    headers: Vec<(String, String)>,
    final_url: String,
    body: String,
}

enum Mode {
    Off,
    Record(PathBuf),
    Replay(PathBuf),
}

static MODE: OnceLock<Mutex<Mode>> = OnceLock::new();
static TABLE: OnceLock<Mutex<HashMap<String, Vec<Entry>>>> = OnceLock::new();
static CURSOR: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();
static MISSES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

fn mode() -> &'static Mutex<Mode> {
    MODE.get_or_init(|| {
        let m = if let Some(d) = std::env::var_os("MANUK_REPLAY").filter(|d| !d.is_empty()) {
            Mode::Replay(d.into())
        } else if let Some(d) = std::env::var_os("MANUK_RECORD").filter(|d| !d.is_empty()) {
            Mode::Record(d.into())
        } else {
            Mode::Off
        };
        if let Mode::Replay(d) = &m {
            load(d);
        }
        Mutex::new(m)
    })
}

fn sha(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

fn key(method: &str, url: &str, req: &str) -> String {
    format!("{method} {url} {req}")
}

fn load(dir: &std::path::Path) {
    let mut t: HashMap<String, Vec<Entry>> = HashMap::new();
    if let Ok(s) = std::fs::read_to_string(dir.join("index.jsonl")) {
        for l in s.lines() {
            if let Ok(e) = serde_json::from_str::<Entry>(l) {
                t.entry(key(&e.method, &e.url, &e.req)).or_default().push(e);
            }
        }
    }
    *TABLE.get_or_init(Default::default).lock().unwrap() = t;
}

/// Start recording into `dir` (programmatic form of `MANUK_RECORD`).
pub fn record_to(dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir.join("bodies"));
    *mode().lock().unwrap() = Mode::Record(dir.to_path_buf());
}

/// Replay from `dir` with no network (programmatic form of `MANUK_REPLAY`).
pub fn replay_from(dir: &std::path::Path) {
    load(dir);
    CURSOR.get_or_init(Default::default).lock().unwrap().clear();
    *mode().lock().unwrap() = Mode::Replay(dir.to_path_buf());
}

pub fn is_replay() -> bool {
    matches!(*mode().lock().unwrap(), Mode::Replay(_))
}

pub fn is_record() -> bool {
    matches!(*mode().lock().unwrap(), Mode::Record(_))
}

/// URLs asked for during replay that the recording did not hold.
pub fn misses() -> Vec<String> {
    MISSES.get_or_init(Default::default).lock().unwrap().clone()
}

fn scrub_header(k: &str, v: &str) -> String {
    if k.eq_ignore_ascii_case("set-cookie") {
        let name = v.split('=').next().unwrap_or("");
        return format!("{name}=•••");
    }
    crate::secrets::redact(v)
}

/// Record `r` as the answer to `(method, url, req_body)`. No-op unless recording.
pub fn record(method: &str, url: &str, req_body: &[u8], r: &Response) {
    let Mode::Record(dir) = &*mode().lock().unwrap() else {
        return;
    };
    let _ = std::fs::create_dir_all(dir.join("bodies"));
    let body: Vec<u8> = match std::str::from_utf8(&r.body) {
        Ok(t) => {
            let red = crate::secrets::redact(t);
            if red == t {
                r.body.to_vec()
            } else {
                red.into_bytes()
            }
        }
        Err(_) => r.body.to_vec(),
    };
    let h = sha(&body);
    let _ = std::fs::write(dir.join("bodies").join(&h), &body);
    let e = Entry {
        method: method.to_string(),
        url: url.to_string(),
        req: if req_body.is_empty() {
            String::new()
        } else {
            sha(req_body)
        },
        status: r.status,
        headers: r
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), scrub_header(k, v)))
            .collect(),
        final_url: r.final_url.to_string(),
        body: h,
    };
    if let Ok(line) = serde_json::to_string(&e) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("index.jsonl"))
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// `None` when not replaying (caller proceeds to the network). `Some(Err)` is a replay miss: the
/// request is refused, never sent. `Some(Ok)` is the recorded response. Repeated identical requests
/// get the recorded answers in order, then the last one again.
pub fn replay(method: &str, url: &str, req_body: &[u8]) -> Option<anyhow::Result<Response>> {
    let dir = match &*mode().lock().unwrap() {
        Mode::Replay(d) => d.clone(),
        _ => return None,
    };
    let req = if req_body.is_empty() {
        String::new()
    } else {
        sha(req_body)
    };
    let k = key(method, url, &req);
    let e = {
        let t = TABLE.get_or_init(Default::default).lock().unwrap();
        let mut c = CURSOR.get_or_init(Default::default).lock().unwrap();
        t.get(&k).map(|v| {
            let i = c.entry(k.clone()).or_insert(0);
            let e = v[(*i).min(v.len() - 1)].clone();
            *i += 1;
            e
        })
    };
    let Some(e) = e else {
        MISSES
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .push(format!("{method} {url}"));
        return Some(Err(anyhow::anyhow!(
            "replay miss (network blocked): {method} {url}"
        )));
    };
    let body = std::fs::read(dir.join("bodies").join(&e.body)).unwrap_or_default();
    Some(Ok(Response {
        status: e.status,
        headers: e.headers,
        body: Bytes::from(body),
        final_url: url::Url::parse(&e.final_url).ok()?,
        http_version: HttpVersion::Http11,
    }))
}
