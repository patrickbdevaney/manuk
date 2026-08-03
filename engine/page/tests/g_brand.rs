//! **G_BRAND — `Object.prototype.toString.call(node)` must NAME the interface, not say `Object`.**
//!
//! ⚠⚠⚠ **THIS IS A WHOLE-PAGE WIPE, MEASURED, NOT A COSMETIC READ.** `www.otomoto.pl` is
//! server-rendered — its ~1,300-tag document arrives complete over the wire — and this engine scored
//! it `render-failed` with coverage **0.4%** and a BLANK screenshot in nine consecutive certification
//! sweeps. The whole chain, read out of the page's own console (t862):
//!
//! ```text
//!   {}.toString.call(div)  ==  "[object Object]"       (Chrome: "[object HTMLDivElement]")
//!     -> tippy.js  isElement(t) = str.indexOf('Element]') > -1   ->  FALSE
//!     -> tippy() returns its ARRAY of instances instead of instances[0]
//!     -> TypeError: can't access property "popperOptions", r.props is undefined
//!     -> TypeError: r.destroy is not a function
//!     -> React error boundary -> Next.js "client-side exception" -> renders /_error
//!     -> THE SERVER-RENDERED DOM IS TORN DOWN AND REPLACED WITH NOTHING
//! ```
//!
//! The brand check is the oldest duck-typing idiom on the web — tippy, lodash's `isElement`, jQuery,
//! every `isPlainObject`, every structured serializer. `[object Object]` is not an absence a caller
//! routes around; it is a **wrong answer of the right type**, which is the shape that gets believed.
//!
//! **The expectations below are Chrome's, captured from a real `chromium --dump-dom` run of this
//! exact fixture**, not recalled — including the two that are easy to get backwards: `<my-thing>` is
//! `HTMLElement` (a valid custom-element name) while `<out>` is `HTMLUnknownElement`, and `document`
//! is `HTMLDocument`, not `Document`.
//!
//! **Proven RED**: before the fix every single row read `[object Object]`, and `tippy-isElement`
//! read `false`.
//!
//! ⚠ Two rows deliberately assert a COARSER answer than Chrome's, so that a later tick which makes
//! them exact does not read as a regression here: an SVG element we do not name individually brands
//! `SVGElement` (Chrome: `SVGSVGElement`), which is still a true statement about it.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="d"></div><a id="a"></a><my-thing id="u"></my-thing><out id="o"></out>
<svg id="svg"><path id="p"/></svg>
<table id="t"><tr id="tr"><td id="td">x</td></tr></table>
<input id="i"><h2 id="h2"></h2>
<div id="out">-</div>
<script>
  var T = function (x) { return Object.prototype.toString.call(x); };
  var R = [];
  var row = function (k, x) { R.push(k + '=' + T(x)); };
  var $ = function (id) { return document.getElementById(id); };

  row('div',      $('d'));
  row('a',        $('a'));
  row('custom',   $('u'));      // <my-thing> — a valid custom-element name
  row('unknown',  $('o'));      // <out> — not an HTML element, and no dash
  row('td',       $('td'));     // ONE interface over two tags
  row('tr',       $('tr'));
  row('input',    $('i'));
  row('h2',       $('h2'));     // ONE interface over six tags
  row('svgpath',  $('p'));
  row('text',     $('td').firstChild);
  row('comment',  document.createComment('c'));
  row('document', document);
  row('doctype',  document.doctype);
  row('frag',     document.createDocumentFragment());
  row('window',   window);
  row('elproto',  Element.prototype);

  // THE FAILING CALL ITSELF, transcribed from tippy.js. This is the assertion with teeth: every
  // brand above could be individually wrong in some new way and this one would still catch a
  // regression that costs a real page its DOM.
  var isType = function (value, type) {
    var str = {}.toString.call(value);
    return str.indexOf('[object') === 0 && str.indexOf(type + ']') > -1;
  };
  R.push('tippy-isElement=' + isType($('d'), 'Element'));
  R.push('tippy-notElement=' + isType({}, 'Element'));   // and it must still say NO to a plain object

  // The other door onto the same question.
  R.push('ctorname=' + $('d').constructor.name);
  R.push('ctorIsHTMLDivElement=' + ($('d').constructor === HTMLDivElement));
  R.push('plainCtor=' + ({}).constructor.name);          // untouched: still Object

  $('out').textContent = R.join(' ');
</script>
</body></html>"##;

#[test]
fn a_dom_node_brands_itself_with_its_interface_name() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://brand.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("BRAND RESULT: {got}");

    for (claim, why) in [
        // ── Chrome's answers, captured from `chromium --dump-dom` on this fixture.
        ("div=[object HTMLDivElement]", "the tag the whole failure chain starts at"),
        ("a=[object HTMLAnchorElement]", "a second tag, so a hard-coded `HTMLDivElement` fails"),
        (
            "custom=[object HTMLElement]",
            "a name containing `-` is a valid custom-element name and gets plain HTMLElement",
        ),
        (
            "unknown=[object HTMLUnknownElement]",
            "…and an undefined name WITHOUT a dash is HTMLUnknownElement. Backwards is the one way \
             this can be actively wrong",
        ),
        (
            "td=[object HTMLTableCellElement]",
            "ONE interface over two tags — a per-tag map that only handles 1:1 fails here",
        ),
        ("tr=[object HTMLTableRowElement]", "its sibling, so the table row is not the cell's answer"),
        ("input=[object HTMLInputElement]", ""),
        (
            "h2=[object HTMLHeadingElement]",
            "ONE interface over six tags, via a different helper than <td>'s",
        ),
        ("svgpath=[object SVGPathElement]", "an SVG tag we DO name individually"),
        ("text=[object Text]", "nodeType 3 — not an element, and not `Object` either"),
        ("comment=[object Comment]", "nodeType 8"),
        (
            "document=[object HTMLDocument]",
            "Chrome says HTMLDocument, not Document — `Document` here would be the plausible wrong \
             answer",
        ),
        ("doctype=[object DocumentType]", "nodeType 10"),
        (
            "frag=[object DocumentFragment]",
            "nodeType 11 with no host — a shadow root shares the nodeType and must not share this",
        ),
        ("window=[object Window]", "the global is not in the DOM chain and needs its own brand"),
        (
            "elproto=[object Element]",
            "the PROTOTYPE objects have undefined reserved slots, so they cannot be branded from \
             nodeType and are matched by identity",
        ),
        // ── The failing call, verbatim from tippy.js.
        (
            "tippy-isElement=true",
            "THE ACTUAL BROKEN CALL: tippy's isElement() decides whether tippy() returns an instance \
             or an ARRAY, and the array is what kills otomoto.pl's whole DOM",
        ),
        (
            "tippy-notElement=false",
            "and it must still answer NO for a plain object — a brand that says yes to everything is \
             not a brand",
        ),
        // ── The same question through `constructor`.
        ("ctorname=HTMLDivElement", "`node.constructor.name` used to be the string \"Object\""),
        ("ctorIsHTMLDivElement=true", "and it must be the interface OBJECT, not just its name"),
        (
            "plainCtor=Object",
            "…while a plain object's constructor is untouched. This is what fails if the accessor \
             was installed on Object.prototype instead of the DOM chain's root",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_BRAND: expected `{claim}`{}\n  got: {got}\n\n  \
             `Object.prototype.toString.call(node)` must name the node's interface. Answering \
             `[object Object]` is a WRONG answer of the RIGHT TYPE: tippy.js's isElement() reads it, \
             concludes the element is not an element, returns an array where the caller expects an \
             instance, and React tears down the server-rendered DOM. Measured on www.otomoto.pl \
             (t862): coverage 0.4%, a blank page, for nine consecutive sweeps.",
            if why.is_empty() {
                String::new()
            } else {
                format!(" — {why}")
            }
        );
    }
}
