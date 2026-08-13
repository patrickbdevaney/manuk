//! **G_XHR_EVENTTARGET (streaming half) — the delivery path that DID fire `loadend`.**
//!
//! `G_XHR_EVENTTARGET_STREAM` — **the filename-derived name, stated here on purpose.** A gate has TWO
//! names, its FILE and the one its own first line declares (`G_XHR_EVENTTARGET`), and until tick 1203
//! this file declared only the second while `CONSTELLATION.tsv` cited only the first. Each
//! instrument was then blind to exactly the gates the other could see, and a real, passing,
//! shipped gate read as a PHANTOM to the reconciler. Both dialects now appear in the file, so
//! either reader validates the citation. (Surface audit #60; the same shape as audit #36's
//! case dialect, with a different pair.)
//!
//!
//! Split into its own file because a `PageContext` is per-PROCESS: two `#[test]`s that each build a
//! page in one binary SIGSEGV. (Learned the hard way often enough to be a standing rule; it cost this
//! tick one run.)
//!
//! Both paths are gated because they had DIVERGED — `loadend` was fired here and not by the buffered
//! `__deliverXhr`, so whether `onloadend` ran depended on whether the response happened to arrive in
//! chunks. A gate on one path calls that divergence green. See `g_xhr_eventtarget.rs`.

use manuk_page::FetchStreamEvent;
use manuk_text::FontContext;

/// The STREAMING delivery path — the one that DID fire `loadend`. Both are asserted because the two
/// had drifted apart, and a gate that covered only one would have called that drift green.
#[test]
fn xhr_fires_the_same_events_on_the_streaming_path() {
    const H: &str = r#"<!doctype html><html><body><div id="out">-</div>
<script>
  var seen = [];
  var x = new XMLHttpRequest();
  ['readystatechange','progress','load','loadend','error'].forEach(function (t) {
    x.addEventListener(t, function () { seen.push(t); });
  });
  x.open('GET', '/stream');
  x.send();
  globalThis.__report = function () {
    document.getElementById('out').textContent = 'seen:' + seen.join(',');
  };
</script></body></html>"#;
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(H, "https://x.test/", &fonts, 800.0);
    let reqs = page.take_fetches();
    assert_eq!(reqs.len(), 1, "the XHR was queued: {reqs:?}");
    let id = reqs[0].0;
    for ev in [
        FetchStreamEvent::Head {
            status: 200,
            headers: vec![],
        },
        FetchStreamEvent::Chunk("ab".into()),
        FetchStreamEvent::End,
    ] {
        page.deliver_fetch_stream(id, &ev, &fonts, 800.0);
    }
    page.eval_for_test("__report()");

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    // Listeners must hear the SAME events the `on*` handlers always did — including `progress`, which
    // is the whole point of the streaming path, and `loadend`, which is what the two paths disagreed
    // about.
    for claim in ["readystatechange", "progress", "load", "loadend"] {
        assert!(
            got.contains(claim),
            "G_XHR_EVENTTARGET(streaming): a listener never heard `{claim}`\n  got: {got}\n\n  \
             The streaming and buffered delivery paths dispatched XHR events from separate open-coded \
             sites and had already diverged on `loadend`. Both now route through `__xhrFire`, and both \
             are gated, because a gate on one path calls the divergence green."
        );
    }
}
