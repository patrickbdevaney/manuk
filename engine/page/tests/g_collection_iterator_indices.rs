//! **G_COLLECTION_ITERATOR_INDICES — `HTMLCollection` iterable surface + numeric `namedItem`.**
//!
//! `HTMLCollection` is NOT a WebIDL `iterable<>` — it has a default `@@iterator` (so `for..of` works)
//! and `item`/`namedItem`, but must NOT carry `values`/`entries`/`keys`/`forEach` (those are the
//! generated members of `NodeList`, which IS `iterable<Node>`). Our shared proxy exposed all four on
//! both. And `namedItem` must coerce its argument to a string, so `namedItem(-2)` finds `id="-2"`
//! (dom/collections/HTMLCollection-iterator + -supported-property-indices).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<p id="-2"></p><p id="-1"></p><p id="p3"></p>
<script>
  var R = [], push = function (k, v) { R.push(k + '=' + v); };
  var ps = document.getElementsByTagName('p');

  // HTMLCollection: iterable via @@iterator, but no values/entries/forEach.
  push('iter', (Symbol.iterator in ps) + ',' + ('values' in ps) + ',' + ('entries' in ps) + ',' + ('forEach' in ps)); // true,false,false,false
  var seen = [];
  for (var el of ps) seen.push(el.id);
  push('forof', seen.join(',')); // -2,-1,p3

  // namedItem coerces its argument to a string → namedItem(-2) matches id="-2".
  push('named', (ps.namedItem(-2) === document.getElementById('-2')) + ',' + (ps[-1] === document.getElementById('-1'))); // true,true

  // NodeList (childNodes) KEEPS the iterable methods.
  var cn = document.body.childNodes;
  push('nodelist', (typeof cn.forEach) + ',' + (typeof cn.entries)); // function,function

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn htmlcollection_iterable_surface_and_numeric_nameditem() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://iter.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("iter=true,false,false,false", "HTMLCollection has @@iterator but NOT values/entries/forEach (not a WebIDL iterable<>)"),
        ("forof=-2,-1,p3", "for..of over the collection yields the elements in order"),
        ("named=true,true", "namedItem coerces to string so namedItem(-2)/coll[-1] resolve id=\"-2\"/\"-1\""),
        ("nodelist=function,function", "NodeList IS iterable<Node> and keeps forEach/entries"),
    ] {
        assert!(
            got.contains(claim),
            "G_COLLECTION_ITERATOR_INDICES: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
