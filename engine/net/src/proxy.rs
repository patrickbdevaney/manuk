//! E7, part 3 — **SOCKS5 proxying** ("VPN" without bundling a VPN).
//!
//! Per CLAUDE.md / IMPLEMENTATION.md the disposition is: **reuse `tokio-socks`** and
//! route through a *user-provided* SOCKS5 proxy. Bundling a WireGuard/OpenVPN client
//! is explicitly **out of scope**; a user who wants WireGuard runs `wireproxy` and
//! points us at its local SOCKS port.
//!
//! [`fetch_via_proxy`] performs a single request over a proxied connection using
//! hyper's low-level `client::conn` API (rather than the pooled `Client`), because a
//! proxied connection is per-request configuration, not a process-global pool.
//!
//! **Documented gaps (not faked):** no proxy connection pooling; `http://` and
//! `https://` only; redirects are not followed here (callers compose that, as
//! [`crate::fetch`] does); SOCKS4 and HTTP `CONNECT` proxies are not implemented.

use anyhow::{bail, Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, HOST, USER_AGENT};
use hyper::Request;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_socks::tcp::Socks5Stream;
use url::Url;

use crate::{HttpVersion, Response};

/// A user-provided SOCKS5 proxy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SocksProxy {
    /// `host:port` of the SOCKS5 server (e.g. `127.0.0.1:1080`, or `wireproxy`'s port).
    pub addr: String,
    /// Optional username/password authentication (RFC 1929).
    pub auth: Option<(String, String)>,
}

impl SocksProxy {
    pub fn new(addr: impl Into<String>) -> Self {
        SocksProxy {
            addr: addr.into(),
            auth: None,
        }
    }

    pub fn with_auth(mut self, user: impl Into<String>, pass: impl Into<String>) -> Self {
        self.auth = Some((user.into(), pass.into()));
        self
    }

    /// Open a TCP stream to `host:port` **through** this proxy.
    ///
    /// The target is passed to the proxy as a *domain name*, never resolved locally —
    /// so DNS happens at the proxy, not on this machine. That is the whole point of
    /// routing through it: a locally-resolved hostname would leak the destination.
    pub async fn connect(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Socks5Stream<tokio::net::TcpStream>> {
        let target = (host, port);
        let stream = match &self.auth {
            Some((u, p)) => Socks5Stream::connect_with_password(self.addr.as_str(), target, u, p)
                .await
                .with_context(|| format!("SOCKS5 connect (auth) via {}", self.addr))?,
            None => Socks5Stream::connect(self.addr.as_str(), target)
                .await
                .with_context(|| format!("SOCKS5 connect via {}", self.addr))?,
        };
        Ok(stream)
    }
}

/// Split `url` into (host, port) using the scheme's default port when absent.
fn host_port(url: &Url) -> Result<(String, u16)> {
    let host = url.host_str().context("URL has no host")?.to_string();
    let port = url.port_or_known_default().context("no default port")?;
    Ok((host, port))
}

/// Perform a GET for `url` over `proxy`. Redirects are **not** followed.
pub async fn fetch_via_proxy(proxy: &SocksProxy, url: &str) -> Result<Response> {
    let url = Url::parse(url).context("invalid URL")?;
    let (host, port) = host_port(&url)?;

    let stream = proxy.connect(&host, port).await?;

    match url.scheme() {
        "http" => send_request(stream, &url, &host).await,
        "https" => {
            let tls = tls_connect(stream, &host).await?;
            send_request(tls, &url, &host).await
        }
        other => bail!("unsupported scheme for proxy fetch: {other}"),
    }
}

/// Wrap a TCP stream in TLS. Certificate validation is the **normal** webpki path — proxying must
/// never weaken TLS verification.
///
/// `pub(crate)` because `wss://` needs exactly this connector: the ring-pinned one. Letting
/// `tokio-tungstenite` bring its own TLS would re-enable the `aws-lc` backend across the whole
/// dependency graph via cargo's feature union (see the warning in `Cargo.toml`).
pub(crate) async fn tls_connect<S>(
    stream: S,
    host: &str,
) -> Result<tokio_rustls::client::TlsStream<S>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let _ = rustls::crypto::ring::default_provider().install_default();
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
    let server_name = rustls_pki_types::ServerName::try_from(host.to_string())
        .context("invalid DNS name for TLS")?;
    connector
        .connect(server_name, stream)
        .await
        .context("TLS handshake through proxy failed")
}

/// Drive one HTTP/1.1 request/response over an already-connected stream.
async fn send_request<S>(stream: S, url: &Url, host: &str) -> Result<Response>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .context("HTTP handshake through proxy failed")?;
    // Drive the connection in the background; it ends when the response is consumed.
    tokio::spawn(async move {
        let _ = conn.await;
    });

    let path = match url.query() {
        Some(q) => format!("{}?{}", url.path(), q),
        None => url.path().to_string(),
    };
    let authority = match url.port() {
        Some(p) => format!("{host}:{p}"),
        None => host.to_string(),
    };

    let req = Request::builder()
        .uri(path)
        .header(HOST, authority)
        .header(USER_AGENT, crate::user_agent())
        .header(ACCEPT, "text/html,application/xhtml+xml,*/*;q=0.8")
        .header(ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .header(ACCEPT_ENCODING, "identity")
        .body(Full::<Bytes>::default())
        .context("building proxied request")?;

    let resp = sender
        .send_request(req)
        .await
        .context("proxied request failed")?;
    let status = resp.status().as_u16();
    let version: HttpVersion = resp.version().into();
    let headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let body = resp
        .into_body()
        .collect()
        .await
        .context("reading proxied body")?
        .to_bytes();

    Ok(Response {
        status,
        headers,
        body,
        final_url: url.clone(),
        http_version: version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    /// A minimal origin server that answers any request with a fixed body.
    async fn spawn_origin() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    let _ = sock.read(&mut buf).await;
                    let body = "proxied-hello";
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.flush().await;
                });
            }
        });
        addr
    }

    /// A minimal no-auth SOCKS5 server. Counts the connections it proxies so the test
    /// can prove the traffic actually went through it.
    async fn spawn_socks5(counter: Arc<AtomicUsize>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            while let Ok((mut client, _)) = listener.accept().await {
                let counter = counter.clone();
                tokio::spawn(async move {
                    // --- greeting: VER=5, NMETHODS, METHODS...
                    let mut head = [0u8; 2];
                    if client.read_exact(&mut head).await.is_err() || head[0] != 5 {
                        return;
                    }
                    let mut methods = vec![0u8; head[1] as usize];
                    if client.read_exact(&mut methods).await.is_err() {
                        return;
                    }
                    // choose "no authentication"
                    if client.write_all(&[5, 0]).await.is_err() {
                        return;
                    }

                    // --- request: VER, CMD, RSV, ATYP, ADDR, PORT
                    let mut req = [0u8; 4];
                    if client.read_exact(&mut req).await.is_err() {
                        return;
                    }
                    let (ver, cmd, atyp) = (req[0], req[1], req[3]);
                    if ver != 5 || cmd != 1 {
                        return; // only CONNECT
                    }
                    let host = match atyp {
                        1 => {
                            let mut a = [0u8; 4];
                            if client.read_exact(&mut a).await.is_err() {
                                return;
                            }
                            format!("{}.{}.{}.{}", a[0], a[1], a[2], a[3])
                        }
                        3 => {
                            let mut l = [0u8; 1];
                            if client.read_exact(&mut l).await.is_err() {
                                return;
                            }
                            let mut d = vec![0u8; l[0] as usize];
                            if client.read_exact(&mut d).await.is_err() {
                                return;
                            }
                            String::from_utf8_lossy(&d).into_owned()
                        }
                        _ => return,
                    };
                    let mut p = [0u8; 2];
                    if client.read_exact(&mut p).await.is_err() {
                        return;
                    }
                    let port = u16::from_be_bytes(p);

                    // --- connect upstream and reply success
                    let Ok(mut upstream) = TcpStream::connect((host.as_str(), port)).await else {
                        let _ = client.write_all(&[5, 1, 0, 1, 0, 0, 0, 0, 0, 0]).await;
                        return;
                    };
                    counter.fetch_add(1, Ordering::SeqCst);
                    if client
                        .write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0])
                        .await
                        .is_err()
                    {
                        return;
                    }
                    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
                });
            }
        });
        addr
    }

    /// E7's second acceptance: traffic routes through a configured SOCKS proxy.
    #[tokio::test]
    async fn fetch_routes_through_the_socks5_proxy() {
        let origin = spawn_origin().await;
        let hits = Arc::new(AtomicUsize::new(0));
        let socks = spawn_socks5(hits.clone()).await;

        let proxy = SocksProxy::new(socks);
        let resp = fetch_via_proxy(&proxy, &format!("http://{origin}/hello"))
            .await
            .expect("proxied fetch should succeed");

        assert_eq!(resp.status, 200);
        assert_eq!(resp.text(), "proxied-hello");
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "the request must have traversed the SOCKS5 proxy exactly once"
        );
    }

    /// A wrong proxy address must fail loudly, never silently fall back to a direct
    /// connection — a silent fallback would leak the user's IP.
    #[tokio::test]
    async fn a_dead_proxy_fails_instead_of_connecting_directly() {
        let origin = spawn_origin().await;
        // Port 1 is reserved and nothing listens there.
        let proxy = SocksProxy::new("127.0.0.1:1");
        let err = fetch_via_proxy(&proxy, &format!("http://{origin}/hello"))
            .await
            .expect_err("must not fall back to a direct connection");
        assert!(
            format!("{err:#}").contains("SOCKS5 connect"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn proxy_auth_is_recorded() {
        let p = SocksProxy::new("127.0.0.1:1080").with_auth("u", "p");
        assert_eq!(p.auth, Some(("u".to_string(), "p".to_string())));
    }
}
