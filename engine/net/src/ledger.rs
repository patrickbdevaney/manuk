//! W3.13: the record of every non-safe request that leaves the engine, and the write-fence that can
//! hold them. A rollback that only rewinds the client must say what it could not rewind: the server
//! saw these requests and nothing here un-sends them.
//!
//! Every HTTP request reaches the wire through `send_raw` and every WebSocket through
//! `WebSocketConn::connect`; both call [`gate`], so `fetch`/XHR bodies, `sendBeacon`, form submits
//! and streaming requests are all covered by one check (see the enumeration test in `mcp/tests`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use bytes::Bytes;

/// One non-safe request that was sent or held.
#[derive(Clone, Debug)]
pub struct Sent {
    pub method: String,
    pub url: String,
}

impl Sent {
    /// `POST /order`: method + path (+ query), the shape a human reads.
    pub fn short(&self) -> String {
        let path = url::Url::parse(&self.url)
            .map(|u| match u.query() {
                Some(q) => format!("{}?{q}", u.path()),
                None => u.path().to_string(),
            })
            .unwrap_or_else(|_| self.url.clone());
        format!("{} {path}", self.method)
    }
}

/// A request the fence refused to send; replayed by [`take_held`] + the embedder on commit.
#[derive(Clone, Debug)]
pub struct Held {
    pub what: Sent,
    pub headers: Vec<(String, String)>,
    pub body: Bytes,
    /// A WebSocket/WebTransport open cannot be replayed after the fact; commit reports it dropped.
    pub replayable: bool,
}

static FENCE: AtomicBool = AtomicBool::new(false);
static SENT: Mutex<Vec<Sent>> = Mutex::new(Vec::new());
static HELD: Mutex<Vec<Held>> = Mutex::new(Vec::new());

/// GET, HEAD and OPTIONS are safe (RFC 9110 §9.2.1); everything else mutates.
pub fn is_safe(method: &str) -> bool {
    ["GET", "HEAD", "OPTIONS"]
        .iter()
        .any(|m| method.eq_ignore_ascii_case(m))
}

pub fn set_fence(on: bool) {
    FENCE.store(on, Ordering::SeqCst);
}

/// How many non-safe requests have been sent so far; a checkpoint remembers this.
pub fn position() -> usize {
    SENT.lock().map(|s| s.len()).unwrap_or(0)
}

/// The non-safe requests sent since `pos`.
pub fn sent_since(pos: usize) -> Vec<Sent> {
    SENT.lock()
        .map(|s| s.iter().skip(pos).cloned().collect())
        .unwrap_or_default()
}

/// Drain what the fence is holding.
pub fn take_held() -> Vec<Held> {
    HELD.lock()
        .map(|mut h| std::mem::take(&mut *h))
        .unwrap_or_default()
}

/// Called with the request about to be sent. `Ok` = send it (and it is now on the ledger if
/// non-safe); `Err` = the fence is up and the request was held, not sent.
pub fn gate(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &Bytes,
    replayable: bool,
) -> anyhow::Result<()> {
    if is_safe(method) {
        return Ok(());
    }
    let what = Sent {
        method: method.to_ascii_uppercase(),
        url: url.to_string(),
    };
    if FENCE.load(Ordering::SeqCst) {
        let short = what.short();
        if let Ok(mut h) = HELD.lock() {
            h.push(Held {
                what,
                headers: headers
                    .iter()
                    .filter(|(k, _)| !k.eq_ignore_ascii_case("cookie"))
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                body: body.clone(),
                replayable,
            });
        }
        anyhow::bail!("held by the branch write-fence: {short}");
    }
    if let Ok(mut s) = SENT.lock() {
        s.push(what);
    }
    Ok(())
}
