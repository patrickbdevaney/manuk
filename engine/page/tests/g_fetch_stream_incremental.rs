//! G_FETCH_STREAM_INCREMENTAL — the answer TYPES ITSELF OUT; it does not appear in one lump.
//!
//! `g_fetch_stream` proved the page can *read* a `response.body` stream. It could still only be fed
//! the whole body at once, because [`Page::resolve_fetch`] settles a request with a complete
//! `String` — so a streamed answer appeared only when the server had finished. That is the
//! difference between "the AI answer renders" and "the AI answer streams".
//!
//! This gate drives the incremental path: `Head` (where `fetch()`'s promise resolves, body still
//! arriving) → `Chunk` → `Chunk` → `End`, asserting the DOM **between** the chunks. Each assertion
//! is made at a point in time when the rest of the body does not exist yet, so a buffered
//! implementation cannot pass it — the body it would need has not been produced.
//!
//! The bar, stated as a falsifiable claim: **after the first chunk and before the second, the page
//! shows the first token and only the first token.**

use manuk_js::FetchStreamEvent;
use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <div id="answer"></div>
  <div id="log"></div>
  <script>
    var $ = function(id) { return document.getElementById(id); };
    var log = [];

    fetch('/v1/chat').then(function(res) {
      // The promise resolved AT THE HEADERS — the body is still on its way.
      log.push('head:' + res.status);
      $('log').textContent = log.join(' ');

      var reader = res.body.getReader();
      var dec = new TextDecoder();
      var pending = '';

      function pump() {
        return reader.read().then(function(step) {
          if (step.done) {
            log.push('done');
            $('log').textContent = log.join(' ');
            return;
          }
          // `{stream:true}` — a chunk boundary can split a multi-byte character, so the decoder
          // must hold the incomplete tail back for the next chunk. Every streaming client on the
          // web passes this flag.
          pending += dec.decode(step.value, { stream: true });

          // Consume whole SSE frames as they complete, appending each token to the page.
          var frames = pending.split('\n');
          pending = frames.pop();
          frames.forEach(function(line) {
            if (line.indexOf('data: ') !== 0) return;
            var payload = line.slice(6);
            if (payload === '[DONE]') return;
            try { $('answer').textContent += JSON.parse(payload).delta; } catch (e) {}
          });
          log.push('chunk');
          $('log').textContent = log.join(' ');
          return pump();
        });
      }
      return pump();
    });
  </script>
</body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`); a second
/// `Page` here segfaulted on teardown, so the multi-byte-split claim rides the same stream below.
#[test]
fn a_streamed_answer_renders_between_chunks_not_only_at_the_end() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://chat.test/", &fonts, 800.0);
    let root = page.dom().root();
    let answer = manuk_css::query_selector_all(page.dom(), root, "#answer")[0];
    let log = manuk_css::query_selector_all(page.dom(), root, "#log")[0];

    let reqs = page.take_fetches();
    assert_eq!(reqs.len(), 1, "the page issued exactly one fetch");
    let id = reqs[0].0;

    // ── Head: the promise resolves here, with NO body delivered at all. ──────────────────────
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
        page.dom().text_content(log),
        "head:200",
        "G_FETCH_STREAM_INCREMENTAL: fetch() must resolve at the RESPONSE HEADERS, not at the end \
         of the body — that is when a real fetch() resolves, and it is what lets the page take a \
         reader and start pumping while the rest is still on the wire"
    );
    assert_eq!(
        page.dom().text_content(answer),
        "",
        "nothing is rendered before any body byte has been delivered"
    );

    // ── Chunk 1. The second token DOES NOT EXIST YET — no buffered path can pass this. ───────
    page.deliver_fetch_stream(
        id,
        &FetchStreamEvent::Chunk(b"data: {\"delta\":\"Hello\"}\n".to_vec()),
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(answer),
        "Hello",
        "G_FETCH_STREAM_INCREMENTAL: after the FIRST chunk the page must already show the first \
         token. This is the whole claim: the answer types itself out. A buffered delivery cannot \
         satisfy it, because the rest of the body has not been produced yet."
    );
    assert_eq!(page.dom().text_content(log), "head:200 chunk");

    // ── Chunk 2 — appended to what is already on screen. ─────────────────────────────────────
    page.deliver_fetch_stream(
        id,
        &FetchStreamEvent::Chunk(b"data: {\"delta\":\" world\"}\n".to_vec()),
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(answer),
        "Hello world",
        "the second chunk appends to the first"
    );

    // ── A chunk boundary that SPLITS A MULTI-BYTE CHARACTER must not corrupt it. ─────────────
    // This is the normal case for a byte stream, not an edge case: "café"'s 'é' is two bytes
    // (0xC3 0xA9) and the split falls between them. It is why chunks cross the Rust↔JS boundary as
    // bytes rather than as a lossily-decoded String, which would substitute U+FFFD.
    let frame = "data: {\"delta\":\"café\"}\n".as_bytes().to_vec();
    let split = frame.iter().position(|&b| b == 0xC3).unwrap() + 1;
    page.deliver_fetch_stream(
        id,
        &FetchStreamEvent::Chunk(frame[..split].to_vec()),
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(answer),
        "Hello world",
        "a half-delivered frame renders nothing new — the page appends whole frames only"
    );
    page.deliver_fetch_stream(
        id,
        &FetchStreamEvent::Chunk(frame[split..].to_vec()),
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(answer),
        "Hello worldcafé",
        "G_FETCH_STREAM_INCREMENTAL: a character split across two chunks must survive reassembly. \
         Lossily decoding chunk bytes to a String en route would replace it with U+FFFD."
    );

    // ── End: the pump loop sees {done: true}. ────────────────────────────────────────────────
    page.deliver_fetch_stream(id, &FetchStreamEvent::End, &fonts, 800.0);
    assert_eq!(
        page.dom().text_content(log),
        "head:200 chunk chunk chunk chunk done",
        "End closes the stream so the pump loop terminates — a reader that never sees `done` is a \
         page that spins forever waiting for an answer it already has"
    );
    assert_eq!(page.dom().text_content(answer), "Hello worldcafé");
}
