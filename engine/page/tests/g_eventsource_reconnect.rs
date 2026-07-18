//! G_EVENTSOURCE_RECONNECT — the live stream survives the connection dropping.
//!
//! **Reconnection is the defining feature of SSE, not a nicety.** The contract a page is written
//! against is "this stream stays alive": servers close idle connections, proxies time out, laptops
//! sleep. Tick 205 shipped SSE without it, so one blip ended the live updates permanently — the
//! ticker froze, the log tail stopped, and the page had no way to know it should care.
//!
//! Two claims, and the second is what separates a reconnect from a restart:
//!   1. when the stream ends, a NEW request is issued;
//!   2. it carries `Last-Event-ID`, so the server replays what was missed instead of the page
//!      silently losing every event during the gap.
//!
//! Plus the negative: a `204` (or any 4xx) is the server saying *stop*, and reconnecting into it
//! forever would be a self-inflicted DoS.

use manuk_page::FetchStreamEvent;
use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <div id="msgs"></div>
  <script>
    var es = new EventSource('/stream');
    es.onmessage = function(e) {
      document.getElementById('msgs').textContent += '[' + e.data + ']';
    };
  </script>
</body></html>"#;

fn deliver_frames(
    page: &mut manuk_page::Page,
    id: u32,
    body: &[u8],
    fonts: &manuk_text::FontContext,
) {
    page.deliver_fetch_stream(
        id,
        &FetchStreamEvent::Head {
            status: 200,
            headers: vec![("Content-Type".into(), "text/event-stream".into())],
        },
        fonts,
        800.0,
    );
    page.deliver_fetch_stream(id, &FetchStreamEvent::Chunk(body.to_vec()), fonts, 800.0);
}

#[test]
fn a_dropped_sse_stream_reconnects_and_resumes_from_the_last_event_id() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://live.test/", &fonts, 800.0);
    let root = page.dom().root();
    let msgs = manuk_css::query_selector_all(page.dom(), root, "#msgs")[0];

    // ── First connection: one frame with an id, then the server drops the stream. ────────────
    let reqs = page.take_fetches();
    assert_eq!(reqs.len(), 1, "the initial connection: {reqs:?}");
    let (id, _url, _m, headers, _b) = reqs[0].clone();
    assert!(
        !headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("last-event-id")),
        "the FIRST request has nothing to resume from: {headers:?}"
    );

    deliver_frames(&mut page, id, b"id: 42\nretry: 0\ndata: before\n\n", &fonts);
    assert_eq!(page.dom().text_content(msgs), "[before]");

    // The connection drops.
    page.deliver_fetch_stream(id, &FetchStreamEvent::End, &fonts, 800.0);

    // ── The reconnect. `retry: 0` was honoured, so the timer is due immediately. ─────────────
    let reqs = page.take_fetches();
    assert_eq!(
        reqs.len(),
        1,
        "G_EVENTSOURCE_RECONNECT: a dropped stream must RECONNECT. Without this, one blip ends the \
         live updates permanently: the ticker freezes and the page never knows. Got: {reqs:?}"
    );
    let (id2, url2, _m, headers2, _b) = reqs[0].clone();
    assert_eq!(url2, "/stream", "it reconnects to the same stream");
    assert!(
        headers2
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("last-event-id") && v == "42"),
        "G_EVENTSOURCE_RECONNECT: the reconnect must carry Last-Event-ID so the server REPLAYS what \
         was missed. Without it this is a restart, not a resume, and every event during the gap is \
         silently lost. Got: {headers2:?}"
    );

    // The resumed stream delivers into the same page state.
    deliver_frames(&mut page, id2, b"id: 43\ndata: after\n\n", &fonts);
    assert_eq!(
        page.dom().text_content(msgs),
        "[before][after]",
        "the resumed stream appends to what the page already had"
    );

    // ── A 204 means STOP. Reconnecting into it forever would be a self-inflicted DoS. ────────
    page.deliver_fetch_stream(id2, &FetchStreamEvent::End, &fonts, 800.0);
    let reqs = page.take_fetches();
    assert_eq!(reqs.len(), 1, "it reconnected again");
    let id3 = reqs[0].0;
    page.deliver_fetch_stream(
        id3,
        &FetchStreamEvent::Head {
            status: 204,
            headers: vec![],
        },
        &fonts,
        800.0,
    );
    page.deliver_fetch_stream(id3, &FetchStreamEvent::End, &fonts, 800.0);
    assert!(
        page.take_fetches().is_empty(),
        "G_EVENTSOURCE_RECONNECT: a 204 is the server saying STOP, and the client must not \
         reconnect into it. An endless retry loop against a server that already said no is a \
         self-inflicted DoS."
    );
}
