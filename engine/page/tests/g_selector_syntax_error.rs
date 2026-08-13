//! # G_SELECTOR_SYNTAX_ERROR — an invalid selector THROWS, and a valid one we cannot match does NOT
//!
//! **`document.querySelectorAll('[')` returned an empty NodeList.** So did `querySelector('div,')`,
//! `matches('::example')` and `closest('^|div')`. All four are specified to throw a `SyntaxError`
//! `DOMException`, and the gap is not pedantry:
//!
//! > **try/catch around a selector is how the web feature-detects selector support.**
//! > `try { document.querySelector(':has(x)') } catch (e) { /* fall back */ }` is the idiom. An
//! > engine that never throws answers *"supported"* for **every** selector — including the ones it
//! > silently cannot match — so the library takes the modern branch and gets an empty list forever.
//!
//! Same shape as jQuery's `support.cors` (t1183) and tippy's brand check: **ask what a library
//! BELIEVES, not what it can detect.**
//!
//! ## THE HALF THAT IS EASY TO GET CATASTROPHICALLY WRONG
//!
//! The tempting implementation is *"throw when the matcher's parser returns `None`"*. It is one line
//! and it is a **capability regression**, because that parser returns `None` for two unrelated
//! reasons:
//!
//! | selector | valid? | correct answer |
//! |---|---|---|
//! | `p::first-line` | **yes** — a real pseudo-element we do not model | empty list, **no throw** |
//! | `div:hover` | **yes** | empty list, no throw |
//! | `::example` | **no** — unknown pseudo-element | **throw** |
//!
//! Throwing on the first two turns *"we don't implement that yet"* into **an exception inside the
//! page's own script**, which is strictly worse than the empty list it had. So the `MUST NOT THROW`
//! half of this gate is the load-bearing half, and it is asserted first.
//!
//! ⚠ And `:has()` is the sharpest case: Stylo's *servo* build rejects `:has()` at parse
//! (`parse_has() -> false`), which is precisely why this engine hand-rolled its own supplement.
//! Delegating validity to Stylo — the obvious "use the real parser" move — would make
//! `querySelector(':has(.x)')` **throw**, deleting a shipped capability. `hasStillMatches` below is
//! that trap, written down as an assertion.
//!
//! ## THE VALIDATOR IS CALIBRATED AGAINST WPT'S OWN CORPUS, NOT AGAINST TASTE
//!
//! `dom/nodes/selectors.js` ships **34 invalid** selectors and **207 valid** ones. The
//! implementation was run against both lists until it scored 34/34 and 0/207 false positives, and
//! three of its rules exist only because that corpus refused an earlier draft:
//!
//! * an **unclosed `[` or `(` at end of input is VALID** — CSS closes an open block at EOF, so
//!   `[align="center"` and `::slotted(foo` are in WPT's *valid* list;
//! * **escapes are identifier characters** — `.foo\:bar` is one class name, not a class and a pseudo;
//! * `:nth-child()`'s argument is **An+B, not a selector list**, and recursing into it rejected
//!   `:nth-child(3n)` — eight valid entries, and zebra striping across the whole web.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <div id="d"><span class="x">s</span></div>
  <div id="out">-</div>
  <script>
    var R = [];
    function t(label, fn) {
      try { R.push(label + '=OK(' + fn() + ')'); }
      catch (e) { R.push(label + '=THROW:' + e.name + '/' + (e instanceof DOMException) + '/' + e.code); }
      document.getElementById('out').textContent = R.join(' ');
    }

    // ── 1. THE LOAD-BEARING HALF, FIRST: a VALID selector must never throw, whether or not this
    //    engine can match it. A false throw lands inside the page's own script.
    t('firstLineNoThrow', function () { return document.querySelectorAll('p::first-line').length; });
    t('hoverNoThrow',     function () { return document.querySelectorAll('div:hover').length; });
    t('hasStillMatches',  function () { return document.querySelectorAll('div:has(.x)').length; });
    t('nthNoThrow',       function () { return document.querySelectorAll('li:nth-child(2n+1)').length; });
    t('unclosedNoThrow',  function () { return document.querySelectorAll('[align="center"').length; });
    t('escapedNoThrow',   function () { return document.querySelectorAll('.foo\\:bar').length; });
    t('anyNsNoThrow',     function () { return document.querySelectorAll('*|div').length; });
    t('plainStillWorks',  function () { return document.querySelectorAll('#d .x').length; });

    // ── 2. THE CUT. Each is in WPT's own invalid list, and each names a DIFFERENT rule.
    t('bracket',      function () { return document.querySelectorAll('[').length; });
    t('trailingComma',function () { return document.querySelector('div,') ? 1 : 0; });
    t('unknownPE',    function () { return document.getElementById('d').matches('::example'); });
    t('badNamespace', function () { return (document.getElementById('d').closest('^|div') || {}).id; });
    t('unknownPC',    function () { return document.querySelectorAll('div:example').length; });
    t('badClass',     function () { return document.querySelectorAll('.5cm').length; });
    t('relative',     function () { return document.querySelectorAll('>*').length; });
    t('empty',        function () { return document.querySelectorAll('').length; });
    t('nestedNs',     function () { return document.querySelectorAll(':not(ns|div)').length; });
    t('badCombinator',function () { return document.querySelectorAll('div ++ address, p').length; });
    t('starAttr',     function () { return document.querySelectorAll('[*=test]').length; });
  </script>
</body></html>"##;

#[test]
fn an_invalid_selector_throws_a_syntaxerror_and_a_valid_one_never_does() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://selsyntax.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SELECTOR-SYNTAX: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_SELECTOR_SYNTAX_ERROR: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }

    // ── The claims above check each arm's LABEL. This asserts the EXACT exception shape, so a wrong
    //    exception type cannot pass by merely being an exception. `assert_throws_dom` checks the
    //    name, the `instanceof` and the legacy numeric code; real code checks the last.
    //
    //    ⚠ ONE `#[test]` in this file on purpose — a second `Page::load` running JS in the same
    //    process SIGSEGVs on the SpiderMonkey seam, so this is folded in rather than split out.
    for label in [
        "bracket",
        "trailingComma",
        "unknownPE",
        "badNamespace",
        "unknownPC",
        "badClass",
        "relative",
        "empty",
        "nestedNs",
        "badCombinator",
        "starAttr",
    ] {
        let want = format!("{label}={THROWS}");
        assert!(
            got.contains(&want),
            "G_SELECTOR_SYNTAX_ERROR: `{label}` must throw `{THROWS}`, the full shape — the name, \
             `instanceof DOMException` AND the legacy numeric `SYNTAX_ERR` code.\n  got: {got}"
        );
    }
}

/// `THROW:SyntaxError/true/12` — the name, `instanceof DOMException`, and the legacy numeric
/// `SYNTAX_ERR` code, because WPT's `assert_throws_dom` checks all three and real code checks the
/// last one.
const THROWS: &str = "THROW:SyntaxError/true/12";

const CLAIMS: &[(&str, &str)] = &[
    (
        "firstLineNoThrow=OK(0)",
        "⚠ THE LOAD-BEARING CLAIM, and the one the obvious implementation fails. `p::first-line` is \
         a VALID selector this engine does not model. It must return an empty list. Throwing here \
         converts 'unimplemented' into an exception in the page's own script — strictly worse than \
         the empty list it had, and exactly what 'throw when the matcher returns None' would do",
    ),
    (
        "hoverNoThrow=OK(0)",
        "`:hover` is valid and unmatched in a static render — no throw",
    ),
    (
        "hasStillMatches=OK(1)",
        "⚠⚠ THE TRAP THIS GATE EXISTS TO PIN. Stylo's *servo* build rejects `:has()` at parse, which \
         is why this engine hand-rolled its own supplement. Delegating validity to Stylo — the \
         obvious 'use the real parser' move — would make this THROW and delete a shipped capability \
         used by 13% of the corpus. It must still MATCH, not merely not-throw",
    ),
    (
        "nthNoThrow=OK(0)",
        "`:nth-child()`'s argument is An+B, NOT a selector list. An earlier draft recursed into it \
         and rejected `:nth-child(3n)` — eight of WPT's valid entries, and zebra striping across the \
         whole web",
    ),
    (
        "unclosedNoThrow=OK(0)",
        "CSS closes an open block at END OF INPUT rather than invalidating it, so `[align=\"center\"` \
         is VALID — it is in WPT's valid list, and rejecting it was this validator's first false \
         positive",
    ),
    (
        "escapedNoThrow=OK(0)",
        "`.foo\\:bar` is ONE class name. A validator that stops the identifier at the backslash \
         reads a class followed by an unknown pseudo `bar` and throws on four valid WPT entries",
    ),
    ("anyNsNoThrow=OK(0)", "`*|div` needs no namespace declaration"),
    (
        "plainStillWorks=OK(1)",
        "THE RATCHET. The ordinary selector every page uses must be unaffected",
    ),
    ("bracket=", "`[` — an empty attribute selector after EOF-closing"),
    ("trailingComma=", "`div,` — an EMPTY list member is a syntax error, and it is the one case a 'split and filter empties' parser cannot see"),
    ("unknownPE=", "`::example` — an unknown pseudo-ELEMENT is invalid, not merely unsupported"),
    ("badNamespace=", "`^|div` — `^` is not an identifier, so the namespace prefix is malformed"),
    ("unknownPC=", "`div:example` — an unknown pseudo-CLASS is invalid"),
    ("badClass=", "`.5cm` — a CSS identifier may not start with a digit"),
    ("relative=", "`>*` — a leading combinator; a relative selector is not a selector"),
    ("empty=", "the empty string is not a selector"),
    (
        "nestedNs=",
        "⚠ `:not(ns|div)` is invalid for what is INSIDE it. A validator that only balanced the \
         parentheses called this valid — the single case the first implementation missed against \
         WPT's list, and the reason the functional pseudos recurse",
    ),
    ("badCombinator=", "`div ++ address` — two combinators in a row"),
    ("starAttr=", "`[*=test]` — `*` is not an attribute name"),
];
