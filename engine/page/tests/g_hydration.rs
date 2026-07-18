//! **G_HYDRATION — the primitives every SSR hydrator is built out of keep working.**
//!
//! The lever board lists it as a `?` with the note *"the dominant delivery pattern; fails
//! SILENTLY"*, and both halves matter. Next.js, Nuxt, SvelteKit, Remix and Astro all ship
//! **server-rendered HTML plus a client bundle that attaches to it** rather than building the DOM
//! from scratch. If hydration fails, the page still *looks* right — the SSR markup is there — and
//! nothing works: no button responds, no menu opens, no form validates. That is the worst possible
//! failure shape and exactly why it needs measuring rather than assuming.
//!
//! **Measured, not assumed: all of it already works** (probed this tick). So this is a *pinning*
//! gate — it exists because the failure mode is silent and catastrophic, and nothing else in the
//! wall covers it. A regression here would present as "every modern site looks perfect and nothing
//! on it responds", which is the single hardest report to act on. This is the sixth feature this
//! project assumed missing and found already built (after `localStorage`, `FormData`,
//! `position: sticky`, `IntersectionObserver`, per-glyph font fallback).
//!
//! This does not run React. It exercises the **primitives every hydrator is built out of**, because
//! this project has learned repeatedly that framework failures are bugs in our own primitives
//! (`docs/loop/PROCESS.md`: four of five app-web blockers were ours, not the framework's). Each step
//! is a thing a real hydrator does on every mount:
//!
//! 1. walk the SSR tree by `childNodes` / `nodeType`, including the TEXT nodes
//! 2. read what the server rendered back out (`textContent`, attributes)
//! 3. compare it against what the client would have produced (the mismatch check)
//! 4. attach listeners to the EXISTING nodes rather than replacing them
//! 5. patch only what differs, then prove the attached listeners actually fire

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="root"><div class="app" data-ssr="1"><h1 id="title">Hello</h1><button id="btn" data-count="0">Count: 0</button><ul id="list"><li>a</li><li>b</li></ul></div></div>
<div id="out">-</div>
<script>
  var R = [];
  function log(k, v) { R.push(k + '=' + v); }

  var root = document.getElementById('root');

  // 1. WALK the server markup, text nodes included. A hydrator that cannot see text nodes
  //    cannot match them, and React's #418 "hydration failed" comes from exactly this walk.
  var el = 0, txt = 0;
  (function walk(n) {
    for (var i = 0; i < n.childNodes.length; i++) {
      var c = n.childNodes[i];
      if (c.nodeType === 1) { el++; walk(c); }
      else if (c.nodeType === 3) { txt++; }
    }
  })(root);
  log('els', el);
  log('texts', txt);

  // 2. READ BACK what the server rendered — the attribute + text state hydration diffs against.
  var app = root.firstElementChild;
  log('ssrattr', app.getAttribute('data-ssr'));
  log('title', document.getElementById('title').textContent);
  log('items', document.querySelectorAll('#list li').length);

  // 3. THE MISMATCH CHECK. Every hydrator compares server output to client expectation and
  //    either patches or bails loudly. Both answers must be reachable.
  var btn = document.getElementById('btn');
  var serverText = btn.textContent;
  var clientText = 'Count: ' + btn.getAttribute('data-count');
  log('match', serverText === clientText);

  // 4. ATTACH to the EXISTING node. This is the whole point of hydration: reuse the server's
  //    DOM instead of re-creating it. If listeners silently fail to bind, the page is inert.
  var fired = 0;
  btn.addEventListener('click', function () {
    fired++;
    var n = parseInt(btn.getAttribute('data-count'), 10) + 1;
    btn.setAttribute('data-count', String(n));
    btn.textContent = 'Count: ' + n;
  });
  log('bound', typeof btn.onclick === 'object' || true);

  // 5. PATCH a node in place, the way a hydrator reconciles a difference it found.
  document.getElementById('title').textContent = 'Hydrated';
  log('patched', document.getElementById('title').textContent);

  globalThis.__report = function () {
    document.getElementById('out').textContent = R.join(' ') + ' fired=' + fired;
  };
  __report();
</script></body></html>"##;

#[test]
fn ssr_markup_is_walkable_readable_and_attachable() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://hydrate.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "els=6",
            "a hydrator walks the server tree by childNodes/nodeType. Six elements is the SSR \
             markup exactly; a different count means the parse produced a tree the client bundle \
             would immediately disagree with",
        ),
        (
            "texts=4",
            "TEXT nodes must be visible to the walk. A hydrator that cannot see them cannot match \
             them, which is precisely where React's #418 'hydration failed' comes from",
        ),
        (
            "ssrattr=1",
            "server-rendered attributes must read back — they are the state hydration diffs against",
        ),
        (
            "items=2",
            "querySelectorAll over SSR markup: how every hydrator locates its mount points",
        ),
        (
            "match=true",
            "the mismatch check must be able to conclude MATCH. If server text and client \
             expectation never compare equal, a real hydrator discards the SSR tree and re-renders \
             from scratch on every page load — correct-looking, and the whole benefit is gone",
        ),
        (
            "patched=Hydrated",
            "a node must be patchable IN PLACE, which is how a hydrator reconciles a difference it \
             found rather than replacing the subtree",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_HYDRATION: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }

    // Does the listener attached during hydration actually fire on a real click? A bound-but-dead
    // listener is precisely the "looks right, does nothing" failure this probe exists to catch.
    let btn = manuk_css::query_selector_all(page.dom(), root, "#btn")[0];
    page.dispatch_click(btn, &fonts, 800.0);
    page.eval_for_test("__report()");
    let after = page.dom().text_content(out);
    assert!(
        after.contains("fired=1"),
        "G_HYDRATION: a listener attached to the EXISTING server-rendered node did not fire on a \
         real click.\n  got: {after}\n\n  This is the entire point of hydration — reuse the \
         server's DOM rather than re-creating it. A listener that binds without error and never \
         fires is the worst shape this failure takes: the page LOOKS perfect, because the SSR \
         markup is right there, and no button, menu or form on it does anything."
    );
}
