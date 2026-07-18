//! G_EVENTSOURCE — a live-updates page receives its stream, frame by frame.
//!
//! **The failure this gate exists for.** `EventSource` constructed and then reported that it could
//! not connect — honest, and better than throwing, but it left every live-updates page dead: score
//! tickers, CI/deploy log tails, notification streams, dashboard metrics, and the many AI chats that
//! use SSE rather than fetch-streaming.
//!
//! It is now built on our own `fetch` (ticks 196-198 made `response.body` a real ReadableStream fed
//! incrementally off the wire), so this gate also proves that spine carries a second consumer.
//!
//! Every assertion is made at a point where the later frames have not been delivered yet, so a
//! buffered implementation cannot pass by construction.

use manuk_page::FetchStreamEvent;
use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <div id="state">init</div>
  <div id="msgs"></div>
  <div id="named"></div>
  <script>
    var $ = function(id) { return document.getElementById(id); };
    var es = new EventSource('/stream');
    es.onopen = function() { $('state').textContent = 'open:' + es.readyState; };
    es.onmessage = function(e) {
      $('msgs').textContent += '[' + e.data + '/' + e.lastEventId + ']';
    };
    // A NAMED event — `event: deploy` must not arrive on onmessage.
    es.addEventListener('deploy', function(e) {
      $('named').textContent += '{' + e.data + '}';
    });
  </script>
</body></html>"#;

#[test]
fn a_server_sent_event_stream_delivers_frames_as_they_arrive() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://live.test/", &fonts, 800.0);
    let root = page.dom().root();
    let state = manuk_css::query_selector_all(page.dom(), root, "#state")[0];
    let msgs = manuk_css::query_selector_all(page.dom(), root, "#msgs")[0];
    let named = manuk_css::query_selector_all(page.dom(), root, "#named")[0];

    // The EventSource issued a real fetch, with the SSE Accept header.
    let reqs = page.take_fetches();
    assert_eq!(reqs.len(), 1, "EventSource opened one request: {reqs:?}");
    let (id, url, _method, headers, _body) = reqs[0].clone();
    assert_eq!(url, "/stream");
    assert!(
        headers
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("accept") && v.contains("text/event-stream")),
        "the request advertises SSE — a server content-negotiates on it: {headers:?}"
    );

    page.deliver_fetch_stream(
        id,
        &FetchStreamEvent::Head {
            status: 200,
            headers: vec![("Content-Type".to_string(), "text/event-stream".to_string())],
        },
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(state),
        "open:1",
        "onopen fires at the headers with readyState OPEN(1)"
    );

    // ── One complete frame, plus the start of another. The partial must NOT dispatch. ────────
    page.deliver_fetch_stream(
        id,
        &FetchStreamEvent::Chunk(b"id: 1\ndata: first\n\ndata: par".to_vec()),
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(msgs),
        "[first/1]",
        "G_EVENTSOURCE: the complete frame dispatched and the PARTIAL one did not. A frame is \
         terminated by a blank line; dispatching on chunk boundaries would deliver half a message."
    );

    // Completing the partial frame dispatches it — and `id` persists as lastEventId.
    page.deliver_fetch_stream(
        id,
        &FetchStreamEvent::Chunk(b"tial\n\n".to_vec()),
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(msgs),
        "[first/1][partial/1]",
        "the frame split across two chunks reassembled, and lastEventId carried over"
    );

    // ── A named event, a multi-line payload, and a comment keepalive. ────────────────────────
    page.deliver_fetch_stream(
        id,
        &FetchStreamEvent::Chunk(
            b": keepalive\n\nevent: deploy\nid: 7\ndata: line one\ndata: line two\n\n".to_vec(),
        ),
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(named),
        "{line one\nline two}",
        "a named event reaches its own listener, and multiple data: lines join with a newline as \
         ONE message rather than arriving as two"
    );
    assert_eq!(
        page.dom().text_content(msgs),
        "[first/1][partial/1]",
        "the named event did NOT also arrive on onmessage, and the comment keepalive dispatched \
         nothing at all"
    );
}
