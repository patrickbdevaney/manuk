//! G_AGENT_WEBSOCKET_SLOW — the agent keeps servicing a socket that owes the page a reply.
//!
//! Wolfram|Alpha answers `newQuery` ~1.4 s later; a pump that called the socket finished after 400 ms of
//! silence left the pods empty. The server here replies after 1.8 s.
//!
//! The GUI shell pumps `take_ws_ops`/`deliver_ws_event`; the agent (and so manuk-mcp) did not, so a
//! page that gets its content over a socket (Wolfram|Alpha's result pods) stayed empty. This serves
//! a page that opens a socket to a real local server and renders what comes back, then asserts
//! `navigate` alone leaves the server's reply in the extracted text.

use futures_util::{SinkExt, StreamExt};
use manuk_agent::AgentBrowser;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::protocol::Message as TMessage;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn navigate_waits_for_a_slow_websocket_reply() {
    let ws_l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ws_port = ws_l.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((sock, _)) = ws_l.accept().await {
            tokio::spawn(async move {
                let Ok(mut ws) = tokio_tungstenite::accept_async(sock).await else {
                    return;
                };
                while let Some(Ok(TMessage::Text(t))) = ws.next().await {
                    tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
                    let _ = ws.send(TMessage::Text(format!("SLOW-{t}-33038"))).await;
                }
            });
        }
    });
    let html = format!(
        r#"<!doctype html><html><body><p id="out">waiting</p><script>
        var ws = new WebSocket('ws://127.0.0.1:{ws_port}/r');
        ws.onopen = function() {{ ws.send('hello'); }};
        ws.onmessage = function(e) {{ document.getElementById('out').textContent += ' [' + e.data + ']'; }};
        </script></body></html>"#
    );
    let http_l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let http_port = http_l.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = http_l.accept().await {
            let html = html.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    html.len(),
                    html
                );
                let _ = s.write_all(resp.as_bytes()).await;
            });
        }
    });

    let mut b = AgentBrowser::new(800, 600);
    b.navigate(&format!("http://127.0.0.1:{http_port}/"))
        .await
        .unwrap();
    let text = b.observe().unwrap().text;
    assert!(
        text.contains("[SLOW-hello-33038]"),
        "G_AGENT_WEBSOCKET_SLOW: the page's frame went out and the server's reply and push came back into \
         onmessage; got {text:?}"
    );
}
