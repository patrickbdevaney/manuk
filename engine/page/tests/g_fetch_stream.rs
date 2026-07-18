//! G_FETCH_STREAM — `response.body` is a real `ReadableStream`, so a streamed answer renders.
//!
//! **The failure this gate exists for.** `__makeResponse` hardcoded `body: null`, and
//! `globalThis.ReadableStream` was an INERT stub (an empty named constructor installed by the
//! `__inertNames` sweep, with no `getReader` on its prototype). So the canonical streaming read —
//!
//! ```js
//! const reader = (await fetch(url)).body.getReader();
//! ```
//!
//! — threw `TypeError: res.body is null` INSIDE the response handler, taking the rest of the handler
//! with it. That is not "the answer streams in slowly"; it is **the answer never appears at all**.
//! Every AI chat (claude.ai, ChatGPT, Gemini), every cloud console live-log tail and every inference
//! token stream ships this exact loop, so the whole class rendered blank.
//!
//! Note what a `typeof` check would have said: `typeof ReadableStream === 'function'` was ALREADY
//! true against the inert stub. This gate therefore asserts **behaviour** — a reader that actually
//! reads — which is the `g_globals` lesson (see `event_loop.rs`, the `__inertNames` comment).
//!
//! **Honest scope.** The body is delivered to JS fully-buffered (the host path
//! `manuk_net::request` → `NavEvent::PageFetch` → `deliver` carries one `String`), so this stream
//! yields its chunks from memory rather than off the wire. The *page's* code path is the real one —
//! reader loop, `done`, `TextDecoder`, SSE framing all execute exactly as written — but incremental
//! wire-level delivery is a separate subsystem and is logged as residue, NOT claimed here.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <div id="out">loading</div>
  <div id="answer"></div>
  <script>
    var r = [];
    var $ = function(id) { return document.getElementById(id); };

    fetch('/v1/chat').then(function(res) {
      // The read that used to be a TypeError.
      r.push('bodynotnull:' + (res.body !== null && typeof res.body.getReader === 'function'));
      r.push('locked0:' + (res.body.locked === false));

      var reader = res.body.getReader();
      r.push('locked1:' + (res.body.locked === true));

      var dec = new TextDecoder();
      var acc = '';
      var chunks = 0;
      var firstIsBytes = null;

      // The canonical pump loop, written the way every streaming client writes it.
      function pump() {
        return reader.read().then(function(step) {
          if (step.done) {
            r.push('donevalue:' + (step.value === undefined));
            return;
          }
          chunks++;
          if (firstIsBytes === null) firstIsBytes = (step.value instanceof Uint8Array);
          acc += dec.decode(step.value);
          return pump();
        });
      }

      return pump().then(function() {
        r.push('chunks:' + (chunks >= 1));
        r.push('bytes:' + firstIsBytes);
        r.push('bodyused:' + (res.bodyUsed === true));

        // Parse it as Server-Sent-Events framing — this is literally how an AI chat
        // reassembles its answer from the token stream.
        var text = '';
        acc.split('\n').forEach(function(line) {
          if (line.indexOf('data: ') !== 0) return;
          var payload = line.slice(6);
          if (payload === '[DONE]') return;
          try { text += JSON.parse(payload).delta; } catch (e) { r.push('parsefail:' + e); }
        });
        $('answer').textContent = text;
        r.push('sse:' + (text === 'Hello world'));

        // A second, independent response (clone) exercises the other consumption routes, so
        // nothing here is an unasserted claim.
        var c2 = res.clone();
        r.push('clonefresh:' + (c2.bodyUsed === false));

        // tee() — an AI SDK forks the token stream (one branch to the UI, one to a log).
        var branches = c2.body.tee();
        var readAll = function(stream) {
          var rd = stream.getReader(), s = '', d = new TextDecoder();
          var go = function() {
            return rd.read().then(function(step) {
              if (step.done) return s;
              s += d.decode(step.value);
              return go();
            });
          };
          return go();
        };
        return Promise.all([readAll(branches[0]), readAll(branches[1])]).then(function(both) {
          r.push('tee:' + (both[0] === both[1] && both[0].indexOf('Hello') !== -1));

          // arrayBuffer() over a third copy — byte length must match the UTF-8 encoding.
          var c3 = res.clone();
          return c3.arrayBuffer().then(function(ab) {
            r.push('ab:' + (ab.byteLength === new TextEncoder().encode(acc).length && ab.byteLength > 0));
            r.push('abused:' + (c3.bodyUsed === true));
            $('out').textContent = r.join(' ');
          });
        });
      });
    }).catch(function(e) {
      $('out').textContent = 'THREW:' + e;
    });
  </script>
</body></html>"#;

/// The SSE body a streaming chat endpoint actually sends back.
const SSE_BODY: &str = "data: {\"delta\":\"Hello\"}\n\
                        \n\
                        data: {\"delta\":\" world\"}\n\
                        \n\
                        data: [DONE]\n";

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn a_streamed_response_body_reads_through_a_real_readablestream() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://chat.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];

    assert_eq!(
        page.dom().text_content(out),
        "loading",
        "the fetch is still pending before the host resolves it"
    );

    let reqs = page.take_fetches();
    assert_eq!(reqs.len(), 1, "the page issued exactly one fetch");
    let (id, url, _method, _headers, _body) = &reqs[0];
    assert_eq!(url, "/v1/chat");

    let headers = vec![("Content-Type".to_string(), "text/event-stream".to_string())];
    page.resolve_fetch(*id, 200, SSE_BODY, &headers, &fonts, 800.0);

    let got = page.dom().text_content(out);
    assert!(
        !got.starts_with("THREW:"),
        "G_FETCH_STREAM: the streaming read threw — {got:?}\n  \
         `res.body.getReader()` is the first line of every streaming client on the web. When \
         `body` is null this throws INSIDE the response handler, so the answer never renders."
    );

    for claim in [
        "bodynotnull:true", // response.body is a real stream, not null
        "locked0:true",     // .locked reports false before a reader is taken
        "locked1:true",     // ...and true after
        "chunks:true",      // the pump loop actually received data
        "bytes:true",       // chunks arrive as Uint8Array, so TextDecoder works on them
        "donevalue:true",   // the final read is {done:true, value:undefined}
        "bodyused:true",    // reading the body flips bodyUsed
        "sse:true",         // SSE framing reassembles into the answer
        "clonefresh:true",  // clone() yields an unconsumed body
        "tee:true",         // tee() mirrors every chunk into both branches
        "ab:true",          // arrayBuffer() returns the body's UTF-8 bytes
        "abused:true",      // ...and flips bodyUsed too
    ] {
        assert!(
            got.contains(claim),
            "G_FETCH_STREAM: expected {claim} in {got:?}"
        );
    }

    // The reassembled answer reached the DOM — the user-visible half.
    let answer = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#answer")[0];
    assert_eq!(
        page.dom().text_content(answer),
        "Hello world",
        "the streamed answer rendered into the page"
    );
}
