//! **G_FOREIGN_CONTENT_NS — inline `<svg>` and `<math>` subtrees carry their REAL namespace.**
//!
//! Found while pinning the SVG row at tick 602: every element inside an inline `<svg>` was in the
//! **XHTML** namespace. The `<svg>` element itself was right, which is exactly why it read as "SVG
//! namespaces mostly work" — **the defect started one level down.**
//!
//! ## Why one wrong answer disabled a whole parser mode
//!
//! html5ever implements the tree builder's SVG/MathML *foreign content* mode correctly. It decides
//! it is in that mode by asking the sink for the **current node's qualified name** — and
//! `TreeSink::elem_name` was hardcoded to `ns!(html)` for every element. So the tree builder could
//! never observe that it was inside an `<svg>`, never switched modes, and every descendant was
//! built as HTML. `create_element` then discarded `name.ns` on top of that.
//!
//! Foreign-content mode is not only about `namespaceURI`: it also drives **attribute-name
//! adjustment** (`viewBox` staying camel-cased rather than lowercasing to `viewbox`,
//! `xlink:href`), self-closing tag handling, and the HTML-breakout rules. One wrong answer from a
//! four-line function turned all of that off.
//!
//! ## Why it matters, in the specific shape that makes it worse than a missing feature
//!
//! `document.createElementNS(SVG_NS, 'rect')` **does** keep its namespace, and has since t125. So a
//! page that builds SVG *in script* and a page that ships the same SVG *in markup* produced **two
//! different DOMs for the same tree**. Every library that branches on `namespaceURI`, matches an
//! `svg|rect` selector, or asks `instanceof SVGElement` got the right answer for one half of the web
//! and the wrong one for the other — and nothing anywhere reported a disagreement.
//!
//! That is the claim `parsedEqMade` exists for: not "is the namespace right" but **"do the two ways
//! of getting there agree"**, which is the property a library actually depends on.
//!
//! ⚠ Residue, measured and NOT fixed here: SVG child elements still report CSS-box geometry from
//! `getBoundingClientRect` rather than user-space geometry, and `getBBox`/`ownerSVGElement` are
//! absent — so charting code that measures nodes still fails. That is SVG *layout*, a subsystem, and
//! it is named rather than half-built.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<svg id="s" width="200" height="100" viewBox="0 0 200 100">
  <rect id="r" width="10" height="10"/>
  <g id="g"><circle id="c" r="5"/></g>
</svg>
<math id="m"><mi id="mi">x</mi></math>
<div id="d">plain</div>
<div id="out">-</div>
<script>
  var R = [], ns = function(id){
    var e = document.getElementById(id);
    return e ? String(e.namespaceURI).replace('http://www.w3.org/','') : 'MISSING';
  };
  ['s','r','g','c','m','mi','d'].forEach(function(id){ R.push(id + '=' + ns(id)); });
  var made = document.createElementNS('http://www.w3.org/2000/svg','rect');
  R.push('parsedEqMade=' + (document.getElementById('r').namespaceURI === made.namespaceURI));
  // Foreign content also preserves camel-cased attribute names.
  R.push('viewBox=' + (document.getElementById('s').getAttribute('viewBox') !== null));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn an_inline_svg_subtree_is_in_the_svg_namespace() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fc.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("FOREIGN CONTENT: {got}");

    for (claim, why) in [
        (
            "s=2000/svg",
            "the `<svg>` element itself — this was ALREADY right, and it is why the bug read as \
             'namespaces mostly work'. It is asserted first precisely so the claims below cannot be \
             mistaken for it",
        ),
        (
            "r=2000/svg",
            "**THE DEFECT**: a `<rect>` inside an inline `<svg>` is an SVG element. It was XHTML, \
             because `TreeSink::elem_name` answered `ns!(html)` for every element and html5ever \
             therefore never entered foreign-content mode",
        ),
        ("g=2000/svg", "…and a `<g>`"),
        (
            "c=2000/svg",
            "…and a `<circle>` NESTED inside that `<g>` — two levels down, which is where a fix that \
             only special-cased direct children of `<svg>` would fail",
        ),
        ("m=1998/Math/MathML", "MathML is the same mode and the same bug"),
        (
            "mi=1998/Math/MathML",
            "…including ITS children, which is the half that was broken",
        ),
        (
            "d=1999/xhtml",
            "**THE GUARD**: an ordinary `<div>` must stay XHTML. Without it, a 'fix' that put \
             everything in the SVG namespace would pass every claim above",
        ),
        (
            "parsedEqMade=true",
            "**THE CLAIM THAT MATTERS TO A LIBRARY.** `createElementNS(SVG_NS,'rect')` has kept its \
             namespace since t125, so parsed SVG and scripted SVG produced TWO DIFFERENT DOMs for \
             the same tree. Every library branching on `namespaceURI`, matching `svg|rect`, or \
             asking `instanceof SVGElement` was right about one half of the web and wrong about the \
             other, with nothing reporting the disagreement",
        ),
        (
            "viewBox=true",
            "foreign-content mode also preserves CAMEL-CASED attribute names — `viewBox` must not \
             lowercase to `viewbox`. It is the same switch, so it is the same bug, and asserting it \
             here keeps the fix from being narrowed to `namespaceURI` alone",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_FOREIGN_CONTENT_NS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
