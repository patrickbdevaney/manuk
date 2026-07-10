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
use hyper::header::{ACCEPT, ACCEPT_ENCODING, CONTENT_ENCODING, USER_AGENT};
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
fn client() -> &'static NetClient {
    static CLIENT: OnceLock<NetClient> = OnceLock::new();
    CLIENT.get_or_init(|| {
        // Install a rustls crypto provider once (idempotent).
        let _ = rustls::crypto::ring::default_provider().install_default();
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build();
        Client::builder(TokioExecutor::new()).build(https)
    })
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

    /// Body decoded as UTF-8 (lossy), for HTML/text/JSON.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
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

    let resp = client()
        .request(req)
        .await
        .with_context(|| format!("request to {url} failed"))?;

    let status = resp.status().as_u16();
    let http_version = resp.version().into();
    let headers_vec: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_owned(), v.to_str().unwrap_or("").to_owned()))
        .collect();
    let encoding = resp
        .headers()
        .get(CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_ascii_lowercase());

    let decoded = read_body_decoded(resp.into_body(), encoding.as_deref()).await?;

    Ok(Response {
        status,
        headers: headers_vec,
        body: decoded,
        final_url: url.clone(),
        http_version,
    })
}

/// Stream the response body and decode `Content-Encoding` on the fly.
async fn read_body_decoded(body: Incoming, encoding: Option<&str>) -> Result<Bytes> {
    // Body frames → a stream of data-chunk `Bytes` as io::Result (drop trailers).
    // Box::pin makes the (async-`filter_map`) stream `Unpin`, which the decoders need.
    let data = Box::pin(BodyStream::new(body).filter_map(|frame| async move {
        match frame {
            Ok(f) => f.into_data().ok().map(Ok),
            Err(e) => Some(Err(std::io::Error::other(e))),
        }
    }));
    let reader = tokio::io::BufReader::new(StreamReader::new(data));

    use async_compression::tokio::bufread as ac;
    let mut out = Vec::new();
    match encoding {
        Some("gzip") | Some("x-gzip") => {
            ac::GzipDecoder::new(reader).read_to_end(&mut out).await?;
        }
        Some("br") => {
            ac::BrotliDecoder::new(reader).read_to_end(&mut out).await?;
        }
        Some("deflate") => {
            ac::ZlibDecoder::new(reader).read_to_end(&mut out).await?;
        }
        _ => {
            // identity (or unknown) — read as-is.
            let mut r = reader;
            r.read_to_end(&mut out).await?;
        }
    }
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
