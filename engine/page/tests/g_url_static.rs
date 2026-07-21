//! **G_URL_STATIC — the static URL validators `URL.canParse` / `URL.parse`.**
//!
//! The modern way to ask "is this a valid URL?" without wrapping `new URL(x)` in a try/catch on the
//! hot path. Form validation, router libraries and input sanitizers call them directly; absent, the
//! call is a hard `TypeError: URL.canParse is not a function`. The `URL` constructor is native here —
//! these were the two static helpers it was missing.
//!
//! Teeth: `canParse` must return a BOOLEAN that agrees with the constructor (true where `new URL`
//! succeeds, false where it throws — including a relative URL with NO base), and `parse` must return
//! a real URL object on success and `null` (NOT a throw) on failure. A stub that returns `true`/an
//! object unconditionally fails the `bad-*` claims.
//!
//! Proven RED: delete the shim and `has-canParse`/`has-parse` read `undefined` and the first call
//! throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
try {
  push('has-canParse:' + (typeof URL.canParse === 'function'));
  push('has-parse:' + (typeof URL.parse === 'function'));

  // canParse: a real boolean that agrees with the constructor (each push prints the raw result).
  push('good-canParse:' + URL.canParse('https://example.test/a?b=1#c'));  // -> true
  push('bad-canParse:' + URL.canParse('not a valid url'));                // -> false, not a throw
  // a relative URL with no base is NOT parseable; with a base it is.
  push('rel-nobase:' + URL.canParse('/path/only'));                       // -> false
  push('rel-withbase:' + URL.canParse('/path/only', 'https://host.test/')); // -> true

  // parse: a URL object on success, null on failure (never a throw).
  var u = URL.parse('https://example.test/x');
  push('good-parse:' + (u !== null && u.href === 'https://example.test/x'));
  push('parse-withbase:' + (URL.parse('/p', 'https://host.test/').href === 'https://host.test/p'));
  push('bad-parse-null:' + (URL.parse('::::not a url') === null));
} catch (e) {
  push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn url_can_parse_and_parse_validate_without_try_catch() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://url.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("URL-STATIC RESULT: {got}");

    for claim in [
        "has-canParse:true",
        "has-parse:true",
        "good-canParse:true",  // valid absolute -> true
        "bad-canParse:false",  // garbage -> false, not a throw
        "rel-nobase:false",    // relative with no base is not parseable
        "rel-withbase:true",   // relative + base is parseable
        "good-parse:true",     // parse returns a real URL with the right href
        "parse-withbase:true", // base resolution works
        "bad-parse-null:true", // failure is null, never a throw
    ] {
        assert!(
            got.contains(claim),
            "G_URL_STATIC: expected `{claim}`\n  got: {got}\n\n  \
             `URL.canParse`/`URL.parse` must validate a URL without try/catch: canParse returns a \
             boolean agreeing with `new URL`, parse returns a URL object or `null` (never a throw)."
        );
    }
}
