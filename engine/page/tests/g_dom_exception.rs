//! **G_DOM_EXCEPTION — the JS-side validation throws are REAL `DOMException`s, not decorated `Error`s.**
//!
//! A whole class of DOM validation errors — `classList.add('a b')`, `createAttribute('')`,
//! `setAttributeNS(ns, '')`, `Range.setStart(node, 999)` — is specified to throw a `DOMException` with a
//! specific *name* AND a specific legacy *numeric `code`*. Several of ours threw a plain `Error` with only
//! `e.name` decorated on: no `.code`, and `e.constructor === Error`, not `DOMException`.
//!
//! That is not cosmetic. WPT's `assert_throws_dom` — which a very large fraction of `dom/` uses — checks
//! **both** `'code' in e && e.code == expected` **and** `e.constructor === DOMException`. A decorated
//! `Error` fails both, so ~420 `dom/` subtests reported the right *name* and still failed. More to the
//! point, real code does `catch (e) { if (e.code === DOMException.SYNTAX_ERR) … }` and
//! `if (e instanceof DOMException)` — a decorated `Error` silently takes the wrong branch.
//!
//! This gate pins the three properties a decorated `Error` cannot satisfy: `instanceof DOMException`,
//! the numeric `.code`, and `.constructor === DOMException`.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out" class="x">-</div><script>
    var R = [];
    var box = document.getElementById('out');

    // A helper that records name/code/ctor/instanceof for a thrown exception, in one token.
    function probe(label, fn) {
      try { fn(); R.push(label + ':NOTHROW'); }
      catch (e) {
        R.push(label + ':' + e.name
          + '|code=' + e.code
          + '|isDE=' + (e instanceof DOMException)
          + '|ctorDE=' + (e.constructor === DOMException));
      }
    }

    // classList: whitespace token → InvalidCharacterError (code 5); empty token → SyntaxError (code 12).
    probe('clsWs',    function () { box.classList.add('a b'); });
    probe('clsEmpty', function () { box.classList.add(''); });

    // createAttribute('') → InvalidCharacterError (code 5).
    probe('createAttr', function () { document.createAttribute(''); });

    // setAttributeNS with an empty qualified name → InvalidCharacterError (code 5).
    probe('setAttrNS', function () { box.setAttributeNS(null, '', 'v'); });

    // A Range offset past the node's length → IndexSizeError (code 1).
    probe('rangeIdx', function () { var r = document.createRange(); r.setStart(box, 999); });

    // The document survives every one of those throws (no partial mutation, no corruption).
    R.push('intact:' + (document.getElementById('out') === box));
    R.push('clsUnchanged:' + (box.className === 'x'));  // clsWs threw before touching the set
    box.textContent = R.join(' ');
  </script></body></html>"#;

#[test]
fn validation_throws_are_real_domexceptions_with_code_and_constructor() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://domexc.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "clsWs:InvalidCharacterError|code=5|isDE=true|ctorDE=true",
        "clsEmpty:SyntaxError|code=12|isDE=true|ctorDE=true",
        "createAttr:InvalidCharacterError|code=5|isDE=true|ctorDE=true",
        "setAttrNS:InvalidCharacterError|code=5|isDE=true|ctorDE=true",
        "rangeIdx:IndexSizeError|code=1|isDE=true|ctorDE=true",
        "intact:true",
        "clsUnchanged:true",
    ] {
        assert!(
            got.contains(claim),
            "G_DOM_EXCEPTION: expected `{claim}`\n  got: {got}\n\n  \
             DOM validation errors must be REAL DOMExceptions: `assert_throws_dom` (and real \
             `e.code`/`instanceof DOMException` branches) check the numeric `.code` and \
             `.constructor === DOMException`, both of which a decorated plain `Error` fails."
        );
    }
}
