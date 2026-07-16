//! **G_NODE_CONSTRUCTORS — `new Text()`, `new Comment()`, `new DocumentFragment()` mint real nodes.**
//!
//! These three DOM interfaces are **constructable**, not merely `instanceof` targets. The engine's
//! generic `iface()` helper gave every interface an *inert* constructor returning an empty `{}` — correct
//! for the un-constructable ones (Element, Node) but wrong for these, where the spec mints a real detached
//! node owned by the current document. So `new Text('x')` returned `{data: undefined, nodeType:
//! undefined}` — a dead object every library that builds nodes with `new Text()` silently got. Each
//! assertion is one spec guarantee the real constructors restore:
//!
//! * **`new Text(data)`** → a text node (`nodeType` 3) whose `data` is the argument (default `""`),
//!   `instanceof Text` and `instanceof Node`, owned by `document`.
//! * **`new Comment(data)`** → a comment node (`nodeType` 8).
//! * **`new DocumentFragment()`** → an empty fragment (`nodeType` 11) that can hold appended children.
//!
//! Own binary: two SpiderMonkey-backed `Page::load`s in one process reuse the JS runtime and can trip the
//! tracked reflector-teardown UAF (see the flexbox-relayout Bar-0 note). One JS gate = one process.
//!
//! **Falsifiable:** with the inert `iface` constructor, `new Text('hi').data` was `undefined` and
//! `.nodeType` `undefined`, so the very first assertion's `#out` read the `-` sentinel — RED. The real
//! constructors (delegating to `document.create*`) turn it GREEN.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
  var R = [];
  function ck(l, g) { R.push(l + ':' + g); }

  var t = new Text('hi');
  ck('tData', t.data);
  ck('tType', t.nodeType);
  ck('tInstText', t instanceof Text);
  ck('tInstNode', t instanceof Node);
  ck('tOwner', t.ownerDocument === document);
  ck('tDefault', JSON.stringify(new Text().data));   // no-arg default is the empty string

  var c = new Comment('x');
  ck('cData', c.data);
  ck('cType', c.nodeType);
  ck('cInst', c instanceof Comment);

  var f = new DocumentFragment();
  ck('fType', f.nodeType);
  ck('fInst', f instanceof DocumentFragment);
  f.appendChild(new Text('a'));
  f.appendChild(document.createElement('span'));
  ck('fKids', f.childNodes.length);

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn node_constructors_mint_real_detached_nodes() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://nc.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("tData:hi", "new Text(data) sets .data to the argument"),
        ("tType:3", "a Text node is nodeType 3"),
        ("tInstText:true", "new Text() is instanceof Text"),
        ("tInstNode:true", "a Text node is a Node"),
        (
            "tOwner:true",
            "the node is owned by the constructing document",
        ),
        (
            "tDefault:\"\"",
            "new Text() with no argument defaults data to the empty string",
        ),
        ("cData:x", "new Comment(data) sets .data"),
        ("cType:8", "a Comment node is nodeType 8"),
        ("cInst:true", "new Comment() is instanceof Comment"),
        ("fType:11", "a DocumentFragment is nodeType 11"),
        (
            "fInst:true",
            "new DocumentFragment() is instanceof DocumentFragment",
        ),
        ("fKids:2", "a constructed fragment holds appended children"),
    ] {
        assert!(
            got.contains(claim),
            "G_NODE_CONSTRUCTORS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
