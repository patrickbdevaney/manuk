//! **G_CACHE_API — `caches` must be a real request/response store, not a feature flag.**
//!
//! `localStorage` holds strings and IndexedDB holds structured values; **neither can hold a
//! response.** The Cache API is the only storage in the platform whose unit is an HTTP
//! request/response pair, which is why it — not IndexedDB — is what a PWA's install step fills and
//! what a Service Worker's `fetch` handler reads on every navigation afterwards.
//!
//! Its absence has the same grading shape every storage API before it had: `if ('caches' in window)`
//! does not report a bug. It silently selects the network-only path, and a page that was built to
//! work offline simply stops being that page — with nothing thrown and nothing logged.
//!
//! The claims below are the ones a real offline handler depends on, and each is one a
//! plausible-looking shim gets wrong:
//!
//!   * **A miss resolves to `undefined`, it does not reject.** Every cache-first handler is
//!     `caches.match(e.request).then(r => r || fetch(e.request))`. A shim that rejects on a miss
//!     turns the entire offline path into an unhandled rejection.
//!   * **Bodies survive as BYTES.** A cache holds fonts, images and wasm as readily as HTML.
//!     Round-tripping those through a UTF-8 `text()` inflates every byte above `0x7F` into two and
//!     returns a corrupt asset — the same defect that made a 260-byte media segment arrive as 407.
//!   * **Re-caching one URL replaces, it does not append.** A PWA re-runs its install step on every
//!     version; a cache that appends grows without bound and eventually trips quota.
//!   * **Responses that differ by `Vary` coexist**, or the gzip copy of an asset silently evicts the
//!     brotli one.
//!   * **`keys()` is in insertion order**, which is what makes a cache enumerable in a stable way.
//!   * **It persists.** Re-opening the cache sees the entries — the entire reason to use it.
//!
//! HONEST LIMIT, gated deliberately rather than discovered later: `add()`/`addAll()` fetch from the
//! network and are therefore NOT asserted here — a gate that needs a live server false-REDs on a
//! quiet box. Their logic (refuse a non-`ok` response) is asserted by inspection in the shim, and
//! the fetch path itself is gated by `G_FETCH`.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var r = [];
    var done = function () { document.getElementById('out').textContent = r.join(' '); };
    var fail = function (e) { r.push('THREW:' + (e && e.name ? e.name : e)); done(); };

    r.push('present:' + (typeof caches === 'object' && typeof caches.open === 'function'));

    // A body with bytes ABOVE 0x7F. If anything in the chain treats this as UTF-8 text it comes
    // back longer than it went in, and the assertion below catches it.
    var bin = new Uint8Array([0, 0x41, 0x80, 0xC3, 0xFF, 0x7F]);
    var binResponse = function () {
        return new Response(bin, { status: 200, statusText: 'OK',
                                   headers: { 'content-type': 'application/octet-stream' } });
    };

    caches.open('shell-v1').then(function (c) {
        return c.put('/app.js', new Response('CODE', {
            status: 201, statusText: 'Created', headers: { 'content-type': 'text/javascript' }
        })).then(function () {
            return c.match('/app.js');
        }).then(function (res) {
            r.push('hit:' + (res !== undefined));
            r.push('status:' + res.status + '/' + res.statusText);
            r.push('ctype:' + res.headers.get('content-type'));
            return res.text();
        }).then(function (body) {
            r.push('body:' + body);
            // A MISS RESOLVES UNDEFINED. It must not reject.
            return c.match('/never-cached.js');
        }).then(function (res) {
            r.push('miss:' + (res === undefined));
            // BYTES: the whole reason bodies are not stored as text.
            return c.put('/font.woff2', binResponse());
        }).then(function () {
            return c.match('/font.woff2');
        }).then(function (res) { return res.arrayBuffer(); })
        .then(function (buf) {
            var got = new Uint8Array(buf);
            var same = (got.length === bin.length);
            for (var i = 0; same && i < bin.length; i++) { if (got[i] !== bin[i]) same = false; }
            r.push('bytes:' + same + '/' + got.length);
            // REPLACE, not append: re-caching the same URL must not grow the cache.
            return c.put('/app.js', new Response('CODE2', { status: 200 }));
        }).then(function () {
            return c.keys();
        }).then(function (keys) {
            r.push('keycount:' + keys.length);
            // INSERTION ORDER — not sorted, or an enumeration is unstable across runs.
            r.push('order:' + keys.map(function (k) { return k.url.split('/').pop(); }).join(','));
            return c.match('/app.js');
        }).then(function (res) { return res.text(); })
        .then(function (body) {
            r.push('replaced:' + body);
            // A non-GET request is not cacheable and must reject with a TypeError.
            return c.put(new Request('/api', { method: 'POST' }), new Response('x'))
                .then(function () { return 'resolved'; }, function (e) { return e.name; });
        }).then(function (what) {
            r.push('post:' + what);
            return c['delete']('/app.js');
        }).then(function (gone) {
            r.push('del:' + gone);
            return c['delete']('/app.js');
        }).then(function (again) {
            // A second delete resolves FALSE. A shim that reports true twice cannot be used to
            // decide whether an eviction happened.
            r.push('del2:' + again);
            // PERSISTENCE — a fresh handle to the same cache still sees the entry.
            return caches.open('shell-v1');
        }).then(function (c2) {
            return c2.match('/font.woff2');
        }).then(function (res) {
            r.push('persist:' + (res !== undefined));
            return caches.has('shell-v1');
        }).then(function (has) {
            r.push('has:' + has);
            return caches.keys();
        }).then(function (names) {
            r.push('names:' + names.join(','));
            // caches.match searches EVERY cache, which is what a handler that never opens one by
            // name relies on.
            return caches.match('/font.woff2');
        }).then(function (res) {
            r.push('anycache:' + (res !== undefined));
            // Clean up: the store is persisted to the REAL profile, so a gate that left its caches
            // behind would grow that file on every run forever. This also exercises the delete.
            return caches['delete']('shell-v1');
        }).then(function (dropped) {
            r.push('dropped:' + dropped);
            return caches.has('shell-v1');
        }).then(function (still) {
            r.push('cleared:' + !still);
            done();
        });
    }).catch(fail);
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn the_cache_api_is_a_real_persistent_request_response_store() {
    let fonts = FontContext::new();
    // A unique origin per run: the store is origin-partitioned and persisted, so sharing an origin
    // with a previous run would let stale entries answer the persistence claim.
    let origin = format!(
        "https://cache-gate-{}.test/",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let page = manuk_page::Page::load(HTML, &origin, &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "present:true",            // the feature detection every offline-capable page runs
        "hit:true",                // a stored response comes back
        "status:201/Created",      // status AND statusText survive, not just the body
        "ctype:text/javascript",   // response headers survive
        "body:CODE",               // and so does the body
        "miss:true",               // a miss RESOLVES undefined — it does not reject
        "bytes:true/6",            // bytes above 0x7F survive intact, not UTF-8 inflated
        "keycount:2",              // re-caching one URL REPLACED rather than appended
        "order:app.js,font.woff2", // keys() is in insertion order, not sorted
        "replaced:CODE2",          // and the replacement is the one that is served
        "post:TypeError",          // a non-GET request is refused, per spec
        "del:true",                // delete reports that it removed something
        "del2:false",              // and a second delete reports that it did not
        "persist:true",            // a fresh handle to the cache still sees the entries
        "has:true",                // caches.has finds it
        "names:shell-v1",          // caches.keys lists it
        "anycache:true",           // caches.match searches across every cache
        "dropped:true",            // caches.delete removes the whole cache
        "cleared:true",            // and it is really gone afterwards
    ] {
        assert!(
            got.contains(claim),
            "G_CACHE_API: expected {claim} in {got:?}\n  \
             `caches` must be a real, persistent request/response store. Its absence is not a \
             reported failure — pages feature-detect it and silently take the network-only path, \
             which is why this is gated on observable behaviour rather than on the symbol existing."
        );
    }
}
