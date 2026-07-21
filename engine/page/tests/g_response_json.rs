//! **G_RESPONSE_JSON — the static `Response.json(data, init)`.**
//!
//! The one-call JSON response: `return Response.json({ ok: true })` in a Service Worker `fetch` handler
//! or an app route. It was missing (`Response.json is not a function`) even though `Response` itself and
//! `res.json()` were real. It JSON-serialises the data, defaults `Content-Type` to `application/json`,
//! and round-trips through `res.json()`.
//!
//! Proven RED: delete the `Response.json` assignment and `present` reads `undefined` while the call
//! throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function finish() { document.getElementById('out').textContent = r.join(' '); }

try {
  push('present:' + (typeof Response.json === 'function'));

  var res = Response.json({ ok: true, n: 42 });
  push('status:' + (res.status === 200));
  push('content-type:' + (res.headers.get('content-type') === 'application/json'));

  var res2 = Response.json({ created: 1 }, { status: 201 });
  push('custom-status:' + (res2.status === 201));

  // round-trip through res.json().
  res.json().then(function (data) {
    push('round-trip:' + (data.ok === true && data.n === 42));
    finish();
  }, function (e) { push('JSON-THREW:' + e); finish(); });
} catch (e) {
  push('THREW:' + e);
  finish();
}
</script></body></html>"##;

#[test]
fn response_json_static_builds_a_json_response() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://respjson.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("RESPONSE-JSON RESULT: {got}");

    for claim in [
        "present:true",
        "status:true",        // defaults to 200
        "content-type:true",  // application/json by default
        "custom-status:true", // honours init.status
        "round-trip:true",    // res.json() parses back the data
    ] {
        assert!(
            got.contains(claim),
            "G_RESPONSE_JSON: expected `{claim}`\n  got: {got}\n\n  \
             `Response.json(data, init)` must build a Response whose body is the JSON of `data`, \
             default Content-Type to application/json, honour init.status, and round-trip via json()."
        );
    }
}
