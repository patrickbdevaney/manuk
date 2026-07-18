//! G_XHR_PROGRESS — an XHR reports progress instead of jumping from "sent" to "done".
//!
//! **The failure this gate exists for.** The streaming delivery path built in ticks 197-198 only
//! knew about `fetch`; `__deliverHead` bailed out on an XHR id (a documented residue). So an XHR
//! still received its whole body in one delivery: `readyState` went 1 → 4, `onprogress` never fired,
//! and `responseText` was empty right up until it was complete. A download progress bar therefore
//! showed nothing and then 100% — the transfer appeared to take zero time.
//!
//! `readyState 3` (LOADING) with a growing `responseText` is the whole mechanism, and it is what
//! every progress bar and pre-`fetch`-era streaming client reads.
//!
//! Each assertion is made at a point where the rest of the body has not been delivered yet, so a
//! buffered implementation cannot pass by construction.

use manuk_page::FetchStreamEvent;
use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <div id="states"></div>
  <div id="seen"></div>
  <div id="done"></div>
  <script>
    var $ = function(id) { return document.getElementById(id); };
    var x = new XMLHttpRequest();
    x.open('GET', '/big');
    x.onreadystatechange = function() {
      $('states').textContent += x.readyState;
      // Record what the partial body looked like AT readyState 3.
      if (x.readyState === 3) { $('seen').textContent += '[' + x.responseText + ']'; }
    };
    x.onprogress = function(e) { $('seen').textContent += '(' + e.loaded + ')'; };
    x.onload = function() { $('done').textContent = 'load:' + x.responseText; };
    x.send();
  </script>
</body></html>"#;

#[test]
fn an_xhr_reports_readystate_3_with_a_growing_body() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://dl.test/", &fonts, 800.0);
    let root = page.dom().root();
    let states = manuk_css::query_selector_all(page.dom(), root, "#states")[0];
    let seen = manuk_css::query_selector_all(page.dom(), root, "#seen")[0];
    let done = manuk_css::query_selector_all(page.dom(), root, "#done")[0];

    let reqs = page.take_fetches();
    assert_eq!(reqs.len(), 1, "the XHR was queued: {reqs:?}");
    let id = reqs[0].0;

    // ── Headers: HEADERS_RECEIVED(2), and nothing has been downloaded yet. ──────────────────
    page.deliver_fetch_stream(
        id,
        &FetchStreamEvent::Head {
            status: 200,
            headers: vec![],
        },
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(states),
        "2",
        "the headers put the XHR in HEADERS_RECEIVED(2) — not straight to DONE"
    );

    // ── First chunk: LOADING(3), with a PARTIAL body readable. ──────────────────────────────
    page.deliver_fetch_stream(
        id,
        &FetchStreamEvent::Chunk(b"hello ".to_vec()),
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(states),
        "23",
        "the first chunk moves the XHR to LOADING(3)"
    );
    assert_eq!(
        page.dom().text_content(seen),
        "[hello ](6)",
        "G_XHR_PROGRESS: at readyState 3 the page can read the PARTIAL body, and onprogress \
         reports how much has arrived. The rest of the response does not exist yet, so a buffered \
         implementation cannot produce this."
    );

    // ── Second chunk: still LOADING, body has grown. ────────────────────────────────────────
    page.deliver_fetch_stream(
        id,
        &FetchStreamEvent::Chunk(b"world".to_vec()),
        &fonts,
        800.0,
    );
    assert_eq!(
        page.dom().text_content(states),
        "233",
        "each chunk fires readystatechange again at LOADING"
    );
    assert_eq!(
        page.dom().text_content(seen),
        "[hello ](6)[hello world](11)",
        "responseText GROWS across chunks — that is what a progress bar reads"
    );
    assert_eq!(
        page.dom().text_content(done),
        "",
        "onload has not fired: the body is not finished"
    );

    // ── End: DONE(4) and onload. ────────────────────────────────────────────────────────────
    page.deliver_fetch_stream(id, &FetchStreamEvent::End, &fonts, 800.0);
    assert_eq!(
        page.dom().text_content(states),
        "2334",
        "the full lifecycle is 2 → 3 → 3 → 4, not 1 → 4"
    );
    assert_eq!(
        page.dom().text_content(done),
        "load:hello world",
        "onload fires once with the complete body"
    );
}
