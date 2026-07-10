//! manuk-net — the networking layer.
//!
//! Per CLAUDE.md we *reuse* the mature Rust networking stack rather than
//! reimplementing protocols where a bug is a security/correctness risk:
//! `tokio` (async runtime), `hyper` (HTTP/1.1, HTTP/2), `rustls` (pure-Rust TLS,
//! no OpenSSL system dependency), with `webpki-roots` for trust anchors.
//!
//! [`fetch`] is GET-with-redirects for page loads; [`request`] is a general single
//! request (any method, custom headers, request body) used by API clients such as
//! the agent phase's Groq backend — so even outbound LLM calls go through this one
//! pure-Rust stack. HTTP/2 (ALPN `h2`) and HTTP/3/QUIC (`quinn`) slot in behind the
//! same surface.

use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::header::{HeaderValue, HOST, USER_AGENT};
use hyper::Request;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use url::Url;

/// Identifies the engine to origin servers. Kept generic and standards-plain — we
/// do not spoof another browser's UA (CLAUDE.md: no bot-evasion in the core engine).
const USER_AGENT_STR: &str = concat!("Manuk/", env!("CARGO_PKG_VERSION"), " (+standards)");

/// Maximum number of 3xx redirects to follow before giving up.
const MAX_REDIRECTS: usize = 10;

/// A fetched HTTP response.
#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Bytes,
    /// The URL the body actually came from (after any redirects).
    pub final_url: Url,
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

/// Any stream we can speak HTTP over (plain TCP or TLS), boxed so both branches
/// unify to one send path.
trait ReadWrite: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> ReadWrite for T {}

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
/// an optional body. Used for API calls (e.g. `POST` JSON to an LLM endpoint).
pub async fn request(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Bytes,
) -> Result<Response> {
    let u = Url::parse(url).with_context(|| format!("invalid URL: {url}"))?;
    send_once(method, &u, headers, body).await
}

/// One request/response over a freshly-opened (optionally TLS) connection.
async fn send_once(
    method: &str,
    url: &Url,
    headers: &[(&str, &str)],
    body: Bytes,
) -> Result<Response> {
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("URL has no host: {url}"))?
        .to_owned();
    let https = match url.scheme() {
        "https" => true,
        "http" => false,
        other => bail!("unsupported URL scheme: {other}"),
    };
    let port = url
        .port_or_known_default()
        .unwrap_or(if https { 443 } else { 80 });
    let authority = match url.port() {
        Some(p) => format!("{host}:{p}"),
        None => host.clone(),
    };
    let mut target = url.path().to_owned();
    if let Some(q) = url.query() {
        target.push('?');
        target.push_str(q);
    }
    if target.is_empty() {
        target.push('/');
    }

    let tcp = TcpStream::connect((host.as_str(), port))
        .await
        .with_context(|| format!("TCP connect to {host}:{port} failed"))?;
    tcp.set_nodelay(true).ok();

    // Unify TLS and plain streams to one boxed type so `body` moves exactly once.
    let stream: Box<dyn ReadWrite> = if https {
        Box::new(tls_connect(tcp, &host).await?)
    } else {
        Box::new(tcp)
    };

    let mut resp = send_request(
        TokioIo::new(stream),
        method,
        &authority,
        &target,
        headers,
        body,
    )
    .await?;
    resp.final_url = url.clone();
    Ok(resp)
}

/// Wrap a TCP stream in TLS using rustls with webpki trust anchors.
async fn tls_connect(
    tcp: TcpStream,
    host: &str,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .context("rustls: no supported TLS protocol versions")?
    .with_root_certificates(roots)
    .with_no_client_auth();
    // We speak HTTP/1.1 for now; advertise it so servers don't pick h2.
    config.alpn_protocols = vec![b"http/1.1".to_vec()];

    let connector = TlsConnector::from(Arc::new(config));
    let server_name = rustls_pki_types::ServerName::try_from(host.to_owned())
        .with_context(|| format!("invalid TLS server name: {host}"))?;
    connector
        .connect(server_name, tcp)
        .await
        .with_context(|| format!("TLS handshake with {host} failed"))
}

/// Perform one HTTP/1.1 exchange over an established stream.
async fn send_request<S>(
    io: TokioIo<S>,
    method: &str,
    authority: &str,
    target: &str,
    headers: &[(&str, &str)],
    body: Bytes,
) -> Result<Response>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .context("HTTP/1.1 handshake failed")?;
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            tracing::debug!("connection task ended: {e}");
        }
    });

    let mut builder = Request::builder()
        .method(method)
        .uri(target)
        .header(HOST, HeaderValue::from_str(authority)?)
        .header(USER_AGENT, USER_AGENT_STR);
    let mut has_accept = false;
    for (k, v) in headers {
        if k.eq_ignore_ascii_case("accept") {
            has_accept = true;
        }
        builder = builder.header(*k, *v);
    }
    if !has_accept {
        builder = builder.header("Accept", "*/*");
    }

    let req = builder.body(Full::new(body)).context("building request")?;

    let res = sender
        .send_request(req)
        .await
        .context("sending request / reading response head")?;

    let status = res.status().as_u16();
    let resp_headers = res
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_owned(), v.to_str().unwrap_or("").to_owned()))
        .collect();
    let resp_body = res
        .into_body()
        .collect()
        .await
        .context("reading response body")?
        .to_bytes();

    Ok(Response {
        status,
        headers: resp_headers,
        body: resp_body,
        final_url: Url::parse("about:blank").unwrap(), // overwritten by send_once
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_scheme() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let err = rt.block_on(fetch("ftp://example.com/")).unwrap_err();
        assert!(err.to_string().contains("scheme"), "got: {err}");
    }

    // Live network test — run with `cargo test -p manuk-net -- --ignored`.
    #[tokio::test]
    #[ignore = "requires network access"]
    async fn fetches_example_com() {
        let resp = fetch("https://example.com/").await.unwrap();
        assert_eq!(resp.status, 200);
        assert!(resp.text().to_lowercase().contains("example domain"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn post_httpbin() {
        let resp = request(
            "POST",
            "https://httpbin.org/post",
            &[("Content-Type", "application/json")],
            Bytes::from_static(b"{\"hi\":1}"),
        )
        .await
        .unwrap();
        assert_eq!(resp.status, 200);
    }
}
