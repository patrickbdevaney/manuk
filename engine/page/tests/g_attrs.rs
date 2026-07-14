//! **G_ATTRS — `element.attributes` was `undefined`. Not incomplete: absent.**
//!
//! `element.attributes.length` was a **`TypeError`** — and iterating an element's attributes is one of the
//! most ordinary things a script does. Every DOM serializer. Every DOM-diffing library. Every "copy these
//! attributes across" helper. **DOMPurify walks `attributes` to strip `on*` handlers** — a sanitizer that
//! cannot enumerate attributes cannot sanitize them.
//!
//! Gone with it: `getAttributeNode`, `setAttributeNode`, `document.createAttribute`, and
//! `toggleAttribute` — the idiomatic way to flip `disabled`/`hidden`/`aria-expanded`.
//!
//! **The map is live, for the same reason `HTMLCollection` is** (tick 73):
//!
//! ```js
//! while (el.attributes.length) el.removeAttribute(el.attributes[0].name);   // "strip everything"
//! ```
//!
//! A frozen `length` makes that spin forever. **The same dead-collection hang, one interface over.**
//!
//! And the thing that is easy to get wrong: **an `Attr` is a HANDLE, not a snapshot.** `attr.value = 'x'`
//! must write through to the owner element. Return a plain object and every `attrs[i].value = ...` in the
//! wild silently writes to nothing.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div><div id="e" class="a" data-x="1"></div>
<div id="strip" a="1" b="2" c="3"></div>
<script>
  var R = [];
  var e = document.getElementById('e');

  R.push('len:' + e.attributes.length);
  R.push('idx:' + e.attributes[0].name + '=' + e.attributes[0].value);
  R.push('named:' + e.attributes.getNamedItem('class').value);
  R.push('owner:' + (e.getAttributeNode('class').ownerElement === e));
  R.push('nodeType:' + e.getAttributeNode('class').nodeType);

  // An Attr is a HANDLE. Writing its value writes THROUGH to the element.
  e.getAttributeNode('class').value = 'zzz';
  R.push('writeThrough:' + e.getAttribute('class'));

  // Live: the map moves with the element.
  var m = e.attributes;
  var n0 = m.length;
  e.setAttribute('q', '9');
  R.push('live:' + n0 + '->' + m.length);

  // createAttribute makes a DETACHED Attr; setAttributeNode attaches it.
  var a = document.createAttribute('role');
  a.value = 'button';
  R.push('detached:' + (a.ownerElement === null));
  e.setAttributeNode(a);
  R.push('attached:' + e.getAttribute('role'));

  // toggleAttribute returns whether the attribute is present AFTERWARDS.
  R.push('toggle:' + e.toggleAttribute('hidden') + ',' + e.hasAttribute('hidden'));
  R.push('untoggle:' + e.toggleAttribute('hidden') + ',' + e.hasAttribute('hidden'));

  // BAR 0: the "strip everything" idiom must TERMINATE. A frozen length spins forever.
  var s = document.getElementById('strip');
  var spins = 0;
  while (s.attributes.length && spins < 100) {
    s.removeAttribute(s.attributes[0].name);
    spins++;
  }
  R.push('strip:' + s.attributes.length + ',' + spins);

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn attributes_are_live_attr_nodes_that_write_through() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://attrs.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("len:3", "`element.attributes` was `undefined`, so `.length` was a TypeError"),
        ("idx:id=e", "indexed access into the NamedNodeMap"),
        ("named:a", "getNamedItem"),
        ("owner:true", "an Attr knows its owner element"),
        ("nodeType:2", "an Attr is a node of type 2"),
        (
            "writeThrough:zzz",
            "**an Attr is a HANDLE, not a snapshot.** `attr.value = 'x'` must write through to the \
             element — return a plain object and every `attrs[i].value = ...` in the wild writes to \
             nothing, silently",
        ),
        ("live:3->4", "the map is LIVE, like every other collection"),
        ("detached:true", "createAttribute makes a detached Attr with no owner"),
        ("attached:button", "…and setAttributeNode attaches it"),
        ("toggle:true,true", "toggleAttribute returns whether the attribute is present AFTERWARDS"),
        ("untoggle:false,false", "…and flips it back"),
        (
            "strip:0,4",
            "**BAR 0.** `while (el.attributes.length) el.removeAttribute(el.attributes[0].name)` must \
             TERMINATE — in exactly 4 spins for 4 attributes (id, a, b, c). A frozen `length` spins forever: the same \
             dead-collection hang as tick 73, one interface over",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_ATTRS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
