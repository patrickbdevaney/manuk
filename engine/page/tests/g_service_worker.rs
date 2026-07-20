//! **G_SERVICE_WORKER — registration, the install→activate lifecycle, and `fetch` interception.**
//!
//! **The failure this gate exists for.** `navigator.serviceWorker` was absent, and what a page loses
//! to that is not "offline mode". It is the entire PWA branch: no offline shell, no instant repeat
//! visit, and on a growing number of sites **no first render at all**, because the page awaits
//! `navigator.serviceWorker.ready` before it will paint. Nothing throws. The page simply never
//! arrives — the same silent-grading shape as `caches` and `localStorage` before it.
//!
//! Tick 279 built the service worker's STORE (the Cache API) and tick 280 built the scope a worker
//! script runs in. This is the third side, and it is the one that makes the other two do anything.
//!
//! **The claim that matters most is `waituntil`.** `install` extending its own lifetime until the
//! cache is filled is the *entire* contract of an offline install step. An implementation that fires
//! `activate` without awaiting the promises passed to `waitUntil` passes every API-shaped assertion —
//! registration resolves, both events fire, in the right order — and then serves from a cache it has
//! not finished writing. That failure does not appear at registration; it appears as a **miss on the
//! first offline load**, long afterwards, and looks like a bug in the site. So the service worker
//! here does its cache write asynchronously inside `waitUntil` and records, *at activate time*,
//! whether that write had finished. Under a lifecycle that does not await, `waituntil` flips alone
//! while `installed`, `activated` and `order` all still pass.
//!
//! **`passthrough` without a network.** A request the worker declines must reach the network, not be
//! swallowed — but proving "it went to the network" in an offline test cannot mean waiting for a
//! response. So the worker records every URL it is asked about and serves that list back on a third
//! URL: the assertion is that the handler *ran* for `/other.txt` and *declined* it, which is exactly
//! the fall-through, observed from the only side that can see it.
//!
//! The rest: `intercepted` (a page `fetch` answered entirely from cache, never touching the
//! network), `controller`/`ready` (what pages actually branch on), `swnodoc` (a service worker is
//! even more emphatically not on the main thread than a dedicated worker is), and `unregistered`.
//!
//! RED: dropping the `waitUntil` await flips `waituntil` alone; returning the network fetch instead
//! of the `respondWith` value flips `intercepted`; falling back to the network when a handler
//! responded flips `intercepted` too; never setting `controller` flips `controller` while everything
//! else passes — the half-working state where a page's own `if (navigator.serviceWorker.controller)`
//! check sends it down the uncontrolled path forever.
//!
//! HONEST LIMITS, gated nowhere because they are absent rather than wrong: no navigation
//! interception, no update/redundant lifecycle, no `clients` beyond a stub, no push, no background
//! sync, and scope matching does not go beyond a path prefix.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } },
    };

    try {
      var SW = [
        "var installed = false, cacheDone = false, seen = [];",
        "self.addEventListener('install', function (e) {",
        "  installed = true;",
        // Asynchronous on purpose: the whole point of waitUntil is that activate must not
        // happen until this settles.
        "  e.waitUntil(caches.open('v1').then(function (c) {",
        "    return c.put('/hello.txt', new Response('FROM-CACHE'));",
        "  }).then(function () { cacheDone = true; }));",
        "});",
        "self.addEventListener('activate', function (e) {",
        // Recorded AT ACTIVATE TIME — this is the ordering proof, and it is only meaningful
        // because it is captured here rather than read later once everything has settled.
        "  var diag = { install: installed, cacheDone: cacheDone, nodoc: (typeof document === 'undefined') };",
        "  e.waitUntil(caches.open('v1').then(function (c) {",
        "    return c.put('/diag', new Response(JSON.stringify(diag)));",
        "  }));",
        "});",
        "self.addEventListener('fetch', function (e) {",
        "  var u = String(e.request && e.request.url ? e.request.url : e.request);",
        "  seen.push(u);",
        "  if (u.indexOf('/hello.txt') >= 0) { e.respondWith(caches.match('/hello.txt')); return; }",
        "  if (u.indexOf('/seen') >= 0) { e.respondWith(new Response(seen.join(','))); return; }",
        // Everything else is DECLINED — no respondWith — and must fall through to the network.
        "});"
      ].join('\n');

      var url = URL.createObjectURL(new Blob([SW]));

      navigator.serviceWorker.register(url).then(function (reg) {
        R.push('registered:' + (!!reg && !!reg.active));
        R.push('activated:' + (reg.active && reg.active.state === 'activated'));
        R.push('controller:' + (!!navigator.serviceWorker.controller));

        return navigator.serviceWorker.ready.then(function (r2) {
          R.push('ready:' + (r2 === reg));

          // What the worker recorded at activate time.
          return caches.match('/diag').then(function (resp) {
            return resp.text();
          }).then(function (t) {
            var d = JSON.parse(t);
            R.push('installed:' + (d.install === true));
            R.push('order:' + (d.install === true));
            // The load-bearing one: the install's async cache write had COMPLETED before
            // activate ran.
            R.push('waituntil:' + (d.cacheDone === true));
            R.push('swnodoc:' + (d.nodoc === true));
          });
        }).then(function () {
          // A page fetch answered entirely from the cache — the network is never touched.
          return fetch('/hello.txt').then(function (r) { return r.text(); }).then(function (body) {
            R.push('intercepted:' + (body === 'FROM-CACHE'));
          });
        }).then(function () {
          // A request the worker declines. Its promise may never settle offline, and that is
          // the point — we do not wait on it.
          var pass = fetch('/other.txt');
          if (pass && typeof pass.catch === 'function') { pass.catch(function () {}); }
          return fetch('/seen').then(function (r) { return r.text(); }).then(function (list) {
            R.push('passthrough:' + (list.indexOf('/other.txt') >= 0));
          });
        }).then(function () {
          return reg.unregister().then(function () {
            R.push('unregistered:' + (navigator.serviceWorker.controller === null));
          });
        });
      }).catch(function (e) { R.push('THREW:' + e); });
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

/// One test in the binary — a second `Page::load` stands up a second SpiderMonkey context in the
/// same process and does not survive teardown (see `g_mse`, `g_web_worker`, `g_globals`).
#[test]
fn a_service_worker_registers_activates_and_intercepts_fetch() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://pwa.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("registered:true", "`navigator.serviceWorker.register()` must resolve to a registration with an active worker — a page that awaits this and never gets it never paints"),
        ("installed:true", "the `install` event must actually fire at the worker scope"),
        ("order:true", "`install` must have run before `activate` — the worker observed it from inside the activate handler"),
        ("waituntil:true", "`activate` must not run until every promise passed to install's `waitUntil` has settled; without this the worker serves from a cache it has not finished writing, and the failure surfaces as a miss on the first offline load"),
        ("activated:true", "the registration's active worker must report state 'activated'"),
        ("controller:true", "`navigator.serviceWorker.controller` must be set — pages branch on it directly, and a null controller sends them down the uncontrolled path forever"),
        ("ready:true", "`navigator.serviceWorker.ready` must resolve with the same registration; a page that awaits it before painting depends on exactly this"),
        ("swnodoc:true", "a service worker must not see `document` — it is even more emphatically off the main thread than a dedicated worker"),
        ("intercepted:true", "a page `fetch` the worker answers with `respondWith` must return the worker's response and never touch the network — this is the whole offline capability"),
        ("passthrough:true", "a request the worker declines must fall through to the network rather than being swallowed; the worker saw `/other.txt` and did not respond to it"),
        ("unregistered:true", "`unregister()` must drop the controller, or an uninstalled worker keeps intercepting"),
    ] {
        assert!(
            got.contains(claim),
            "G_SERVICE_WORKER: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
