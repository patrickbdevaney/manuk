//! **G_NAMES — a name is not a string: `DOMTokenList`, name validation, and namespaces.**
//!
//! Three gaps, all silent, all in the same family — *the engine accepted things that are not names, and
//! then produced elements and classes that could never match anything.*
//!
//! * **`classList` was not a `DOMTokenList`.** `classList[0]` was `undefined` (no indexed access at all),
//!   and — the part that costs the most — **the token methods never threw.** `classList.add('btn primary')`
//!   is a bug; the author meant two tokens. A browser that silently writes the single class
//!   `"btn primary"` produces an element matching **neither** selector, with no error anywhere. The spec
//!   throws `InvalidCharacterError` for whitespace and `SyntaxError` for the empty string.
//! * **`createElement('')` and `createElement('<div>')` produced elements.** A perfectly good node with a
//!   nonsense tag, which then matched no selector and rendered nothing — silently. The spec throws
//!   `InvalidCharacterError`, and a page that catches it can recover; a page handed a phantom cannot even
//!   see the problem.
//! * **`createElementNS` threw the namespace away.** It split off the prefix, called `createElement`, and
//!   returned an HTML element. `namespaceURI` then said XHTML for an SVG node, `localName` was
//!   `undefined`, and `tagName` was **uppercased** — which for SVG's `linearGradient` is simply wrong, and
//!   is the single reason `dom/nodes/case.html` scored 7/285.
//!
//! And the correction that cost me: **HTML does not split prefixes.** `document.createElement('a:b')` has
//! `localName === "a:b"` — the colon is just a character. Only a *namespaced* element has a prefix.
//! Splitting unconditionally renamed every HTML element containing a colon and made the score go *down*.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div><div id="e" class="a b"></div>
<script>
  var R = [];
  var e = document.getElementById('e');
  function thrown(f) { try { f(); return 'no-throw'; } catch (x) { return x.name; } }

  // ── DOMTokenList
  R.push('idx:' + e.classList[0]);
  R.push('len:' + e.classList.length);
  R.push('empty:' + thrown(function () { e.classList.add(''); }));
  R.push('space:' + thrown(function () { e.classList.add('a b'); }));
  R.push('intact:' + e.getAttribute('class'));      // the failed calls must not have mutated it
  R.push('toggle:' + e.classList.toggle('c') + ',' + e.classList.contains('c'));
  R.push('live:' + (function () { var c = e.classList; e.className = 'x y z'; return c.length; })());

  // ── createElement validates its name
  R.push('ceEmpty:' + thrown(function () { document.createElement(''); }));
  R.push('ceBad:' + thrown(function () { document.createElement('<div>'); }));
  R.push('ceOk:' + document.createElement('my-widget').localName);
  // HTML does NOT split prefixes — the colon is part of the name.
  R.push('ceColon:' + document.createElement('a:b').localName);

  // ── namespaces are real
  var svg = document.createElementNS('http://www.w3.org/2000/svg', 'linearGradient');
  R.push('ns:' + svg.namespaceURI);
  R.push('nsLocal:' + svg.localName);               // case PRESERVED
  R.push('nsTag:' + svg.tagName);                   // NOT uppercased
  R.push('htmlTag:' + document.createElement('div').tagName);   // …but HTML still is
  R.push('nsPrefix:' + thrown(function () { document.createElementNS(null, 'p:q'); }));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn token_lists_validate_names_validate_and_namespaces_survive() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://names.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("idx:a", "`classList[0]` was `undefined` — there was no indexed access at all"),
        ("len:2", "…and `length` must agree with it"),
        ("empty:SyntaxError", "an empty token is a SyntaxError, per spec"),
        (
            "space:InvalidCharacterError",
            "`classList.add('a b')` is a BUG — the author meant two tokens. Writing the single class \
             \"a b\" produces an element that matches NEITHER selector, with no error anywhere",
        ),
        ("intact:a b", "and a throwing call must not have mutated the attribute — no partial effect"),
        ("toggle:true,true", "toggle returns whether the token is now present"),
        ("live:3", "the list is live over the attribute"),
        ("ceEmpty:InvalidCharacterError", "`createElement('')` produced a perfectly good phantom element"),
        ("ceBad:InvalidCharacterError", "…and so did `createElement('<div>')`"),
        ("ceOk:my-widget", "…while a custom element name is perfectly valid and must not throw"),
        (
            "ceColon:a:b",
            "**HTML DOES NOT SPLIT PREFIXES.** The colon is just a character. Splitting it renamed every \
             HTML element containing one, and made the WPT score go DOWN",
        ),
        ("ns:http://www.w3.org/2000/svg", "createElementNS threw the namespace away and returned XHTML"),
        (
            "nsLocal:linearGradient",
            "a foreign element PRESERVES ITS CASE. `lineargradient` matches nothing",
        ),
        ("nsTag:linearGradient", "…and its tagName is not uppercased either"),
        ("htmlTag:DIV", "…while an HTML element's tagName still is. The rule is per-namespace"),
        ("nsPrefix:NamespaceError", "a prefix with no namespace is a NamespaceError"),
    ] {
        assert!(
            got.contains(claim),
            "G_NAMES: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
