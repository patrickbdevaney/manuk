//! E4 — the WebSocket transport for the BiDi remote end.
//!
//! Deliberately thin: it frames JSON text messages, hands each decoded [`Command`] to
//! [`Session::dispatch`], and writes back the reply followed by any events the command
//! produced. All protocol behavior lives in [`crate::protocol`], which is why the
//! protocol is testable without a socket.
//!
//! **Scope:** one client per session (a BiDi session *is* a client connection). A
//! malformed frame yields a protocol error with `id: 0` rather than dropping the
//! connection, matching how real remote ends stay usable after a client bug.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use crate::protocol::{BidiError, Command, ErrorCode, Outgoing, Session};

/// Serve BiDi on `addr` until the listener is dropped. Returns the bound address, so
/// callers (and tests) can bind port 0 and learn the real port.
pub async fn bind(addr: &str) -> Result<(TcpListener, std::net::SocketAddr)> {
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding BiDi remote end to {addr}"))?;
    let local = listener.local_addr()?;
    Ok((listener, local))
}

/// Accept connections forever, giving each its own [`Session`].
///
/// Each connection runs on its **own thread with a current-thread runtime**, not a
/// `tokio::spawn`. The page pipeline is `!Send` (`manuk_text::FontContext` holds
/// `Rc<fontdue::Font>`), so a session's future cannot migrate between worker threads.
/// A thread per connection keeps real concurrency across clients while each session
/// stays pinned to one thread. The `TcpStream` is handed over as a `std` socket so it
/// registers with the new thread's reactor rather than the accepting one.
pub async fn serve(listener: TcpListener, width: u32, height: u32) -> Result<()> {
    loop {
        let (stream, peer) = listener.accept().await?;
        let std_stream = stream.into_std().context("detaching accepted socket")?;
        std::thread::Builder::new()
            .name(format!("bidi-{peer}"))
            .spawn(move || {
                let run = || -> Result<()> {
                    std_stream
                        .set_nonblocking(true)
                        .context("setting socket nonblocking")?;
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .context("building per-connection runtime")?;
                    rt.block_on(async move {
                        let stream = TcpStream::from_std(std_stream)
                            .context("re-registering socket on the session reactor")?;
                        handle_connection(stream, width, height).await
                    })
                };
                if let Err(e) = run() {
                    tracing::warn!(%peer, error = %format!("{e:#}"), "BiDi connection ended with an error");
                }
            })
            .context("spawning BiDi session thread")?;
    }
}

/// Drive one client connection to completion.
pub async fn handle_connection(stream: TcpStream, width: u32, height: u32) -> Result<()> {
    let ws = tokio_tungstenite::accept_async(stream)
        .await
        .context("WebSocket handshake failed")?;
    let (mut tx, mut rx) = ws.split();
    let mut session = Session::new(width, height);

    while let Some(msg) = rx.next().await {
        let text = match msg? {
            Message::Text(t) => t,
            Message::Binary(_) => {
                // BiDi is a JSON text protocol.
                let e = BidiError::new(ErrorCode::InvalidArgument, "expected a text frame");
                tx.send(Message::Text(Outgoing::error(0, &e).to_json())).await?;
                continue;
            }
            Message::Close(_) => break,
            // tungstenite answers Ping itself; nothing to do for Pong/Frame.
            _ => continue,
        };

        let cmd: Command = match serde_json::from_str(&text) {
            Ok(c) => c,
            Err(err) => {
                // A malformed frame must not kill the connection.
                let e = BidiError::new(
                    ErrorCode::InvalidArgument,
                    format!("could not decode command: {err}"),
                );
                tx.send(Message::Text(Outgoing::error(0, &e).to_json())).await?;
                continue;
            }
        };

        tracing::debug!(id = cmd.id, method = %cmd.method, params = %cmd.params, "bidi <-");
        let dispatched = session.dispatch(cmd).await;
        // Events the spec orders BEFORE the reply (e.g. contextCreated) go first.
        for ev in dispatched.before {
            tracing::debug!(event = %ev.to_json(), "bidi -> (pre)");
            tx.send(Message::Text(ev.to_json())).await?;
        }
        tracing::debug!(reply = %dispatched.reply.to_json(), "bidi ->");
        tx.send(Message::Text(dispatched.reply.to_json())).await?;
        for ev in dispatched.events {
            tx.send(Message::Text(ev.to_json())).await?;
        }
    }
    Ok(())
}
