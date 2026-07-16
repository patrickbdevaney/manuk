//! **G_DOC_COLLECTIONS — the HTMLDocument named-collection accessors.**
//!
//! `document.forms`, `document.images`, `document.links`, `document.scripts`, `document.embeds`/`plugins`,
//! `document.anchors`, and `document.getElementsByName(n)` were **all `undefined`** — so `document.forms.length`
//! was a `TypeError` that takes the rest of a bundle down with it. Every form library enumerates
//! `document.forms`; analytics and prerender scanners walk `document.links`/`images`/`scripts`; legacy code
//! resolves controls with `getElementsByName`.
//!
//! Each is a static Array (like `getElementsByTagName`) in **tree order**. The subtle ones:
//!   * `document.links` is `a` AND `area` **with an `href`** — a bare `<a>` anchor is NOT a link.
//!   * `document.anchors` is `a` **with a `name`**.
//!   * `getElementsByName` matches ANY element type by its `name` content attribute, exact string.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<form id="f1"></form>
<img id="i1"><img id="i2">
<a href="/x" id="l1">link</a>
<a id="anch" name="top">anchor-no-href</a>
<area href="/y" id="l2">
<embed id="e1">
<input name="dup"><select name="dup"></select>
<script>
  var R = [];
  try {
    R.push('forms:' + document.forms.length + ',' + document.forms[0].id);           // 1,f1
    R.push('images:' + document.images.length);                                       // 2
    R.push('links:' + document.links.length + ',' + document.links[0].id + ',' + document.links[1].id); // 2,l1,l2
    R.push('embeds:' + document.embeds.length + ',' + (document.plugins.length === document.embeds.length)); // 1,true
    R.push('anchors:' + document.anchors.length + ',' + document.anchors[0].id);      // 1,anch
    R.push('scripts:' + (document.scripts.length >= 1));                              // true
    var named = document.getElementsByName('dup');
    R.push('named:' + named.length + ',' + named[0].tagName + ',' + named[1].tagName);// 2,INPUT,SELECT
    R.push('nameMiss:' + document.getElementsByName('nope').length);                  // 0
  } catch (e) { R.push('THREW:' + e); }
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn document_named_collections_and_get_elements_by_name() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://dc.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("forms:1,f1", "document.forms is the live collection of <form> elements"),
        ("images:2", "document.images enumerates every <img>"),
        ("links:2,l1,l2", "document.links is a/area WITH href, in tree order (the bare <a name> anchor is excluded)"),
        ("embeds:1,true", "document.embeds enumerates <embed>; document.plugins is a synonym"),
        ("anchors:1,anch", "document.anchors is <a> elements that carry a name attribute"),
        ("scripts:true", "document.scripts enumerates <script> elements"),
        ("named:2,INPUT,SELECT", "getElementsByName matches any element type by its name attr, tree order"),
        ("nameMiss:0", "getElementsByName returns an empty list (not null/throw) when nothing matches"),
    ] {
        assert!(
            got.contains(claim),
            "G_DOC_COLLECTIONS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
