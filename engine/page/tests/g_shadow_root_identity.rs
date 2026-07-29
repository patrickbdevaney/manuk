//! **G_SHADOW_ROOT_IDENTITY — a shadow root could not say what it was, whose it was, or that it was
//! closed.**
//!
//! Chrome-measured on one fixture, three of fifteen shadow-DOM rows wrong:
//!
//! ```text
//!                                        CHROME    BEFORE    AFTER
//!   root.mode                             open    undefined   open
//!   root.host === theElement              true      FALSE     true
//!   closedHost.shadowRoot === null        true      FALSE     true
//! ```
//!
//! ⚠⚠ **`host` was wrong because it is TWO properties sharing one name.** On an `<a>`/`<area>` it is
//! URL decomposition (`hostname:port`); on a **ShadowRoot** it is the element the shadow is attached
//! to — the most-used property on a shadow root, since it is how a component reaches its own host
//! from inside. They share a reflector surface here, so the URL getter answered for both and
//! `root.host === el` was `false`. Resolved by node kind, shadow root first: a shadow root is never
//! an `<a>`, so the cases cannot overlap and `g_anchor_url_setters` still passes.
//!
//! ⚠⚠ **`closed` now reads `null`, superseding a deliberate earlier position** whose reasoning was
//! nearly right and is worth keeping: *"hiding it is a follow-on and would only obscure the page from
//! itself."* True about secrecy — `closed` is an encapsulation contract, not a security boundary, and
//! nothing here is protected by it. But the property is **observable**, and libraries BRANCH on it:
//! `el.shadowRoot === null` is the standard test for *"is this root closed / not mine?"*. Answering
//! with the root sends that branch down a path that works **here and nowhere else**, which is the
//! worse failure — the kind that only appears in production, on a real browser.
//!
//! ⚠ **Named residue, measured in the same run and NOT fixed** (each its own change): `root.getElementById`
//! is `undefined` (Chrome: `function`), `activeElement` is absent from the root, a second
//! `attachShadow` returns the existing root where Chrome throws `NotSupportedError`, and a **composed
//! event is not retargeted** — `event.target` inside a listener on `document` reads the inner node
//! where Chrome reads the HOST. That last one leaks shadow internals to every outside listener and is
//! the largest of the four.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
 <div id="h">light</div>
 <div id="closed">c</div>
 <a id="link" href="https://example.com:8080/p?q=1">a</a>
 <div id="out">-</div>
 <script>
   window.addEventListener('load', function () {
     function T(n, f) { try { return n + '=' + f(); } catch (e) { return n + '=' + e.name; } }
     var h = document.getElementById('h');
     var root = h.attachShadow({ mode: 'open' });
     root.innerHTML = '<b id="in">x</b>';
     var c = document.getElementById('closed');
     c.attachShadow({ mode: 'closed' });
     document.getElementById('out').textContent = [
       T('mode', function () { return root.mode; }),
       T('host', function () { return root.host === h; }),
       T('shadowRoot', function () { return h.shadowRoot === root; }),
       T('nodeType', function () { return root.nodeType; }),
       T('closedNull', function () { return c.shadowRoot === null; }),
       // The shadow tree is real, not a decoration: content put in it is reachable, and the light
       // DOM it replaced is untouched.
       T('inner', function () { return root.querySelector('#in').textContent; }),
       T('light', function () { return h.textContent.indexOf('light') >= 0; }),
       // The collision control: `host` on an <a> must STILL be the URL authority.
       T('anchorHost', function () { return document.getElementById('link').host; }),
       // …and `mode` is not invented for elements that are not shadow roots.
       T('elMode', function () { return String(h.mode); }),
       // ── RETARGETING (t739). A listener OUTSIDE the shadow tree must see the HOST as
       //    `event.target`; a listener ON the root still sees the inner node. Without it, a
       //    component leaks its internals to every outside listener and the ordinary delegation
       //    test `event.target.closest('.item')` searches a tree it is not in.
       T('retarget', function () {
         var r = {};
         var inner = root.querySelector('#in');
         document.addEventListener('rt', function (e) { r.out = e.target.id; });
         root.addEventListener('rt', function (e) { r.root = e.target.id; });
         inner.addEventListener('rt', function (e) { r.at = e.target.id; });
         inner.dispatchEvent(new Event('rt', { bubbles: true, composed: true }));
         return r.out + '/' + r.root + '/' + r.at;
       }),
       // …and the SECOND HALF: `composedPath()` must stay FULL for the outside listener, which it
       //    cannot be if it is derived from a retargeted `target`.
       T('outPath', function () {
         var p = null;
         var inner2 = root.querySelector('#in');
         document.addEventListener('rt2', function (e) {
           p = e.composedPath().map(function (n) {
             return n === window ? 'window' : (n === document ? 'document' : (n.id || n.nodeName));
           }).join('>');
         });
         inner2.dispatchEvent(new Event('rt2', { bubbles: true, composed: true }));
         return p;
       })
     ].join(' ');
   });
 </script>
</body></html>"#;

fn out(page: &manuk_page::Page) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn a_shadow_root_knows_its_mode_its_host_and_when_to_hide() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://shadow.test/",
        &fonts,
        800.0,
    ));
    let got = out(&page);
    println!("SHADOW-ROOT {got}");
    let has = |s: &str| got.contains(s);

    // (1) **`mode`.** RED: remove the `mode` property → `undefined`, which is neither "open" nor
    // "closed" and defeats every library that branches on it.
    assert!(
        has("mode=open"),
        "shadowRoot.mode must report the mode it was created with — got {got:?}"
    );

    // (2) **`host` is the HOST ELEMENT on a shadow root.** RED: drop the shadow-root branch → the URL
    // getter answers and `host=false`, which is what shipped.
    assert!(
        has("host=true") && has("shadowRoot=true") && has("nodeType=11"),
        "shadowRoot.host must be the hosting ELEMENT (and the round-trip must hold) — got {got:?}"
    );

    // (3) **THE COLLISION CONTROL.** `host` on an `<a>` is still the URL authority. A fix that made
    // `host` mean "shadow host" everywhere satisfies (2) and breaks every anchor on the web.
    assert!(
        has("anchorHost=example.com:8080"),
        "`host` on an <a> must still be the URL authority `hostname:port` — got {got:?}"
    );
    assert!(
        has("elMode=undefined"),
        "`mode` must not be invented for elements that are not shadow roots — got {got:?}"
    );

    // (4) **`closed` hides the root.** RED: return the root regardless of mode → `closedNull=false`,
    // and `el.shadowRoot === null` — the standard "is this closed / not mine?" test — sends the
    // caller down a path that works here and nowhere else.
    assert!(
        has("closedNull=true"),
        "a `closed` root must read `null` from element.shadowRoot — got {got:?}"
    );

    // (6) **RETARGETING.** RED: drop the `targets` table → `retarget=in/in/in`, and every outside
    // listener sees the component's internals. Chrome measures `h/in/in`.
    assert!(
        has("retarget=h/in/in"),
        "a listener OUTSIDE the shadow tree must see the HOST as event.target, while one ON the root \
         still sees the inner node — got {got:?}"
    );

    // (7) **THE SECOND HALF, and it is why this is one change and not two.** `composedPath()` is
    // derived from `this.target` unless it is captured at dispatch — so retargeting ALONE silently
    // hands the outside listener a SHORTER path than Chrome does. RED: remove the capture → the
    // path starts at `h` instead of `in`, and (6) still passes.
    assert!(
        has("outPath=in>#document-fragment>h>BODY>HTML>document>window"),
        "composedPath() must stay the FULL composed path for a listener outside the shadow tree, \
         even though its `target` was retargeted — got {got:?}"
    );

    // (5) **The tree is real.** Without this, every assertion above could hold over an object that
    // hosts nothing.
    assert!(
        has("inner=x") && has("light=true"),
        "the shadow tree must hold content, and the light DOM must be untouched — got {got:?}"
    );
}
