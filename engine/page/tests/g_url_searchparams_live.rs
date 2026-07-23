//! **G_URL_SEARCHPARAMS_LIVE — `url.searchParams` is LIVE, and its constructor accepts any iterable.**
//!
//! Two gaps in the URL query surface, both on the single most common "build a URL with query params"
//! path:
//!
//!   * **`url.searchParams` was a dead SNAPSHOT.** `const u = new URL(page); u.searchParams.set('page',
//!     '2'); fetch(u)` is how every paginator, filter bar and API client assembles a request — and the
//!     mutation vanished: `u.href` and `u.search` kept their original value, so the ORIGINAL url was
//!     fetched, silently. The spec makes `searchParams` live: a mutation rewrites `search` and `href`.
//!   * **`new URLSearchParams(formData)` produced garbage.** A FormData is iterable of `[name, value]`
//!     pairs, so `new URLSearchParams(new FormData(form))` is the standard "serialize this form to a
//!     query string" idiom — but the constructor only special-cased `Array.isArray`, so a FormData fell
//!     to the record branch and iterated its own METHODS (append/get/…) as keys.
//!
//! The claims check observable `href`/`search`/`get` results, each a way the old snapshot / narrow
//! constructor goes RED:
//!
//!   * **`searchParams.set/append/delete`** rewrite `url.href` and `url.search`.
//!   * A mutation **preserves the `#hash`** and, when it empties the query, **drops the `?`**.
//!   * **`new URLSearchParams(formData | Map | URLSearchParams)`** reads the pairs, not the object's keys.
//!   * A **standalone** URLSearchParams (no URL) still works — the live hook is a no-op there.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><form id="f"><input name="a" value="1"><input name="b" value="2"></form><div id="out">-</div><script>
    var r = [];
    // Live: set reflects into href + search.
    var u1 = new URL('https://x.com/p?a=1');
    u1.searchParams.set('b', '2');
    r.push('set:' + u1.href + '|' + u1.search);
    // Live: append keeps duplicates and order.
    var u2 = new URL('https://x.com/p');
    u2.searchParams.append('a', '1'); u2.searchParams.append('a', '2');
    r.push('append:' + u2.href);
    // Live: delete + hash preservation.
    var u3 = new URL('https://x.com/p?a=1&b=2#frag');
    u3.searchParams.delete('a');
    r.push('del:' + u3.href);
    // Live: emptying the query drops the '?'.
    var u4 = new URL('https://x.com/p?a=1');
    u4.searchParams.delete('a');
    r.push('empty:' + u4.href + '|' + (u4.search === '' ? 'noq' : u4.search));
    // Constructor: FormData is iterable of pairs.
    var p = new URLSearchParams(new FormData(document.getElementById('f')));
    r.push('fd:' + p.get('a') + ',' + p.get('b'));
    // Constructor: a Map is iterable of pairs too.
    r.push('map:' + new URLSearchParams(new Map([['k', 'v']])).get('k'));
    // Standalone URLSearchParams still mutates fine (the live hook is inert without a URL).
    var sp = new URLSearchParams('a=1'); sp.set('b', '2');
    r.push('solo:' + sp.toString());
    document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn url_searchparams_is_live_and_accepts_iterables() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://url-searchparams-live.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "set:https://x.com/p?a=1&b=2|?a=1&b=2", // set rewrites href AND search
        "append:https://x.com/p?a=1&a=2",       // append keeps duplicates in order
        "del:https://x.com/p?b=2#frag",         // delete rewrites href and preserves the #hash
        "empty:https://x.com/p|noq",            // emptying the query drops the '?'
        "fd:1,2",                               // new URLSearchParams(formData) reads the pairs
        "map:v",                                // ...and a Map
        "solo:a=1&b=2", // a standalone URLSearchParams is unaffected (no regression)
    ] {
        assert!(
            got.contains(claim),
            "G_URL_SEARCHPARAMS_LIVE: expected {claim} in {got:?}\n  \
             url.searchParams must be LIVE (a mutation rewrites href/search) and its constructor must \
             accept any iterable of pairs (FormData/Map) — a dead snapshot silently fetches the \
             original URL, and every paginator/filter/API client builds requests this way."
        );
    }
}
