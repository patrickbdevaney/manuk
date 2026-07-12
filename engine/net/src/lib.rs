//! manuk-net — the networking layer.
//!
//! Per CLAUDE.md we *reuse* the mature Rust networking stack: `tokio`, `hyper`,
//! `rustls` (pure-Rust TLS, no OpenSSL), with `webpki-roots` trust anchors.
//!
//! **P0.4 net redesign:** a process-global pooled `hyper_util::client::legacy::Client`
//! over a `hyper-rustls::HttpsConnector` (ALPN `h2,http/1.1`) — one stack that gives
//! connection pooling + Happy Eyeballs (B2), automatic HTTP/2 (B3), and a streamable
//! response body (B1). `Content-Encoding` (gzip/deflate/br) is decoded via
//! `async-compression` over the body stream.
//!
//! [`fetch`] is GET-with-redirects; [`request`] is a general single request (any
//! method/headers/body) used by API clients such as the agent's Groq backend.

use std::sync::OnceLock;

use anyhow::{bail, Context, Result};
/// `pub` so callers (the shell's page-fetch pump) can build request bodies without a direct
/// `bytes` dependency.
pub use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::{BodyStream, Full};
use hyper::body::Incoming;
use hyper::header::{
    ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CONTENT_ENCODING, LOCATION, USER_AGENT,
};
use hyper::Request;
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;
use url::Url;

/// E7 storage layer — RFC 6265 cookie jar.
pub mod webstorage;
pub mod cookies;

pub mod downloads;

pub mod multipart;

/// E7 storage layer — profile/container/site-partitioned cookies, localStorage, history.
pub mod storage;

/// E7 — SOCKS5 proxying (user-provided proxy; no bundled VPN).
pub mod proxy;

/// E5 native content-blocking (feature `adblock`).
#[cfg(feature = "adblock")]
pub mod blocker;

/// **Honest `User-Agent`** (F1) — truthful, never competitor mimicry (CLAUDE.md Axis
/// F). Form: `Mozilla/5.0 (<real-os>; <real-arch>) Manuk/<ver> (+standards)`. The
/// `Mozilla/5.0` prefix is the *universal* compatibility token (every engine + many
/// bots send it; it names no specific competitor); the OS/arch are the machine's
/// **real** values (`std::env::consts`), and the product token names Manuk + its real
/// version. **No** Chrome/Safari/Firefox spoofing, header-order copying, or JA3/JA4
/// mimicry — see the module policy + the `user_agent_is_honest` guard test.
pub fn user_agent() -> &'static str {
    static UA: OnceLock<String> = OnceLock::new();
    UA.get_or_init(|| {
        let os = match std::env::consts::OS {
            "linux" => "X11; Linux",
            "macos" => "Macintosh; macOS",
            "windows" => "Windows NT",
            other => other,
        };
        format!(
            "Mozilla/5.0 ({}; {}) Manuk/{} (+standards)",
            os,
            std::env::consts::ARCH,
            env!("CARGO_PKG_VERSION")
        )
    })
}

/// `Accept-Language` default (English; a real preference, not a fingerprint knob).
const ACCEPT_LANGUAGE_STR: &str = "en-US,en;q=0.9";

/// Maximum number of 3xx redirects to follow before giving up.
const MAX_REDIRECTS: usize = 10;

type NetClient = Client<HttpsConnector<HttpConnector>, Full<Bytes>>;

/// The process-global pooled client. Cheap to clone; reuses idle connections
/// (default 90s keep-alive), so sequential same-origin fetches skip the TLS
/// handshake.
/// The process-global HTTPS connector, shared by the pooled [`client`] and the
/// [`Preconnector`]. Cloning it shares the same `Arc<rustls::ClientConfig>` — so a
/// preconnect's TLS handshake populates the **same session cache** the real
/// navigation resumes from (a warm preconnect saves the TLS round-trips even though
/// the raw socket itself is not adopted by the pool).
fn connector() -> HttpsConnector<HttpConnector> {
    static CONN: OnceLock<HttpsConnector<HttpConnector>> = OnceLock::new();
    CONN.get_or_init(|| {
        // Install a rustls crypto provider once (idempotent).
        let _ = rustls::crypto::ring::default_provider().install_default();
        hyper_rustls::HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build()
    })
    .clone()
}

/// **THE async runtime. Singular. (METHODOLOGY Part 25.1.)**
///
/// There should be exactly one Tokio runtime, one Rayon pool, for the life of the process — created
/// once at startup and reused. The shell was building **two** multi-threaded runtimes: one in `main`
/// and one in `App`. On a 32-thread machine that is 64 worker threads, two schedulers and ~128MB of
/// stacks, for a browser that needs one of each.
///
/// The canonical failure this guards against is worse and is the reason for the gate rather than a
/// one-off fix: a runtime created *per navigation* or *per search*. That is invisible at idle,
/// invisible in a profile of a single action, and lethal after an hour of browsing — precisely the
/// shape of the wheel-event clone regression, one layer up the stack. `G_RUNTIME_COUNT` asserts the
/// instantiation count stays FLAT across a scripted session, so "one runtime" is a measured fact and
/// not an architectural intention.
pub static RUNTIME_INSTANTIATIONS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        RUNTIME_INSTANTIATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("the process-wide async runtime")
    })
}

fn client() -> &'static NetClient {
    static CLIENT: OnceLock<NetClient> = OnceLock::new();
    CLIENT.get_or_init(|| Client::builder(TokioExecutor::new()).build(connector()))
}

/// The negotiated HTTP version of a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpVersion {
    Http09,
    Http10,
    Http11,
    Http2,
    Http3,
    Other,
}

impl From<hyper::Version> for HttpVersion {
    fn from(v: hyper::Version) -> Self {
        match v {
            hyper::Version::HTTP_09 => HttpVersion::Http09,
            hyper::Version::HTTP_10 => HttpVersion::Http10,
            hyper::Version::HTTP_11 => HttpVersion::Http11,
            hyper::Version::HTTP_2 => HttpVersion::Http2,
            hyper::Version::HTTP_3 => HttpVersion::Http3,
            _ => HttpVersion::Other,
        }
    }
}

/// A fetched HTTP response. `body` is already `Content-Encoding`-decoded.
#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Bytes,
    /// The URL the body came from (after any redirects).
    pub final_url: Url,
    pub http_version: HttpVersion,
}

impl Response {
    /// Value of a response header, case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Body decoded as UTF-8 (lossy), for JSON/text where the charset is known-UTF-8.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Body decoded to a `String` using the **WHATWG charset sniff** (D4):
    /// BOM → HTTP `Content-Type` charset → `<meta>` prescan (first 1024 bytes) →
    /// `chardetng` detector → UTF-8 default. Use this for HTML documents.
    pub fn decoded_text(&self) -> String {
        let ct = self.header("content-type");
        charset::decode_html(&self.body, ct)
    }
}

/// Charset detection + decoding per the WHATWG Encoding Standard (reuses
/// `encoding_rs` for decode and `chardetng` for the legacy fallback detector).
pub mod charset {
    use encoding_rs::{Encoding, UTF_8};

    /// Decode HTML `bytes` to a `String` following the WHATWG sniff order.
    pub fn decode_html(bytes: &[u8], content_type: Option<&str>) -> String {
        let enc = sniff(bytes, content_type);
        let (text, _, _) = enc.decode(bytes);
        text.into_owned()
    }

    /// Pick the encoding: BOM → Content-Type charset → `<meta>` prescan → detector.
    pub fn sniff(bytes: &[u8], content_type: Option<&str>) -> &'static Encoding {
        // 1. BOM.
        if let Some((enc, _)) = Encoding::for_bom(bytes) {
            return enc;
        }
        // 2. HTTP Content-Type charset.
        if let Some(label) = content_type.and_then(charset_from_content_type) {
            if let Some(enc) = Encoding::for_label(label.as_bytes()) {
                return enc;
            }
        }
        // 3. <meta> prescan of the first 1024 bytes.
        if let Some(enc) = meta_prescan(&bytes[..bytes.len().min(1024)]) {
            return enc;
        }
        // 4. chardetng detector fallback.
        let mut det = chardetng::EncodingDetector::new();
        det.feed(bytes, true);
        let guess = det.guess(None, true);
        if guess != UTF_8 {
            return guess;
        }
        // 5. Default.
        UTF_8
    }

    fn charset_from_content_type(ct: &str) -> Option<String> {
        ct.split(';').find_map(|part| {
            let part = part.trim();
            let rest = part.strip_prefix("charset=").or_else(|| {
                part.to_ascii_lowercase()
                    .starts_with("charset=")
                    .then(|| &part[8..])
            })?;
            Some(rest.trim().trim_matches('"').to_string())
        })
    }

    /// Minimal `<meta charset>` / `<meta http-equiv=content-type>` prescan.
    fn meta_prescan(head: &[u8]) -> Option<&'static Encoding> {
        let text = String::from_utf8_lossy(head).to_ascii_lowercase();
        let mut search = text.as_str();
        while let Some(pos) = search.find("<meta") {
            let tag_end = search[pos..]
                .find('>')
                .map(|e| pos + e)
                .unwrap_or(search.len());
            let tag = &search[pos..tag_end];
            // <meta charset="...">
            if let Some(cs) = attr_value(tag, "charset") {
                if let Some(enc) = Encoding::for_label(cs.as_bytes()) {
                    return Some(enc);
                }
            }
            // <meta http-equiv="content-type" content="...; charset=...">
            if tag.contains("http-equiv") {
                if let Some(content) = attr_value(tag, "content") {
                    if let Some(label) = charset_from_content_type(&content) {
                        if let Some(enc) = Encoding::for_label(label.as_bytes()) {
                            return Some(enc);
                        }
                    }
                }
            }
            search = &search[tag_end.min(search.len())..];
        }
        None
    }

    fn attr_value(tag: &str, attr: &str) -> Option<String> {
        let idx = tag.find(&format!("{attr}="))? + attr.len() + 1;
        let rest = tag[idx..].trim_start();
        let val = if let Some(q) = rest.strip_prefix('"') {
            q.split('"').next()?
        } else if let Some(q) = rest.strip_prefix('\'') {
            q.split('\'').next()?
        } else {
            rest.split([' ', '/', '>']).next()?
        };
        Some(val.trim().to_string())
    }
}

/// A minimal in-memory HTTP cache (a subset of RFC 9111): fresh `GET` `200` responses
/// with an explicit freshness lifetime (`Cache-Control: max-age` / `s-maxage`) are stored
/// and served without a network round-trip until they go stale. Deliberately omitted for
/// now (documented, not faked): `Vary`, conditional revalidation (`ETag`/`If-None-Match`),
/// heuristic freshness, and disk persistence.
mod http_cache {
    use super::Response;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    struct Entry {
        response: Response,
        stored: Instant,
        fresh_for: Duration,
    }

    fn store() -> &'static Mutex<HashMap<String, Entry>> {
        static S: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
        S.get_or_init(|| Mutex::new(HashMap::new()))
    }

    /// A still-fresh cached response for `url`, if any.
    pub fn get(url: &str) -> Option<Response> {
        let map = store().lock().ok()?;
        let e = map.get(url)?;
        (e.stored.elapsed() < e.fresh_for).then(|| e.response.clone())
    }

    /// Cache `response` for `url` if it is cacheable: `GET`-implied `200` with a positive
    /// `max-age` and no `no-store`/`private`.
    pub fn put(url: &str, response: &Response) {
        if response.status != 200 {
            return;
        }
        let cc = response.header("cache-control").unwrap_or("").to_ascii_lowercase();
        if cc.contains("no-store") || cc.contains("private") || cc.contains("no-cache") {
            return;
        }
        let Some(secs) = max_age(&cc) else { return };
        if secs == 0 {
            return;
        }
        if let Ok(mut map) = store().lock() {
            map.insert(
                url.to_string(),
                Entry { response: response.clone(), stored: Instant::now(), fresh_for: Duration::from_secs(secs) },
            );
        }
    }

    /// Parse `max-age`/`s-maxage` (seconds) from a lowercased `Cache-Control` value.
    fn max_age(cc: &str) -> Option<u64> {
        for directive in ["s-maxage=", "max-age="] {
            if let Some(i) = cc.find(directive) {
                let rest = &cc[i + directive.len()..];
                let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(v) = n.parse::<u64>() {
                    return Some(v);
                }
            }
        }
        None
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::HttpVersion;
        use bytes::Bytes;
        use url::Url;

        fn resp(cc: &str) -> Response {
            Response {
                status: 200,
                headers: vec![("cache-control".into(), cc.into())],
                body: Bytes::from_static(b"x"),
                final_url: Url::parse("https://e.test/a.css").unwrap(),
                http_version: HttpVersion::Http11,
            }
        }

        #[test]
        fn caches_fresh_and_skips_uncacheable() {
            let u = "https://e.test/max-age";
            put(u, &resp("max-age=300"));
            assert!(get(u).is_some(), "fresh max-age response is served from cache");

            put("https://e.test/nostore", &resp("no-store, max-age=300"));
            assert!(get("https://e.test/nostore").is_none(), "no-store is not cached");

            put("https://e.test/zero", &resp("max-age=0"));
            assert!(get("https://e.test/zero").is_none(), "max-age=0 is not cached");
        }
    }
}

/// Fetch `url` with GET, following redirects. `url` must be an absolute
/// `http`/`https` URL. A still-fresh in-memory cache entry (see [`http_cache`]) short-
/// circuits the network round-trip.
/// **Every request has a deadline.** There was none — not a connect timeout, not a read timeout,
/// nothing — and the consequence is not subtle: one subresource that completes its TCP handshake and
/// then never answers stalls the `join_all` that fetches the page's stylesheets or images until the
/// *kernel* gives up, which is minutes. The tab is frozen for the whole of it.
///
/// This is not an exotic failure. It is the ordinary condition of the real web: ad hosts, trackers,
/// analytics beacons and geoblocked CDNs blackhole connections constantly, and a browser that waits
/// for them is a browser that cannot open the pages people actually visit. Measured on
/// w3schools.com: **37.8s** for us against Chromium's 12.5s on the identical page, with the whole
/// difference sitting in subresource fetches nobody was ever going to get an answer from.
///
/// A browser's contract is that the page renders. A subresource is an *enhancement* — if it does not
/// arrive in time, the page renders without it, exactly as Chromium does. It is never allowed to
/// hold the document hostage.
///
/// `MANUK_NET_TIMEOUT_MS` overrides; the default is deliberately well under human patience.
pub fn request_timeout() -> std::time::Duration {
    static T: OnceLock<std::time::Duration> = OnceLock::new();
    *T.get_or_init(|| {
        let ms = std::env::var("MANUK_NET_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(8_000);
        std::time::Duration::from_millis(ms)
    })
}

/// **Part 22.3: no URL is fetched twice for one navigation.** A duplicate fetch is both a
/// performance bug and a correctness risk (two copies of a resource can disagree). Counted here so
/// the claim is a measurement rather than an assertion.
pub static FETCHES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub static FETCH_DUPES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static SEEN: OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = OnceLock::new();

pub fn fetch_stats() -> (usize, usize) {
    (
        FETCHES.load(std::sync::atomic::Ordering::Relaxed),
        FETCH_DUPES.load(std::sync::atomic::Ordering::Relaxed),
    )
}
pub fn reset_fetch_stats() {
    FETCHES.store(0, std::sync::atomic::Ordering::Relaxed);
    FETCH_DUPES.store(0, std::sync::atomic::Ordering::Relaxed);
    if let Some(m) = SEEN.get() {
        m.lock().unwrap().clear();
    }
}

pub async fn fetch(url: &str) -> Result<Response> {
    FETCHES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    {
        let seen = SEEN.get_or_init(Default::default);
        let mut g = seen.lock().unwrap();
        if !g.insert(url.to_string()) {
            FETCH_DUPES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            tracing::debug!(%url, "DUPLICATE FETCH for one navigation");
        }
    }
    fetch_with_deadline(url, request_timeout()).await
}

/// **The document is not an enhancement, and must not share the enhancement's deadline.**
///
/// The subresource timeout exists so a dead tracker cannot hold the page hostage. Applying the same
/// 8s to the *main document* inverts that: a slow-but-alive server — a big page on a bad link, an
/// origin behind a cold cache — would now fail to open at all, and we would have traded "some sites
/// hang" for "some sites are unreachable", which is not a trade, it is a different bug.
///
/// So the document gets a human-patience deadline and the subresources get a machine one. Nothing
/// is unbounded either way; that was the actual defect.
pub async fn fetch_document(url: &str) -> Result<Response> {
    let d = std::env::var("MANUK_DOC_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(std::time::Duration::from_millis)
        .unwrap_or(std::time::Duration::from_secs(30));
    fetch_with_deadline(url, d).await
}

async fn fetch_with_deadline(url: &str, d: std::time::Duration) -> Result<Response> {
    match tokio::time::timeout(d, fetch_inner(url)).await {
        Ok(r) => r,
        Err(_) => {
            let secs = d.as_secs_f32();
            tracing::warn!(%url, "timed out after {secs:.1}s");
            bail!("timed out after {secs:.1}s: {url}")
        }
    }
}

async fn fetch_inner(url: &str) -> Result<Response> {
    if let Some(cached) = http_cache::get(url) {
        tracing::debug!(%url, "served from HTTP cache");
        return Ok(cached);
    }
    let mut current = Url::parse(url).with_context(|| format!("invalid URL: {url}"))?;
    for _ in 0..=MAX_REDIRECTS {
        let resp = send_once("GET", &current, &[], Bytes::new()).await?;
        if (300..400).contains(&resp.status) {
            if let Some(loc) = resp.header("location") {
                let next = current
                    .join(loc)
                    .with_context(|| format!("bad redirect target: {loc}"))?;
                tracing::debug!(%current, %next, status = resp.status, "following redirect");
                current = next;
                continue;
            }
        }
        http_cache::put(url, &resp);
        return Ok(resp);
    }
    bail!("too many redirects (>{MAX_REDIRECTS}) starting at {url}")
}

/// Metadata for a [`fetch_streaming`] response (everything but the body, which was
/// delivered chunk-by-chunk to the sink).
#[derive(Clone, Debug)]
pub struct ResponseMeta {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub final_url: Url,
    pub http_version: HttpVersion,
}

impl ResponseMeta {
    /// Case-insensitive response header lookup.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// **Streaming fetch** (the B-latency enabler): GET `url`, follow redirects, and
/// deliver the `Content-Encoding`-decoded body to `on_chunk` **as chunks arrive** off
/// the socket — never buffering the whole body. Feed each chunk to a
/// [`manuk_html::StreamParser`](../manuk_html/struct.StreamParser.html) for a
/// first-paint checkpoint before the tail lands. Returns the response metadata once
/// the body completes.
pub async fn fetch_streaming<F: FnMut(&[u8])>(url: &str, mut on_chunk: F) -> Result<ResponseMeta> {
    let mut current = Url::parse(url).with_context(|| format!("invalid URL: {url}"))?;
    for _ in 0..=MAX_REDIRECTS {
        let resp = send_raw("GET", &current, &[], Bytes::new()).await?;
        let status = resp.status().as_u16();

        // Follow 3xx (its body is dropped unconsumed when `resp` goes out of scope).
        if (300..400).contains(&status) {
            if let Some(loc) = resp.headers().get(LOCATION).and_then(|v| v.to_str().ok()) {
                let next = current
                    .join(loc)
                    .with_context(|| format!("bad redirect target: {loc}"))?;
                tracing::debug!(%current, %next, status, "following redirect (streaming)");
                current = next;
                continue;
            }
        }

        // Final response: stream its decoded body to the sink.
        let http_version = resp.version().into();
        let headers = collect_headers(&resp);
        let encoding = content_encoding(&resp);
        stream_body_decoded(resp.into_body(), encoding.as_deref(), &mut on_chunk).await?;
        return Ok(ResponseMeta {
            status,
            headers,
            final_url: current,
            http_version,
        });
    }
    bail!("too many redirects (>{MAX_REDIRECTS}) starting at {url}")
}

/// Outcome of a speculative preconnect attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preconnect {
    /// TCP + TLS handshake warmed (DNS resolved, TLS session cached for resumption).
    Warmed,
    /// The origin was warmed within the idle window — nothing to do.
    SkippedRecent,
    /// The in-flight preconnect budget is full.
    SkippedBusy,
    /// **Privacy policy:** cross-origin preconnects are never done speculatively.
    DeclinedCrossOrigin,
    /// The target is not an `http(s)` URL.
    DeclinedScheme,
    /// The connection attempt failed (host down, TLS error, …).
    Failed,
}

/// **Speculative preconnect** (B-latency): on a user gesture (link hover / pointer-
/// down) warm the connection to the link's origin so the click reuses it. Warming is a
/// TCP + TLS handshake only — **no HTTP request** — and populates the shared connector's
/// TLS session cache (so the real navigation resumes the handshake).
///
/// Privacy (CLAUDE.md Axis F): **user-initiated + same-origin only**. Speculative
/// cross-origin preconnect is refused ([`Preconnect::DeclinedCrossOrigin`]) so hover
/// intent never leaks to a third-party origin. (Page-*declared* `<link rel=preconnect>`
/// to a subresource origin is a separate, solicited path handled by the D4 scheduler.)
/// Bounded by an in-flight cap and a per-origin idle window.
pub struct Preconnector {
    warmed: std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>,
    in_flight: std::sync::atomic::AtomicUsize,
    max_in_flight: usize,
    idle: std::time::Duration,
}

impl Default for Preconnector {
    fn default() -> Self {
        Self::new()
    }
}

impl Preconnector {
    pub fn new() -> Self {
        Preconnector {
            warmed: std::sync::Mutex::new(std::collections::HashMap::new()),
            in_flight: std::sync::atomic::AtomicUsize::new(0),
            max_in_flight: 6,
            idle: std::time::Duration::from_secs(10),
        }
    }

    /// Pure policy: may we speculatively preconnect from `current_page` to `target`?
    /// `Ok(origin)` if allowed (same-origin `http(s)`), else the declining outcome.
    pub fn classify(current_page: &str, target: &str) -> Result<String, Preconnect> {
        let tgt = Url::parse(target).map_err(|_| Preconnect::DeclinedScheme)?;
        if !matches!(tgt.scheme(), "http" | "https") {
            return Err(Preconnect::DeclinedScheme);
        }
        let cur = Url::parse(current_page).map_err(|_| Preconnect::DeclinedCrossOrigin)?;
        // Same-origin only (scheme + host + port must match).
        if cur.origin() != tgt.origin() {
            return Err(Preconnect::DeclinedCrossOrigin);
        }
        Ok(origin_key(&tgt))
    }

    /// Warm the connection to `target`'s origin if policy + budget allow. `current_page`
    /// is the page the gesture happened on (for the same-origin check).
    pub async fn preconnect(&self, current_page: &str, target: &str) -> Preconnect {
        let origin = match Self::classify(current_page, target) {
            Ok(o) => o,
            Err(decline) => return decline,
        };

        // Per-origin idle window.
        {
            let map = self.warmed.lock().unwrap();
            if let Some(&at) = map.get(&origin) {
                if at.elapsed() < self.idle {
                    return Preconnect::SkippedRecent;
                }
            }
        }

        // In-flight budget.
        use std::sync::atomic::Ordering;
        if self.in_flight.load(Ordering::Relaxed) >= self.max_in_flight {
            return Preconnect::SkippedBusy;
        }
        self.in_flight.fetch_add(1, Ordering::Relaxed);
        self.warmed
            .lock()
            .unwrap()
            .insert(origin, std::time::Instant::now());

        let result = warm_origin(target).await;
        self.in_flight.fetch_sub(1, Ordering::Relaxed);
        if result {
            Preconnect::Warmed
        } else {
            Preconnect::Failed
        }
    }
}

/// R4 — speculatively warm the connection to `target`'s origin (same-origin policy + recency
/// + budget enforced by the process-global [`Preconnector`]). Fire-and-forget from a link
/// hover or the omnibox: the subsequent real request reuses the warm TLS connection.
pub async fn preconnect(current_page: &str, target: &str) -> Preconnect {
    static P: std::sync::OnceLock<Preconnector> = std::sync::OnceLock::new();
    P.get_or_init(Preconnector::new).preconnect(current_page, target).await
}

/// Origin key `scheme://host:port` for the warmed-origins map.
fn origin_key(u: &Url) -> String {
    format!(
        "{}://{}:{}",
        u.scheme(),
        u.host_str().unwrap_or(""),
        u.port_or_known_default().unwrap_or(0)
    )
}

/// Establish (and immediately drop) a TCP + TLS connection to `target`'s origin — a
/// request-free preconnect that warms DNS + the TLS session cache.
async fn warm_origin(target: &str) -> bool {
    use tower_service::Service;
    let Ok(uri) = target.parse::<hyper::Uri>() else {
        return false;
    };
    let mut conn = connector();
    // Drive the connector's `Service<Uri>` to a connection, then drop it.
    let ready = std::future::poll_fn(|cx| conn.poll_ready(cx)).await;
    if ready.is_err() {
        return false;
    }
    conn.call(uri).await.is_ok()
}

/// A general single request (no redirect following): any method, extra headers, and
/// a request body. Used for API calls (e.g. `POST` JSON to an LLM endpoint).
pub async fn request(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Bytes,
) -> Result<Response> {
    let u = Url::parse(url).with_context(|| format!("invalid URL: {url}"))?;
    send_once(method, &u, headers, body).await
}

/// Process-global RFC-6265 cookie jar shared by every request (U6). Single-profile for now;
/// per-container/site partitioning (via `storage.rs`) and disk persistence are follow-ons.
/// Where persistent cookies live: `$MANUK_STATE` / `$XDG_STATE_HOME/manuk` / `~/.local/state/manuk`
/// (mirrors the shell's session store), file `cookies.json`.
pub fn cookie_store_path() -> std::path::PathBuf {
    let dir = if let Some(d) = std::env::var_os("MANUK_STATE") {
        std::path::PathBuf::from(d)
    } else if let Some(d) = std::env::var_os("XDG_STATE_HOME") {
        std::path::PathBuf::from(d).join("manuk")
    } else if let Some(home) = std::env::var_os("HOME") {
        std::path::PathBuf::from(home).join(".local/state/manuk")
    } else {
        std::path::PathBuf::from(".manuk")
    };
    dir.join("cookies.json")
}

fn cookie_jar() -> &'static std::sync::Mutex<cookies::CookieJar> {
    static JAR: std::sync::OnceLock<std::sync::Mutex<cookies::CookieJar>> = std::sync::OnceLock::new();
    // Load persistent cookies from disk on first use, so a prior session's logins survive.
    JAR.get_or_init(|| std::sync::Mutex::new(cookies::CookieJar::load_from(&cookie_store_path())))
}

/// `document.cookie` (getter) for `url` — the **same jar the network uses**, minus `HttpOnly`
/// cookies, which script must never see. That exclusion is the whole point of the flag: it is what
/// stops an XSS from reading a session token.
pub fn document_cookie(url: &str) -> String {
    let Ok(u) = Url::parse(url) else {
        return String::new();
    };
    let Ok(jar) = cookie_jar().lock() else {
        return String::new();
    };
    jar.cookie_header_where(&u, std::time::SystemTime::now(), |c| !c.http_only)
        .unwrap_or_default()
}

/// `document.cookie = "..."` (setter) — one `Set-Cookie`-shaped assignment into the real jar, so a
/// cookie a script writes is a cookie the next request sends. Scripts cannot set `HttpOnly`.
pub fn set_document_cookie(url: &str, assignment: &str) -> bool {
    let Ok(u) = Url::parse(url) else {
        return false;
    };
    if assignment.to_ascii_lowercase().contains("httponly") {
        return false;
    }
    let Ok(mut jar) = cookie_jar().lock() else {
        return false;
    };
    jar.store(&u, assignment)
}

/// Flush persistent cookies to disk. Call on navigation-commit and on quit so logins with a
/// multi-week expiry survive a restart. Best-effort; a write failure is logged, not fatal.
pub fn save_cookies() {
    if let Ok(jar) = cookie_jar().lock() {
        if let Err(e) = jar.save_to(&cookie_store_path()) {
            tracing::warn!(error = %e, "failed to persist cookies");
        }
    }
}

async fn send_once(
    method: &str,
    url: &Url,
    headers: &[(&str, &str)],
    body: Bytes,
) -> Result<Response> {
    // Attach any stored cookies for this URL (so a logged-in session stays logged in).
    let cookie = cookie_jar().lock().ok().and_then(|j| j.cookie_header(url));
    let mut hdrs: Vec<(&str, &str)> = headers.to_vec();
    if let Some(c) = &cookie {
        hdrs.push(("cookie", c.as_str()));
    }
    let resp = send_raw(method, url, &hdrs, body).await?;
    let status = resp.status().as_u16();
    let http_version = resp.version().into();
    let headers_vec = collect_headers(&resp);
    // Store any Set-Cookie the server sent, and flush to disk if we saw one (so a login
    // response's persistent cookies survive a restart without waiting for a clean quit).
    let mut saw_set_cookie = false;
    if let Ok(mut jar) = cookie_jar().lock() {
        for (k, v) in &headers_vec {
            if k.eq_ignore_ascii_case("set-cookie") {
                jar.store(url, v);
                saw_set_cookie = true;
            }
        }
    }
    if saw_set_cookie {
        save_cookies();
    }
    let encoding = content_encoding(&resp);

    let decoded = read_body_decoded(resp.into_body(), encoding.as_deref()).await?;

    Ok(Response {
        status,
        headers: headers_vec,
        body: decoded,
        final_url: url.clone(),
        http_version,
    })
}

/// Build and send one request, returning the raw hyper response with its body
/// **unconsumed** (so callers can either buffer it or stream it).
async fn send_raw(
    method: &str,
    url: &Url,
    headers: &[(&str, &str)],
    body: Bytes,
) -> Result<hyper::Response<Incoming>> {
    match url.scheme() {
        "http" | "https" => {}
        other => bail!("unsupported URL scheme: {other}"),
    }

    let mut builder = Request::builder()
        .method(method)
        .uri(url.as_str())
        .header(USER_AGENT, user_agent());
    let (mut has_accept, mut has_al, mut has_ae) = (false, false, false);
    for (k, v) in headers {
        has_accept |= k.eq_ignore_ascii_case("accept");
        has_al |= k.eq_ignore_ascii_case("accept-language");
        has_ae |= k.eq_ignore_ascii_case("accept-encoding");
        builder = builder.header(*k, *v);
    }
    // A complete, consistently-ordered default header set (F1) — Accept,
    // Accept-Language, Accept-Encoding — added only when the caller didn't set them.
    if !has_accept {
        builder = builder.header(ACCEPT, "text/html,application/xhtml+xml,*/*;q=0.8");
    }
    if !has_al {
        builder = builder.header(ACCEPT_LANGUAGE, ACCEPT_LANGUAGE_STR);
    }
    if !has_ae {
        builder = builder.header(ACCEPT_ENCODING, "gzip, deflate, br");
    }
    let req = builder.body(Full::new(body)).context("building request")?;

    client()
        .request(req)
        .await
        .with_context(|| format!("request to {url} failed"))
}

fn collect_headers(resp: &hyper::Response<Incoming>) -> Vec<(String, String)> {
    resp.headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_owned(), v.to_str().unwrap_or("").to_owned()))
        .collect()
}

fn content_encoding(resp: &hyper::Response<Incoming>) -> Option<String> {
    resp.headers()
        .get(CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_ascii_lowercase())
}

/// Build an `AsyncBufRead` over a hyper body's data frames (dropping trailers).
/// `Box::pin` makes the async-`filter_map` stream `Unpin`, which the decoders need.
fn body_reader(body: Incoming) -> impl tokio::io::AsyncBufRead + Send {
    let data = Box::pin(BodyStream::new(body).filter_map(|frame| async move {
        match frame {
            Ok(f) => f.into_data().ok().map(Ok),
            Err(e) => Some(Err(std::io::Error::other(e))),
        }
    }));
    tokio::io::BufReader::new(StreamReader::new(data))
}

/// Wrap `reader` in the right `Content-Encoding` decoder (gzip/br/deflate/identity),
/// as a boxed `AsyncRead`.
fn wrap_decoder<R: tokio::io::AsyncBufRead + Unpin + Send + 'static>(
    reader: R,
    encoding: Option<&str>,
) -> std::pin::Pin<Box<dyn tokio::io::AsyncRead + Send>> {
    use async_compression::tokio::bufread as ac;
    match encoding {
        Some("gzip") | Some("x-gzip") => Box::pin(ac::GzipDecoder::new(reader)),
        Some("br") => Box::pin(ac::BrotliDecoder::new(reader)),
        Some("deflate") => Box::pin(ac::ZlibDecoder::new(reader)),
        _ => Box::pin(reader), // identity / unknown
    }
}

/// Stream the response body, decode `Content-Encoding` on the fly, and hand each
/// decoded chunk to `on_chunk` (never buffering the whole body).
async fn stream_body_decoded<F: FnMut(&[u8])>(
    body: Incoming,
    encoding: Option<&str>,
    on_chunk: &mut F,
) -> Result<()> {
    let mut decoded = wrap_decoder(body_reader(body), encoding);
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        let n = decoded.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        on_chunk(&buf[..n]);
    }
    Ok(())
}

/// Buffer the whole response body, decoding `Content-Encoding` on the fly. (The
/// streaming counterpart is [`stream_body_decoded`]; both share [`body_reader`] +
/// [`wrap_decoder`].)
async fn read_body_decoded(body: Incoming, encoding: Option<&str>) -> Result<Bytes> {
    let mut decoded = wrap_decoder(body_reader(body), encoding);
    let mut out = Vec::new();
    decoded.read_to_end(&mut out).await?;
    Ok(Bytes::from(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn rejects_unknown_scheme() {
        let err = rt().block_on(fetch("ftp://example.com/")).unwrap_err();
        assert!(err.to_string().contains("scheme"), "got: {err}");
    }

    #[test]
    fn user_agent_is_honest() {
        let ua = user_agent();
        // Truthful: names Manuk + the universal Mozilla/5.0 compat token.
        assert!(ua.starts_with("Mozilla/5.0 ("), "got: {ua}");
        assert!(ua.contains("Manuk/"), "must name Manuk: {ua}");
        // The no-mimicry guard: never impersonate a mainstream browser engine.
        for competitor in [
            "Chrome",
            "Safari",
            "Firefox",
            "Edg",
            "AppleWebKit",
            "Gecko/",
        ] {
            assert!(
                !ua.contains(competitor),
                "UA must not mimic {competitor}: {ua}"
            );
        }
    }

    #[test]
    fn preconnect_policy_is_same_origin_only() {
        let cur = "https://example.com/page";
        // Same origin (any path) → allowed.
        assert!(Preconnector::classify(cur, "https://example.com/other").is_ok());
        // Cross-origin (host/scheme/port differ) → refused for privacy.
        assert_eq!(
            Preconnector::classify(cur, "https://evil.test/track"),
            Err(Preconnect::DeclinedCrossOrigin)
        );
        assert_eq!(
            Preconnector::classify(cur, "http://example.com/"),
            Err(Preconnect::DeclinedCrossOrigin) // scheme differs → different origin
        );
        assert_eq!(
            Preconnector::classify(cur, "https://example.com:8443/"),
            Err(Preconnect::DeclinedCrossOrigin) // port differs
        );
        // Non-http(s) target → declined.
        assert_eq!(
            Preconnector::classify(cur, "ftp://example.com/"),
            Err(Preconnect::DeclinedScheme)
        );
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn preconnect_warms_then_skips_recent() {
        let pc = Preconnector::new();
        // Same-origin hover → warm the connection (TCP+TLS, no request).
        let first = pc
            .preconnect("https://example.com/", "https://example.com/path")
            .await;
        assert_eq!(first, Preconnect::Warmed);
        // A second hover within the idle window is skipped.
        let second = pc
            .preconnect("https://example.com/", "https://example.com/other")
            .await;
        assert_eq!(second, Preconnect::SkippedRecent);
        // Cross-origin is refused without touching the network.
        assert_eq!(
            pc.preconnect("https://example.com/", "https://other.test/")
                .await,
            Preconnect::DeclinedCrossOrigin
        );
    }

    #[test]
    fn charset_via_content_type() {
        // 0xE9 is `é` in windows-1252.
        let s = charset::decode_html(
            b"<html>caf\xe9</html>",
            Some("text/html; charset=windows-1252"),
        );
        assert!(s.contains("café"), "got: {s}");
    }

    #[test]
    fn charset_via_meta_prescan() {
        let s = charset::decode_html(b"<meta charset=\"windows-1252\"><p>caf\xe9</p>", None);
        assert!(s.contains("café"), "got: {s}");
    }

    #[test]
    fn charset_bom_wins_over_content_type() {
        // UTF-8 BOM must override a conflicting Content-Type charset.
        let s = charset::decode_html(
            "\u{feff}<p>über</p>".as_bytes(),
            Some("text/html; charset=windows-1252"),
        );
        assert!(s.contains("über"), "got: {s}");
    }

    // Live network tests — run with `cargo test -p manuk-net -- --ignored`.
    #[tokio::test]
    #[ignore = "requires network access"]
    async fn fetches_example_com_and_negotiates_h2() {
        let resp = fetch("https://example.com/").await.unwrap();
        assert_eq!(resp.status, 200);
        assert!(resp.text().to_lowercase().contains("example domain"));
        // example.com offers h2 via ALPN.
        assert_eq!(resp.http_version, HttpVersion::Http2, "expected HTTP/2");
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn streaming_fetch_reassembles_body() {
        // Stream example.com and reassemble the chunks; the result must equal the
        // buffered fetch's body (proves the chunked path decodes identically), and it
        // should arrive in one or more chunks.
        let mut chunks = 0usize;
        let mut assembled = Vec::new();
        let meta = fetch_streaming("https://example.com/", |c| {
            chunks += 1;
            assembled.extend_from_slice(c);
        })
        .await
        .unwrap();
        assert_eq!(meta.status, 200);
        assert!(chunks >= 1, "body delivered in at least one chunk");
        let text = String::from_utf8_lossy(&assembled);
        assert!(text.to_lowercase().contains("example domain"));

        let buffered = fetch("https://example.com/").await.unwrap();
        assert_eq!(
            assembled,
            buffered.body.as_ref(),
            "streamed body must match the buffered body"
        );
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn connection_pool_reused() {
        // Two sequential same-origin fetches through the shared pooled client; the
        // second reuses the idle connection (no new TLS handshake).
        let a = fetch("https://example.com/").await.unwrap();
        let b = fetch("https://example.com/").await.unwrap();
        assert_eq!(a.status, 200);
        assert_eq!(b.status, 200);
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn decodes_gzip() {
        // httpbin gzips the body; our async-compression path must decode it to JSON.
        let resp = fetch("https://httpbin.org/gzip").await.unwrap();
        assert_eq!(resp.status, 200);
        assert!(resp.text().contains("\"gzipped\""), "body: {}", resp.text());
    }
}
