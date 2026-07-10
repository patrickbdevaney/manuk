//! E4 acceptance — a real WebSocket client connects to the remote end and drives a
//! page. This exercises the transport (handshake, JSON text frames, ordering of
//! reply-then-events), not just the dispatcher.
//!
//! Puppeteer itself is not run here (no Node runtime in this environment), so this is
//! the strongest honest check available: a standards-shaped BiDi client session over
//! the wire. See CLAUDE.md § E4 for that documented gap.

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

/// Start the remote end on an ephemeral port and return its `ws://` URL.
async fn start() -> String {
    let (listener, addr) = manuk_bidi::server::bind("127.0.0.1:0").await.unwrap();
    tokio::spawn(async move {
        let _ = manuk_bidi::server::serve(listener, 800, 600).await;
    });
    format!("ws://{addr}")
}

struct Client {
    ws: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    next_id: u64,
}

impl Client {
    async fn connect(url: &str) -> Self {
        let (ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        Client { ws, next_id: 1 }
    }

    /// Send a command and read exactly one frame back (the reply).
    async fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({"id": id, "method": method, "params": params});
        self.ws.send(Message::Text(msg.to_string())).await.unwrap();
        let reply = self.recv().await;
        assert_eq!(reply["id"], id, "reply id must echo the command id");
        reply
    }

    async fn recv(&mut self) -> Value {
        loop {
            match self.ws.next().await.expect("stream ended").unwrap() {
                Message::Text(t) => return serde_json::from_str(&t).unwrap(),
                Message::Close(_) => panic!("closed"),
                _ => continue,
            }
        }
    }

    async fn send_raw(&mut self, raw: &str) -> Value {
        self.ws.send(Message::Text(raw.to_string())).await.unwrap();
        self.recv().await
    }
}

fn data_url(html: &str) -> String {
    format!("data:text/html,{html}")
}

#[tokio::test]
async fn a_websocket_client_drives_a_full_bidi_session() {
    let url = start().await;
    let mut c = Client::connect(&url).await;

    // status works before a session exists
    let r = c.call("session.status", json!({})).await;
    assert_eq!(r["type"], "success");
    assert_eq!(r["result"]["ready"], true);

    // session.new
    let r = c.call("session.new", json!({})).await;
    assert_eq!(r["result"]["capabilities"]["browserName"], "manuk");

    // the session came with one context
    let r = c.call("browsingContext.getTree", json!({})).await;
    let ctx = r["result"]["contexts"][0]["context"].as_str().unwrap().to_string();

    // subscribe, then navigate: reply first, then the load event
    c.call("session.subscribe", json!({"events": ["browsingContext.load"]}))
        .await;

    let page = data_url("<title>Hello</title><body><h1>Hi there</h1></body>");
    let r = c
        .call("browsingContext.navigate", json!({"context": ctx, "url": page}))
        .await;
    assert_eq!(r["result"]["url"], page);

    let ev = c.recv().await;
    assert_eq!(ev["type"], "event");
    assert_eq!(ev["method"], "browsingContext.load");
    assert_eq!(ev["params"]["context"], ctx);

    // the tree reflects the navigation
    let r = c.call("browsingContext.getTree", json!({})).await;
    assert_eq!(r["result"]["contexts"][0]["url"], page);

    // screenshot is a real PNG
    let r = c
        .call("browsingContext.captureScreenshot", json!({"context": ctx}))
        .await;
    let b64 = r["result"]["data"].as_str().unwrap();
    use base64::Engine as _;
    let png = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");

    // a second context, then close it
    let r = c.call("browsingContext.create", json!({"type": "tab"})).await;
    let ctx2 = r["result"]["context"].as_str().unwrap().to_string();
    let r = c.call("browsingContext.getTree", json!({})).await;
    assert_eq!(r["result"]["contexts"].as_array().unwrap().len(), 2);
    c.call("browsingContext.close", json!({"context": ctx2})).await;
    let r = c.call("browsingContext.getTree", json!({})).await;
    assert_eq!(r["result"]["contexts"].as_array().unwrap().len(), 1);
}

/// A malformed frame must produce a protocol error, not kill the connection — a real
/// remote end has to survive a buggy client.
#[tokio::test]
async fn a_malformed_frame_errors_without_dropping_the_connection() {
    let url = start().await;
    let mut c = Client::connect(&url).await;

    let r = c.send_raw("{not json").await;
    assert_eq!(r["type"], "error");
    assert_eq!(r["error"], "invalid argument");
    assert_eq!(r["id"], 0);

    // The connection is still usable.
    let r = c.call("session.status", json!({})).await;
    assert_eq!(r["result"]["ready"], true);
}

/// Two clients get independent sessions (independent contexts), which is what a
/// thread-per-connection remote end must guarantee.
#[tokio::test]
async fn two_clients_get_independent_sessions() {
    let url = start().await;
    let mut a = Client::connect(&url).await;
    let mut b = Client::connect(&url).await;

    a.call("session.new", json!({})).await;
    b.call("session.new", json!({})).await;

    let ctx_a = {
        let r = a.call("browsingContext.getTree", json!({})).await;
        r["result"]["contexts"][0]["context"].as_str().unwrap().to_string()
    };
    a.call(
        "browsingContext.navigate",
        json!({"context": ctx_a, "url": data_url("<body>AAA</body>")}),
    )
    .await;

    // b's context is untouched by a's navigation.
    let r = b.call("browsingContext.getTree", json!({})).await;
    assert_eq!(r["result"]["contexts"][0]["url"], "about:blank");
}
