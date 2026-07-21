//! **G_URLPATTERN — `URLPattern`, the URL matcher for routing.**
//!
//! SPA routers and Service Worker routing dispatch a request by shape:
//! `new URLPattern({ pathname: '/users/:id' }).exec(url).pathname.groups.id`. It was ABSENT, so
//! `new URLPattern(...)` threw. This is a real matcher for the pathname component — the one routers key
//! on — so the teeth are actual match/no-match results and extracted named groups, not just presence.
//!
//! Proven RED: delete the `URLPattern` block and `present` reads `undefined` while the first call throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }

try {
  push('present:' + (typeof URLPattern === 'function'));

  var p = new URLPattern({ pathname: '/users/:id' });
  push('match:' + (p.test('https://ex.com/users/42') === true));
  push('no-match:' + (p.test('https://ex.com/users/42/extra') === false));

  var m = p.exec('https://ex.com/users/42');
  push('group:' + (m !== null && m.pathname.groups.id === '42'));
  push('null-on-miss:' + (p.exec('https://ex.com/posts/1') === null));

  // wildcard captures the rest of the path (indexed group '0').
  var w = new URLPattern({ pathname: '/files/*' });
  var wm = w.exec('/files/a/b/c.txt');
  push('wildcard:' + (w.test('/files/a/b/c.txt') === true && wm.pathname.groups['0'] === 'a/b/c.txt'));

  // string shorthand is a pathname pattern.
  push('shorthand:' + (new URLPattern('/x/:y').exec('/x/9').pathname.groups.y === '9'));
} catch (e) {
  push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn url_pattern_matches_and_extracts_groups() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://pat.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("URLPATTERN RESULT: {got}");

    for claim in [
        "present:true",
        "match:true",        // /users/:id matches /users/42
        "no-match:true",     // ...but not /users/42/extra
        "group:true",        // exec extracts groups.id === '42'
        "null-on-miss:true", // exec returns null when nothing matches
        "wildcard:true",     // * captures the rest of the path
        "shorthand:true",    // string form is a pathname pattern
    ] {
        assert!(
            got.contains(claim),
            "G_URLPATTERN: expected `{claim}`\n  got: {got}\n\n  \
             `URLPattern` must match the pathname, extract `:named` groups and `*` wildcards via \
             `exec`, and return null on a miss. A stub that always matches (or never captures) fails."
        );
    }
}
