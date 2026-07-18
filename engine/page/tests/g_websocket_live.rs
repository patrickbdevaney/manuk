//! G_WEBSOCKET_LIVE — the whole stack, against a real server: page → transport → wire → page.
//!
//! `g_websocket` gates the page-facing half with a simulated host, and `manuk-net`'s own gate covers
//! the transport against a real server. Neither proves they COMPOSE — and the composition is where
//! the shell's wiring lives (`gui.rs::pump_websockets`), which has no UI harness to test through.
//!
//! So this gate does exactly what the shell does, in the same order, with a real socket in the
//! middle: drain the page's ops, connect a `WebSocketConn`, feed the events back into the page, and
//! assert the DOM. If the two halves disagree about anything — the op encoding, the byte
//! convention, the subprotocol, the close semantics — this fails and the unit gates do not.

use futures_util::{SinkExt, StreamExt};
use manuk_page::{WsEvent, WsOp};
use manuk_text::FontContext;
use tokio_tungstenite::tungstenite::protocol::Message as TMessage;

const HTML: &str = r#"<!doctype html>
<html><body>
  <div id="out">offline</div>
  <script>
    var ws = new WebSocket('/live', ['chat.v1']);
    ws.onopen = function() { ws.send('ping'); };
    ws.onmessage = function(e) {
      document.getElementById('out').textContent += '[' + e.data + ']';
    };
    ws.onclose = function(e) {
      document.getElementById('out').textContent += '(closed ' + e.code + ')';
    };
  </script>
</body></html>"#;

#[test]
fn the_page_talks_to_a_real_websocket_server_end_to_end() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // A real echo server that also pushes unprompted.
    let addr = rt.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            while let Ok((sock, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let cb = |req: &tokio_tungstenite::tungstenite::handshake::server::Request,
                              mut resp: tokio_tungstenite::tungstenite::handshake::server::Response| {
                        if let Some(p) = req.headers().get("sec-websocket-protocol") {
                            let first =
                                p.to_str().unwrap_or("").split(',').next().unwrap_or("").trim().to_string();
                            if !first.is_empty() {
                                resp.headers_mut()
                                    .insert("sec-websocket-protocol", first.parse().unwrap());
                            }
                        }
                        Ok(resp)
                    };
                    let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(sock, cb).await else {
                        return;
                    };
                    while let Some(Ok(msg)) = ws.next().await {
                        if let TMessage::Text(t) = msg {
                            let _ = ws.send(TMessage::Text(format!("pong:{t}"))).await;
                            let _ = ws.send(TMessage::Text("push".to_string())).await;
                            let _ = ws.close(None).await;
                        }
                    }
                });
            }
        });
        addr
    });

    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, &format!("http://{addr}/room"), &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];

    // ── What the shell does, step for step. ─────────────────────────────────────────────────
    let ops = page.take_ws_ops();
    assert_eq!(ops.len(), 1, "one connect op: {ops:?}");
    let (id, WsOp::Connect { url, protocols }) = ops.into_iter().next().unwrap() else {
        panic!("expected a Connect op");
    };
    // The page wrote `new WebSocket('/live')` — a relative URL the shell resolves against the doc.
    let abs = url::Url::parse(&format!("http://{addr}/room"))
        .unwrap()
        .join(&url)
        .unwrap();
    let ws_url = format!("ws://{}{}", abs.authority(), abs.path());

    rt.block_on(async {
        let mut conn = manuk_net::websocket::WebSocketConn::connect(&ws_url, &protocols)
            .await
            .expect("connected to the real server");

        page.deliver_ws_event(
            id,
            &WsEvent::Open {
                protocol: conn.protocol().to_string(),
                extensions: String::new(),
            },
            &fonts,
            800.0,
        );

        // onopen called send() — drain and put it on the real wire, exactly as the shell does.
        let ops = page.take_ws_ops();
        assert_eq!(
            ops,
            vec![(
                id,
                WsOp::Send {
                    data: b"ping".to_vec(),
                    binary: false
                }
            )],
            "the frame the page sent reached the host queue: {ops:?}"
        );
        conn.send(manuk_net::websocket::WsMessage::Text("ping".into()))
            .await
            .unwrap();

        // Pump the server's replies back into the page until it closes.
        loop {
            match conn.recv().await.expect("no transport error") {
                Some(manuk_net::websocket::WsMessage::Text(t)) => page.deliver_ws_event(
                    id,
                    &WsEvent::Message {
                        data: t.into_bytes(),
                        binary: false,
                    },
                    &fonts,
                    800.0,
                ),
                Some(manuk_net::websocket::WsMessage::Binary(b)) => page.deliver_ws_event(
                    id,
                    &WsEvent::Message {
                        data: b,
                        binary: true,
                    },
                    &fonts,
                    800.0,
                ),
                None => {
                    page.deliver_ws_event(
                        id,
                        &WsEvent::Close {
                            code: 1000,
                            reason: String::new(),
                            clean: true,
                        },
                        &fonts,
                        800.0,
                    );
                    break;
                }
            }
        }
    });

    assert_eq!(
        page.dom().text_content(out),
        "offline[pong:ping][push](closed 1000)",
        "G_WEBSOCKET_LIVE: the page's own frame went out over a real socket, the server's reply AND \
         its unprompted push both came back into onmessage and mutated the DOM, and the close was \
         reported. If the two halves disagreed about the op encoding, the byte convention or the \
         close semantics, this is where it shows."
    );
}
