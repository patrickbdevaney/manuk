//! **G_ANCHOR_URL_SETTERS — the WRITE side of the URL-decomposition IDL on `<a>`/`<area>`.**
//!
//! The getters (`a.protocol`/`hostname`/`port`/`host`/`pathname`/`search`/`hash`/`origin`) have worked
//! since the mdbook TOC fix, but every SETTER was a silent no-op — `link.search = '?utm=x'` (the canonical
//! analytics-tag idiom) and `a.hash = '#' + id` (in-page nav) changed nothing, and `a.href` never moved. The
//! assignment must re-parse the element's resolved href, mutate the one component with the real URL parser,
//! and write the re-serialised URL back to `href` so the getter and any later navigation see it. Each claim
//! is a way this goes RED:
//!
//!   * `search`/`hash`/`pathname`/`hostname`/`protocol`/`port` each rewrite only their own component of `href`.
//!   * a `?`-less value assigned to `search` is normalised (the getter reads back `?q=1`).
//!   * the setters work on `<area>` too, and `origin` stays read-only (no setter).

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<a id="s" href="https://ex.com/a/b?x=1#f">L</a>
<a id="h" href="https://ex.com/a/b?x=1#f">L</a>
<a id="p" href="https://ex.com/a/b?x=1#f">L</a>
<a id="n" href="https://ex.com/a/b?x=1#f">L</a>
<a id="pr" href="https://ex.com/a/b?x=1#f">L</a>
<a id="po" href="https://ex.com/a/b?x=1#f">L</a>
<a id="nq" href="https://ex.com/a/b?x=1#f">L</a>
<area id="ar" href="https://ex.com/x?a=1">
<div id="out">-</div><script>
var r=[];
function k(n,v){ r.push(n+':'+JSON.stringify(v)); }
function g(id){ return document.getElementById(id); }
var s=g('s'); s.search='?z=9'; k('search', s.href);
var h=g('h'); h.hash='#top'; k('hash', h.href);
var p=g('p'); p.pathname='/c/d'; k('pathname', p.href);
var n=g('n'); n.hostname='other.com'; k('hostname', n.href);
var pr=g('pr'); pr.protocol='http:'; k('protocol', pr.href);
var po=g('po'); po.port='8443'; k('port', po.href);
var nq=g('nq'); nq.search='q=1'; k('noq', nq.search);
var ar=g('ar'); ar.search='?b=2'; k('area', ar.href);
document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn anchor_url_component_setters() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://anchor-url.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "search:\"https://ex.com/a/b?z=9#f\"", // query replaced, path/hash kept
        "hash:\"https://ex.com/a/b?x=1#top\"", // fragment replaced
        "pathname:\"https://ex.com/c/d?x=1#f\"", // path replaced
        "hostname:\"https://other.com/a/b?x=1#f\"", // host replaced, port/path/query/hash kept
        "protocol:\"http://ex.com/a/b?x=1#f\"", // scheme replaced
        "port:\"https://ex.com:8443/a/b?x=1#f\"", // port added
        "noq:\"?q=1\"",                        // a ?-less search is normalised on read-back
        "area:\"https://ex.com/x?b=2\"",       // the IDL is on <area> too
    ] {
        assert!(
            got.contains(claim),
            "G_ANCHOR_URL_SETTERS: expected {claim} in {got:?}\n  \
             a.search/hash/pathname/hostname/protocol/port assignment must re-serialise href (and work on \
             <area>), or every analytics tag that sets link.search silently no-ops."
        );
    }
}
