//! G_WEBSOCKET — a live chat receives a message the page never asked for, and it appears.
//!
//! **The failure this gate exists for.** `WebSocket` was an *honest stub*: it constructed, sat in
//! CONNECTING, then fired `error` + `close`. That was a deliberate improvement over a
//! `ReferenceError` (which wiped aljazeera.com's article — see the comment in `event_loop.rs`), but
//! it means every live-blog, DM thread, presence indicator and console log-tail connected, failed,
//! and rendered nothing. `send()` threw unconditionally, because the socket was never open.
//!
//! This gate drives the real path: the page queues a connect, the host reports the handshake, and
//! then the SERVER pushes — which is the capability polling cannot express and the whole reason the
//! transport exists. Each assertion is made at a point where the next event has not happened yet.
//!
//! The transport itself (`manuk_net::websocket`) is gated separately against a real server in
//! `manuk-net`; this gate is the page-facing half — ops out, events in, DOM updated.

use manuk_js::{WsEvent, WsOp};
use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <div id="status">offline</div>
  <div id="log"></div>
  <div id="bin"></div>
  <script>
    var $ = function(id) { return document.getElementById(id); };
    var ws = new WebSocket('wss://chat.test/room/42', ['chat.v1', 'chat.v0']);
    ws.binaryType = 'arraybuffer';

    // A client that sends before OPEN must get InvalidStateError — clients are written for it.
    try { ws.send('too early'); $('status').textContent = 'NO-THROW'; }
    catch (e) { $('status').textContent = 'early:' + e.name; }

    ws.onopen = function() {
      $('status').textContent = 'open:' + ws.protocol + ':' + ws.readyState;
      ws.send('hello');
    };
    ws.onmessage = function(e) {
      if (typeof e.data === 'string') {
        $('log').textContent += '[' + e.data + ']';
      } else {
        var b = new Uint8Array(e.data);
        $('bin').textContent = 'bytes:' + b.length + ':' + b[0] + ',' + b[b.length - 1];
      }
    };
    ws.onclose = function(e) {
      $('status').textContent = 'closed:' + e.code + ':' + e.wasClean + ':' + ws.readyState;
    };
  </script>
</body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn a_live_chat_connects_sends_and_receives_a_server_push() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://chat.test/", &fonts, 800.0);
    let root = page.dom().root();
    let status = manuk_css::query_selector_all(page.dom(), root, "#status")[0];
    let log = manuk_css::query_selector_all(page.dom(), root, "#log")[0];
    let bin = manuk_css::query_selector_all(page.dom(), root, "#bin")[0];

    // ── The page asked to connect, and said which subprotocols it speaks. ────────────────────
    let ops = page.take_ws_ops();
    assert_eq!(ops.len(), 1, "exactly one connect op was queued: {ops:?}");
    let (id, op) = &ops[0];
    assert_eq!(
        op,
        &WsOp::Connect {
            url: "wss://chat.test/room/42".to_string(),
            protocols: vec!["chat.v1".to_string(), "chat.v0".to_string()],
        },
        "the URL and the OFFERED subprotocols reach the host — a server cannot select from a list \
         it was never sent"
    );
    let id = *id;

    assert_eq!(
        page.dom().text_content(status),
        "early:InvalidStateError",
        "G_WEBSOCKET: send() before OPEN must throw InvalidStateError. Clients are written for \
         this; silently swallowing it loses the frame with no way to know."
    );

    // ── Handshake. Nothing has been received yet. ────────────────────────────────────────────
    page.deliver_ws_event(
        id,
        &WsEvent::Open {
            protocol: "chat.v1".to_string(),
            extensions: String::new(),
        },
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(status),
        "open:chat.v1:1",
        "onopen fires with the SERVER's chosen subprotocol and readyState OPEN(1)"
    );

    // onopen sent a frame — it must have reached the host queue.
    let ops = page.take_ws_ops();
    assert_eq!(
        ops,
        vec![(
            id,
            WsOp::Send {
                data: b"hello".to_vec(),
                binary: false
            }
        )],
        "the frame the page sent from onopen reached the host: {ops:?}"
    );

    // ── THE SERVER PUSHES. The page never asked for this. ────────────────────────────────────
    assert_eq!(
        page.dom().text_content(log),
        "",
        "nothing has been pushed yet"
    );
    page.deliver_ws_event(
        id,
        &WsEvent::Message {
            data: b"ada joined".to_vec(),
            binary: false,
        },
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(log),
        "[ada joined]",
        "G_WEBSOCKET: an unprompted server push reached the page's onmessage and mutated the DOM. \
         This is the capability polling cannot express, and the entire reason live chat, presence \
         and log-tails need this transport."
    );

    page.deliver_ws_event(
        id,
        &WsEvent::Message {
            data: b"bob joined".to_vec(),
            binary: false,
        },
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(log),
        "[ada joined][bob joined]",
        "a second push appends — the socket stays open across messages"
    );

    // ── A binary frame honours binaryType. ───────────────────────────────────────────────────
    page.deliver_ws_event(
        id,
        &WsEvent::Message {
            data: vec![0xff, 0x00, 0x7f],
            binary: true,
        },
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(bin),
        "bytes:3:255,127",
        "binaryType='arraybuffer' yields an ArrayBuffer of the exact bytes — 0xFF survives, which \
         a UTF-8 round-trip would have destroyed"
    );

    // ── Close. ───────────────────────────────────────────────────────────────────────────────
    page.deliver_ws_event(
        id,
        &WsEvent::Close {
            code: 1000,
            reason: "bye".to_string(),
            clean: true,
        },
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(status),
        "closed:1000:true:3",
        "onclose reports the code, a CLEAN close, and readyState CLOSED(3) — a reconnect loop \
         backs off on wasClean=false and must not see it here"
    );
}
