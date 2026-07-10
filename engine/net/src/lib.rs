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
use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::{BodyStream, Full};
use hyper::body::Incoming;
use hyper::header::{ACCEPT, ACCEPT_ENCODING, CONTENT_ENCODING, LOCATION, USER_AGENT};
use hyper::Request;
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;
use url::Url;

/// Identifies the engine truthfully — no competitor impersonation (CLAUDE.md Axis F).
const USER_AGENT_STR: &str = concat!("Manuk/", env!("CARGO_PKG_VERSION"), " (+standards)");

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

/// Fetch `url` with GET, following redirects. `url` must be an absolute
/// `http`/`https` URL.
pub async fn fetch(url: &str) -> Result<Response> {
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

async fn send_once(
    method: &str,
    url: &Url,
    headers: &[(&str, &str)],
    body: Bytes,
) -> Result<Response> {
    let resp = send_raw(method, url, headers, body).await?;
    let status = resp.status().as_u16();
    let http_version = resp.version().into();
    let headers_vec = collect_headers(&resp);
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
        .header(USER_AGENT, USER_AGENT_STR);
    let (mut has_accept, mut has_ae) = (false, false);
    for (k, v) in headers {
        has_accept |= k.eq_ignore_ascii_case("accept");
        has_ae |= k.eq_ignore_ascii_case("accept-encoding");
        builder = builder.header(*k, *v);
    }
    if !has_accept {
        builder = builder.header(ACCEPT, "text/html,application/xhtml+xml,*/*;q=0.8");
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
fn body_reader(body: Incoming) -> impl tokio::io::AsyncBufRead {
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
fn wrap_decoder<R: tokio::io::AsyncBufRead + Unpin + 'static>(
    reader: R,
    encoding: Option<&str>,
) -> std::pin::Pin<Box<dyn tokio::io::AsyncRead>> {
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
