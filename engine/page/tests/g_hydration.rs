//! **G_HYDRATION — server-rendered markup, then client attach, without rebuilding the DOM.**
//!
//! Hydration is how the overwhelming majority of the modern web is delivered: Next.js, Nuxt, Remix,
//! SvelteKit, Astro, Rails/Phoenix + a sprinkle. The server ships real HTML so the page is visible
//! immediately, and the client framework then **adopts** that existing markup — walking it, checking
//! it matches what it would have rendered, and attaching listeners to the nodes that are already
//! there.
//!
//! **Why the constellation called it a SILENT failure.** Every part of hydration is individually
//! ordinary DOM work, so nothing throws when it goes wrong. The page *looks* right — the server's
//! markup is on screen — and it is simply **dead**: buttons do nothing, menus do not open, forms do
//! not validate. There is no error, no blank screen, no missing API. A capability check that renders
//! a page and looks at it cannot tell hydrated from un-hydrated; only *driving* it can.
//!
//! **So this gate asserts the four things hydration actually needs**, each of which is a real
//! framework behaviour rather than a proxy for one:
//!
//!   1. **The markup is queryable before any script touches it** — the server's DOM is really there.
//!   2. **Node IDENTITY is preserved across attach.** This is the load-bearing one. Hydration means
//!      *adopting* the existing node; a framework that silently re-created it would produce an
//!      identical-looking DOM and throw away the server's work (and, in a real framework, every
//!      listener and scroll position with it). The gate stamps a JS property on the node before
//!      hydrating and requires the same object — `===`, plus the stamp — afterwards.
//!   3. **Listeners attached to server markup fire**, and the handler sees the node's *current*
//!      state.
//!   4. **A hydration mismatch is detectable** — a framework compares server text against what it
//!      would render, and must be able to see the difference to patch it.
//!
//! Measured, then pinned: whatever this reports becomes the constellation's verdict for
//! `hydration (SSR markup + client attach)`, which has been `unknown` since tick 64 rendered React
//! but never drove an attach.

use manuk_text::FontContext;

/// Server-rendered markup, then the client script that adopts it — the shape a framework's
/// `hydrateRoot` performs, written out longhand so the gate tests the ENGINE and not a bundle.
const HTML: &str = r##"<!doctype html>
<html><body>
  <div id="app">
    <h1 id="title">Server Title</h1>
    <button id="btn" data-count="0">Clicked 0 times</button>
    <ul id="list"><li>alpha</li><li>bravo</li></ul>
  </div>
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
      // ── 1. The server's markup is really in the DOM before the client does anything.
      var btn = $('btn');
      R.push('ssr:' + ($('title').textContent === 'Server Title' &&
                       btn.getAttribute('data-count') === '0' &&
                       $('list').children.length === 2));

      // Stamp the pre-hydration nodes. If anything below replaces rather than adopts them, the
      // stamp is gone — which is the whole difference between hydrating and re-rendering.
      btn.__hydrationStamp = 'original';
      var stampedList = $('list');
      stampedList.__hydrationStamp = 'original';

      // ── 2. The attach itself: adopt the existing node, do not create one.
      var state = { count: Number(btn.getAttribute('data-count')) };
      btn.addEventListener('click', function () {
        state.count += 1;
        btn.setAttribute('data-count', String(state.count));
        btn.textContent = 'Clicked ' + state.count + ' times';
      });

      // ── 3. Identity survives. `===` against the node fetched fresh from the document, plus the
      // stamp, which together rule out a look-alike replacement.
      var afterBtn = $('btn');
      R.push('identity:' + (afterBtn === btn && afterBtn.__hydrationStamp === 'original'));
      R.push('listidentity:' + ($('list').__hydrationStamp === 'original'));

      // ── 4. A hydration MISMATCH is detectable: the framework compares the server's text with what
      // it would have rendered, and has to be able to see a difference in order to patch it.
      var serverText = $('title').textContent;
      R.push('mismatch:' + (serverText !== 'Client Title'));
      $('title').textContent = 'Client Title';
      R.push('patched:' + ($('title').textContent === 'Client Title'));

      R.push('ready:true');
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

#[test]
fn server_markup_is_adopted_by_the_client_not_rebuilt() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://ssr.test/", &fonts, 800.0);
    let root = page.dom().root();

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("ssr:true", "the server's markup must be in the DOM before any script runs — that is the entire premise of SSR, and the reason the page is visible early"),
        ("identity:true", "hydration ADOPTS the existing node; a re-created look-alike throws away the server's work and every listener attached to it, while looking identical"),
        ("listidentity:true", "child subtrees are adopted too, not just the node the framework happened to start at"),
        ("mismatch:true", "a framework must be able to SEE that server text differs from what it would render — that comparison is how it decides to patch"),
        ("patched:true", "and it must be able to patch it"),
        ("ready:true", "the whole attach sequence must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_HYDRATION: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }

    // ── The half a script cannot self-report: the listener attached to SERVER markup must actually
    // fire when the engine dispatches a real click. This is what separates a hydrated page from a
    // dead one, and it is exactly what no amount of looking at the rendered page can tell you.
    let btn = manuk_css::query_selector_all(page.dom(), root, "#btn")[0];
    page.dispatch_click(btn, &fonts, 800.0);
    let after = page.dom().text_content(btn);
    assert_eq!(
        after.trim(),
        "Clicked 1 times",
        "a listener attached during hydration must fire on a real engine-dispatched click, and the \
         handler must see current state\n  got: {after}\n\n  \
         This is the silent failure hydration is famous for: the server's markup renders perfectly, \
         so the page LOOKS correct, and every button on it is inert."
    );

    let count = page
        .dom()
        .element(btn)
        .and_then(|e| e.attr("data-count"))
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        count, "1",
        "the handler's attribute write must land on the adopted node — if identity were lost, this \
         would update a detached copy and the visible node would never change"
    );
}
