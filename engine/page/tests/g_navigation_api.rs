//! **G_NAVIGATION_API — `window.navigation` intercepts a same-document navigation.**
//!
//! The Navigation API is the modern successor to the `history.pushState` + click/popstate
//! interception dance that every SPA router used to hand-roll. Instead of monkey-patching link
//! clicks and reconstructing state from the History API, the router listens for one `navigate` event
//! — which fires for *every* same-document navigation — and calls `event.intercept({ handler })` to
//! take it over. Newer frameworks feature-detect `window.navigation` and prefer it; the old fallback
//! path is increasingly the untested one.
//!
//! **The failure without it is the silent-router shape.** A router doing
//! `navigation.addEventListener('navigate', e => e.intercept({ handler: () => render(e.destination.url) }))`
//! finds `navigation` undefined: it either throws (dead router) or binds nothing, and every in-app
//! link then triggers a full document load or does nothing — no error the user can see, the app just
//! stops behaving like an app.
//!
//! So this gate drives the capability rather than detecting it:
//!
//!   1. `window.navigation` exists and its `currentEntry.url` reflects the real document URL.
//!   2. A `navigate` listener FIRES on `navigation.navigate(url)`, and the event carries the
//!      destination (`destination.url`), is interceptable (`canIntercept`), and reports the type.
//!   3. `intercept({ handler })` HANDLER RUNS — the client-side route change actually happens (the
//!      whole point), observed here as a DOM mutation that lands after the microtask drain.
//!   4. The commit reaches the shared History/Location plumbing: `location`/`currentEntry` advance to
//!      the new URL, so the omnibox and back-stack stay consistent.
//!   5. `preventDefault()` ABORTS: the navigation does not commit and `committed` rejects — a router
//!      must be able to veto a navigation (guards, "unsaved changes").
//!
//! RED: removing the `navigation` shim drops every claim below at once — the exact dead-router state.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <h1 id="view">Home</h1>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } },
      join: function (sep) { return this.a.join(sep); }
    };
    var $ = function (id) { return document.getElementById(id); };

    try {
      // ── 1. The object exists and knows where we are.
      R.push('exists:' + (typeof navigation === 'object' && navigation !== null &&
                          typeof navigation.navigate === 'function'));
      R.push('entryurl:' + (String(navigation.currentEntry.url).indexOf('/start') >= 0));

      // ── 2 + 3. The router: one `navigate` listener that intercepts and renders the destination.
      var fired = false, destSeen = '', canInt = false, navType = '';
      navigation.addEventListener('navigate', function (e) {
        fired = true;
        destSeen = e.destination.url;
        canInt = e.canIntercept;
        navType = e.navigationType;
        e.intercept({ handler: function () {
          // The client-side route change. Runs in a microtask; the engine drains it at end of load.
          $('view').textContent = 'Profile:' + e.destination.url;
        } });
      });

      var result = navigation.navigate('/profile?id=7');
      R.push('fired:' + fired);
      R.push('dest:' + (String(destSeen).indexOf('/profile') >= 0));
      R.push('canintercept:' + (canInt === true));
      R.push('navtype:' + (navType === 'push'));
      R.push('result:' + (result && typeof result.committed.then === 'function' &&
                          typeof result.finished.then === 'function'));

      // ── 4. Commit reached the shared plumbing.
      R.push('committed:' + (location.pathname === '/profile' &&
                             String(navigation.currentEntry.url).indexOf('/profile') >= 0));

      // ── 5. A vetoed navigation must not commit. Bind a one-shot guard that cancels.
      var guard = function (e) { e.preventDefault(); };
      navigation.addEventListener('navigate', guard);
      var vetoed = navigation.navigate('/should-not-happen');
      var rejected = false;
      vetoed.committed.then(function () {}, function () { rejected = true; });
      navigation.removeEventListener('navigate', guard);
      // The URL must NOT have moved to the vetoed target.
      R.push('vetoed:' + (location.pathname === '/profile'));

      R.push('ready:true');
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

#[test]
fn navigation_api_intercepts_a_same_document_navigation() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://nav.test/start", &fonts, 800.0);
    let root = page.dom().root();

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("exists:true", "`window.navigation` with a `navigate()` method must be present — newer routers feature-detect it and take this path in preference to the History API"),
        ("entryurl:true", "`currentEntry.url` must reflect the real document URL, or the router cannot reason about where it is"),
        ("fired:true", "the `navigate` event must fire for a same-document navigation — it is the single hook the whole API is built around"),
        ("dest:true", "the event must carry `destination.url` so the router knows what to render"),
        ("canintercept:true", "`canIntercept` must be true for a same-document navigation, which is how the router decides it may take over"),
        ("navtype:true", "the navigation type must be reported (`push`) so history-vs-replace routing works"),
        ("result:true", "`navigate()` must return the `{committed, finished}` promises the caller awaits"),
        ("committed:true", "the commit must reach the shared History/Location plumbing so `location` and `currentEntry` advance together — otherwise the omnibox and back-stack drift from the app's idea of the URL"),
        ("vetoed:true", "`preventDefault()` must abort the navigation — a router needs to veto (route guards, unsaved-changes prompts), and a veto that still committed would be worse than no API"),
        ("ready:true", "the whole sequence must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_NAVIGATION_API: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }

    // ── The load-bearing half a synchronous script cannot self-report: the intercept HANDLER ran and
    // performed the client-side route change. It executes in a microtask; the engine drains those at
    // end of load, so the DOM mutation is observable here.
    let view = manuk_css::query_selector_all(page.dom(), root, "#view")[0];
    let after = page.dom().text_content(view);
    assert!(
        after.contains("Profile:") && after.contains("/profile"),
        "the `intercept({{handler}})` handler must RUN and apply the client-side route change — this \
         is the entire capability; without it the URL changes but the view never updates, which is \
         the dead-router failure\n  got view text: {after}"
    );
}
