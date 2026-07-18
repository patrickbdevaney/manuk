//! **WebSocket** — live chat, DMs, presence, cloud live-logs, collaborative editing.
//!
//! Phase-0 finish-line lever 3. The page-facing `WebSocket` constructor has existed for a while as
//! an *honest stub*: it constructs, then reports failure, so a live-news site's live-blog silently
//! never updated rather than throwing. This is the transport that makes it real.
//!
//! **BORROWED, not hand-rolled** (`tokio-tungstenite`). RFC 6455 framing, client-side masking, the
//! close handshake, continuation frames and ping/pong are exactly the wheel that should not be
//! reinvented — and getting masking or the close handshake subtly wrong produces a connection that
//! works against one server and hangs against another.
//!
//! **TLS is ours, deliberately.** `tokio-tungstenite`'s own TLS features pull an unpinned
//! `tokio-rustls`, and cargo's feature UNION would re-enable the `aws-lc` backend across the whole
//! dependency graph — the exact failure documented in `engine/net/Cargo.toml` that once broke the
//! Windows build outright. So we connect the socket and run TLS with the ring-pinned connector
//! ourselves, then hand tungstenite a ready stream via `client_async`.

use anyhow::{bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::protocol::Message as TMessage;
use tokio_tungstenite::{client_async, WebSocketStream};
use url::Url;

/// A WebSocket message, in the two shapes a page can send or receive.
///
/// Ping/pong are handled inside the library and deliberately not surfaced: they are keepalive, not
/// page data, and the `WebSocket` API does not expose them either.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WsMessage {
    Text(String),
    Binary(Vec<u8>),
}

/// An open WebSocket connection.
///
/// Split rather than combined because a page reads and writes independently — a chat client sits in
/// a receive loop while a keystroke sends at any moment — and a single `&mut self` for both would
/// serialise them.
pub struct WebSocketConn {
    inner: Stream,
    /// The subprotocol the SERVER chose, `""` when none was negotiated. A client that offered
    /// `["graphql-ws"]` and got back `""` must not then speak graphql-ws at it.
    protocol: String,
}

/// `ws://` and `wss://` produce different stream types; boxing keeps one connection type.
type Stream = WebSocketStream<Box<dyn Duplex>>;

/// The trait object bound for a stream tungstenite can drive.
pub trait Duplex: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Duplex for T {}

impl WebSocketConn {
    /// The subprotocol the server selected (`""` if none).
    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    /// Open a connection to `url` (`ws://` or `wss://`), offering `protocols`.
    pub async fn connect(url: &str, protocols: &[String]) -> Result<Self> {
        let u = Url::parse(url).with_context(|| format!("invalid WebSocket URL: {url}"))?;
        let secure = match u.scheme() {
            "wss" => true,
            "ws" => false,
            other => bail!("not a WebSocket URL scheme: {other}"),
        };
        let host = u
            .host_str()
            .context("WebSocket URL has no host")?
            .to_string();
        let port = u.port().unwrap_or(if secure { 443 } else { 80 });

        let tcp = TcpStream::connect((host.as_str(), port))
            .await
            .with_context(|| format!("WebSocket TCP connect failed: {host}:{port}"))?;
        // Interactive traffic: a chat keystroke must not wait on Nagle coalescing.
        let _ = tcp.set_nodelay(true);

        let stream: Box<dyn Duplex> = if secure {
            Box::new(crate::proxy::tls_connect(tcp, &host).await?)
        } else {
            Box::new(tcp)
        };

        // Build the handshake request ourselves so `Sec-WebSocket-Protocol` can carry the page's
        // offered subprotocols — tungstenite's convenience constructor does not take them.
        let mut req = tokio_tungstenite::tungstenite::handshake::client::Request::builder()
            .uri(url)
            .header("Host", format!("{host}:{port}"))
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tokio_tungstenite::tungstenite::handshake::client::generate_key(),
            );
        if !protocols.is_empty() {
            req = req.header("Sec-WebSocket-Protocol", protocols.join(", "));
        }
        let req = req.body(()).context("building the WebSocket handshake")?;

        let (inner, resp) = client_async(req, stream)
            .await
            .with_context(|| format!("WebSocket handshake failed: {url}"))?;

        let protocol = resp
            .headers()
            .get("sec-websocket-protocol")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        Ok(Self { inner, protocol })
    }

    /// Send one message.
    pub async fn send(&mut self, msg: WsMessage) -> Result<()> {
        let m = match msg {
            WsMessage::Text(t) => TMessage::Text(t),
            WsMessage::Binary(b) => TMessage::Binary(b),
        };
        self.inner.send(m).await.context("WebSocket send failed")
    }

    /// Await the next message.
    ///
    /// `Ok(Some(msg))` is page data; `Ok(None)` means the connection ended cleanly, and an `Err`
    /// that it did not (a server that drops the socket without the closing handshake is reported,
    /// not silently treated as a clean close). Ping/pong and the library's automatic pong replies
    /// are consumed here rather than surfaced, because they are keepalive and not page data.
    ///
    /// The close CODE and reason are not surfaced yet — `onclose`'s `code`/`reason`/`wasClean` need
    /// them, and that rides with the JS surface in the next tick.
    pub async fn recv(&mut self) -> Result<Option<WsMessage>> {
        loop {
            match self.inner.next().await {
                Some(Ok(TMessage::Text(t))) => return Ok(Some(WsMessage::Text(t))),
                Some(Ok(TMessage::Binary(b))) => return Ok(Some(WsMessage::Binary(b.to_vec()))),
                // Keepalive and the close frame itself: not page data.
                Some(Ok(TMessage::Ping(_) | TMessage::Pong(_) | TMessage::Frame(_))) => continue,
                Some(Ok(TMessage::Close(_))) => return Ok(None),
                Some(Err(e)) => return Err(anyhow::Error::new(e).context("WebSocket receive")),
                None => return Ok(None),
            }
        }
    }

    /// Close the connection with the normal-closure code, completing the closing handshake.
    pub async fn close(&mut self) -> Result<()> {
        self.inner
            .close(None)
            .await
            .context("WebSocket close failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    /// **The WebSocket transport gate.** Against a REAL server (tungstenite's accept side, not a
    /// mock of our own client): the handshake completes, a subprotocol is negotiated, text and
    /// binary round-trip intact, the server can push unprompted — which is the entire point of the
    /// transport, and what polling cannot do — and a close is observed as end-of-stream.
    ///
    /// RED before this module existed: there was no client at all, only the page-facing stub that
    /// reports failure by design.
    #[test]
    fn a_websocket_round_trips_text_binary_and_a_server_push() {
        let rt = rt();
        let addr = rt.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap().to_string();
            tokio::spawn(async move {
                while let Ok((sock, _)) = listener.accept().await {
                    tokio::spawn(async move {
                        // Accept, echoing the subprotocol the client offered.
                        let cb = |req: &tokio_tungstenite::tungstenite::handshake::server::Request,
                                  mut resp: tokio_tungstenite::tungstenite::handshake::server::Response| {
                            if let Some(p) = req.headers().get("sec-websocket-protocol") {
                                let first = p
                                    .to_str()
                                    .unwrap_or("")
                                    .split(',')
                                    .next()
                                    .unwrap_or("")
                                    .trim()
                                    .to_string();
                                if !first.is_empty() {
                                    resp.headers_mut().insert(
                                        "sec-websocket-protocol",
                                        first.parse().unwrap(),
                                    );
                                }
                            }
                            Ok(resp)
                        };
                        let Ok(mut ws) =
                            tokio_tungstenite::accept_hdr_async(sock, cb).await
                        else {
                            return;
                        };
                        while let Some(Ok(msg)) = ws.next().await {
                            match msg {
                                TMessage::Text(t) => {
                                    let _ = ws.send(TMessage::Text(format!("echo:{t}"))).await;
                                    // An UNPROMPTED push right after — a server telling the page
                                    // something it never asked for is why this transport exists.
                                    let _ = ws
                                        .send(TMessage::Text("push:presence".to_string()))
                                        .await;
                                }
                                TMessage::Binary(b) => {
                                    let mut out = b.to_vec();
                                    out.reverse();
                                    let _ = ws.send(TMessage::Binary(out.into())).await;
                                }
                                // Deliberately NOT `return` here. tungstenite replies to a close
                                // frame from inside `next()`, so bailing out on the first Close
                                // drops the socket before the reply is flushed — which is what a
                                // crashed server looks like, and the client is right to report
                                // that as an error rather than a clean close. Keep polling and the
                                // loop ends on its own once the handshake completes.
                                TMessage::Close(_) => {}
                                _ => {}
                            }
                        }
                    });
                }
            });
            addr
        });

        rt.block_on(async {
            let mut ws = WebSocketConn::connect(
                &format!("ws://{addr}/live"),
                &["chat.v1".to_string(), "chat.v0".to_string()],
            )
            .await
            .expect("the handshake completed");

            assert_eq!(
                ws.protocol(),
                "chat.v1",
                "the SERVER's chosen subprotocol is reported — a client that offered two and got \
                 one back must not speak the other at it"
            );

            ws.send(WsMessage::Text("hello".into())).await.unwrap();
            assert_eq!(
                ws.recv().await.unwrap(),
                Some(WsMessage::Text("echo:hello".into())),
                "text round-trips"
            );
            assert_eq!(
                ws.recv().await.unwrap(),
                Some(WsMessage::Text("push:presence".into())),
                "G_WEBSOCKET: the server pushed a message the client never asked for. This is the \
                 capability polling cannot express, and the reason live chat/presence/log-tail need \
                 this transport at all."
            );

            ws.send(WsMessage::Binary(vec![1, 2, 3])).await.unwrap();
            assert_eq!(
                ws.recv().await.unwrap(),
                Some(WsMessage::Binary(vec![3, 2, 1])),
                "binary frames round-trip as bytes, not as lossy text"
            );

            ws.close().await.unwrap();
            assert_eq!(
                ws.recv().await.unwrap(),
                None,
                "after the close handshake the stream ends — a page's onclose fires instead of it \
                 waiting forever"
            );
        });
    }
}
