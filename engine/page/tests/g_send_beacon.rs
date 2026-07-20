//! **G_SEND_BEACON — `navigator.sendBeacon` actually POSTs, and refuses honestly when it cannot.**
//!
//! Every analytics, RUM and error-reporting library ends a session by calling
//! `navigator.sendBeacon(url, payload)` from a `pagehide`/`visibilitychange` handler — the one moment
//! a normal `fetch` cannot be relied on because the page is going away. It was ABSENT, so an unguarded
//! `navigator.sendBeacon(...)` threw on `undefined` and took the rest of the unload handler with it,
//! which is where SPAs flush their final state.
//!
//! The trap this gate exists for: `sendBeacon` returns a **boolean**, so the cheapest wrong
//! implementation is `return true` — which passes every "is it a function / did it return true" check
//! while sending nothing at all. That is indistinguishable from a working beacon until the telemetry
//! never arrives. So this gate does not assert the return value alone; it drains the page's outgoing
//! request queue (`take_fetches`, the same channel `fetch` uses) and asserts a real **POST** left the
//! page, with the right body and the content-type the payload's type implies.
//!
//! **RED, run:** delete the impl → `present:false` and the first call throws (nothing after it
//! records). Make it `return true` without enqueuing → the return-value claims stay green but
//! `take_fetches` is empty and every `posted:*` claim fails. Make the oversized branch enqueue instead
//! of returning false → `over:false` flips and the too-big POST appears in the queue. The
//! return-value claims and the queue claims are complements: no constant satisfies both.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<div id="out">-</div>
<script>
  var R = [];
  var nav = navigator;
  R.push('present:' + (typeof nav.sendBeacon === 'function'));

  // 1. A string payload — the default, text/plain;charset=UTF-8.
  R.push('ret:' + nav.sendBeacon('https://beacon.test/collect', 'e=click&n=1'));

  // 2. A typed Blob — the content-type comes from the Blob, not a guess.
  var b = new Blob(['{"event":"purchase"}'], { type: 'application/json' });
  R.push('blobret:' + nav.sendBeacon('https://beacon.test/blob', b));

  // 3. No payload at all — a bare ping, no content-type.
  R.push('nodata:' + nav.sendBeacon('https://beacon.test/ping'));

  // 4. An oversized payload must be REFUSED with false and NOT queued — a page that checks the
  //    return value falls back to a synchronous request, and a silent drop would lose the data.
  var big = new Array(70000).join('x');   // 69999 chars, over the 65536 cap
  R.push('over:' + nav.sendBeacon('https://beacon.test/toobig', big));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"#;

#[test]
fn send_beacon_posts_for_real_and_refuses_oversized() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://beacon.test/", &fonts, 800.0);

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("present:true", "an absent sendBeacon throws on the first unload-handler call"),
        ("ret:true", "a queued string beacon returns true"),
        ("blobret:true", "a queued Blob beacon returns true"),
        ("nodata:true", "a payload-less ping is valid and returns true"),
        (
            "over:false",
            "a payload past the in-flight cap is refused with false — never silently dropped while \
             telling the page it was sent",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_SEND_BEACON: expected {claim} in {got:?}\n  {why}"
        );
    }

    // The real proof: draining the outgoing queue shows genuine POSTs, not a return-true no-op.
    // (id, url, method, headers, body)
    let fetches = page.take_fetches();

    let find = |needle: &str| {
        fetches
            .iter()
            .find(|f| f.1.contains(needle))
            .unwrap_or_else(|| {
                panic!(
                    "G_SEND_BEACON: no POST to {needle:?} in the outgoing queue {:?}\n  \
                 sendBeacon must ACTUALLY send — a `return true` that enqueues nothing is the \
                 vacuous stub this gate exists to catch.",
                    fetches
                        .iter()
                        .map(|f| (&f.1, &f.2, &f.4))
                        .collect::<Vec<_>>()
                )
            })
    };
    let ct = |f: &(u32, String, String, Vec<(String, String)>, String)| {
        f.3.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };

    let collect = find("/collect");
    assert_eq!(
        collect.2, "POST",
        "G_SEND_BEACON: a beacon is a POST, got {:?}",
        collect.2
    );
    assert_eq!(
        collect.4, "e=click&n=1",
        "G_SEND_BEACON: the string body must reach the wire verbatim"
    );
    assert!(
        ct(collect).contains("text/plain"),
        "G_SEND_BEACON: a string beacon is text/plain, got content-type {:?}",
        ct(collect)
    );

    let blob = find("/blob");
    assert_eq!(blob.2, "POST");
    assert_eq!(
        blob.4, "{\"event\":\"purchase\"}",
        "G_SEND_BEACON: the Blob's bytes are the body"
    );
    assert!(
        ct(blob).contains("application/json"),
        "G_SEND_BEACON: a typed Blob carries ITS type, not a guessed one, got {:?}",
        ct(blob)
    );

    let ping = find("/ping");
    assert_eq!(ping.2, "POST");
    assert_eq!(
        ping.4, "",
        "G_SEND_BEACON: a payload-less ping has an empty body"
    );
    assert!(
        ct(ping).is_empty(),
        "G_SEND_BEACON: a no-data beacon sends no content-type, got {:?}",
        ct(ping)
    );

    assert!(
        !fetches.iter().any(|f| f.1.contains("toobig")),
        "G_SEND_BEACON: the oversized beacon returned false and MUST NOT be in the queue — \
         found it in {:?}",
        fetches.iter().map(|f| &f.1).collect::<Vec<_>>()
    );
}
