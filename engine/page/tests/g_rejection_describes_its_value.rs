//! **G_REJECTION_DESCRIBES_ITS_VALUE — an unhandled rejection must say WHAT was rejected.**
//!
//! `String(reason)` on a plain object is `[object Object]`, and that is a log line that costs a tick.
//! Measured on `beb88run.xyz` (t891): **sixteen unhandled rejections in one load, every one reported
//! as `error=[object Object]`** — so the log named the count and nothing else, while the page was
//! missing a 458-element carousel subtree and the investigation had no thread to pull.
//!
//! **A rejected value is very often not an `Error`.** `fetch` handlers reject with a `Response`, XHR
//! wrappers with `{status, statusText}`, and a large share of the ad/analytics bundles on the web
//! reject with a bare config object. This is the standing rule — *if a message speculates about state
//! the process is holding, print the state* (t684-692) — applied to the one place that was printing a
//! default `toString` instead.
//!
//! `window.__describeRejection` is the same function the unhandled-rejection reporter uses, exposed so
//! the behaviour is assertable rather than only observable in a log the test harness does not capture.
//!
//! ⚠ The description is **bounded on purpose**: constructor name, first six own keys, a JSON body
//! clipped at 300 chars. A log line that dumps an object graph is as unreadable as `[object Object]`
//! and is a denial-of-service on the sweep's own output.
//!
//! ⚠ And `__`-prefixed keys are filtered, because they are THIS ENGINE'S internals (`__nodeId` and
//! friends). Without that filter a plain `<div>` reports `keys=[__nodeId]`, which tells the reader a
//! page object carries state it does not — and gives them no way to tell ours from theirs.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
  var d = window.__describeRejection, r = [];
  r.push('http=' + d({status:404, statusText:'Not Found', url:'/api/x'}));
  r.push('empty=' + d({}));
  r.push('node=' + d(document.createElement('div')));
  r.push('str=' + d('a string'));
  r.push('null=' + d(null));
  r.push('num=' + d(7));
  document.getElementById('out').textContent = r.join(' | ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn an_unhandled_rejection_describes_the_value_it_carries() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://rejection.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("REJECTION DESCRIPTIONS: {got}");

    for (claim, why) in [
        (
            r#"http={"status":404,"statusText":"Not Found","url":"/api/x"}"#,
            "**THE CASE THIS EXISTS FOR.** A rejected fetch-shaped object read `[object Object]`, so \
             sixteen of them on one page said only that there were sixteen. The status, the text and \
             the URL are the whole diagnosis and they were all present and all discarded",
        ),
        (
            "empty=Object (no own keys)",
            "an object with nothing in it must SAY so rather than read as an unstringifiable \
             mystery — 'there is no information here' is itself information, and it is what \
             distinguishes a bare `Promise.reject({})` from a payload we failed to print",
        ),
        (
            "node=HTMLDivElement <div> (no own keys)",
            "**A HOST OBJECT HAS NO USEFUL JSON, SO THE TAG IS THE FACT.** And the `(no own keys)` is \
             the guard on the internals filter: without it this reads `keys=[__nodeId]`, which \
             advertises OUR expando as the page's own state",
        ),
        (
            "str=a string",
            "**THE GUARD**: a primitive must pass through untouched. A describer that wrapped every \
             value in constructor-and-keys ceremony would make the common case worse than the bug",
        ),
        ("null=null", "…and `null` is a legitimate rejection value, not an error in the describer"),
        ("num=7", "…and a number"),
    ] {
        assert!(
            got.contains(claim),
            "G_REJECTION_DESCRIBES_ITS_VALUE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
