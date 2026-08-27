//! # G_STRUCTURAL_PSEUDOS — the `-of-type` family and `:nth-last-child`, and a dropped selector matches NOTHING
//!
//! **`document.querySelectorAll('em:nth-of-type(3)')` returned an EMPTY LIST.** So did
//! `li:nth-last-child(3n)`, `:first-of-type`, `:last-of-type` and `:only-of-type`. Meanwhile
//! `li:nth-child(2n)` answered correctly — which is precisely what made this invisible for so long:
//! the one member of the family that was implemented worked, so the family looked implemented.
//!
//! `manuk_css`'s `Pseudo` enum carried `NthChild` **alone**. Every sibling of it fell through the
//! parser's `_ => return None` arm, and that arm drops the **whole selector**, not the unknown part.
//! An empty NodeList from a valid selector is the hardest failure to notice, because it is
//! indistinguishable from a page that genuinely has nothing to match.
//!
//! Measured before the fix, on the fixture below's shape:
//!
//! ```text
//!   li:nth-child(2n)         3   ← correct, and the reason nobody looked
//!   em:nth-of-type(3)        0   (Chrome 1)
//!   li:nth-last-child(3n)    0   (Chrome 2)
//!   #p :last-of-type         0   (Chrome 2)
//!   #p :nth-last-of-type(2n) 0   (Chrome 2)
//! ```
//!
//! ## `:first-of-type` is NOT `:first-child`, and the difference is most of real markup
//!
//! In `<p><em>a</em><span>b</span><em>c</em></p>` the `<span>` is **first of its type** and is
//! nobody's first child. A fix that counted all element siblings would pass a fixture whose children
//! are homogeneous — a list of `<li>` — and be wrong on every mixed run of inline content, which is
//! what a paragraph, a card body and a nav bar all are. So the fixture below is deliberately
//! **heterogeneous**, and the `#list` arm exists only as the homogeneous control.
//!
//! ## ONE RULE, TWO IMPLEMENTATIONS — checked here rather than assumed
//!
//! The live cascade is **Stylo's** and has always resolved these pseudos, so
//! `em:nth-of-type(2) { color: … }` *rendered* correctly the whole time while
//! `querySelectorAll('em:nth-of-type(2)')` found nothing. That is this project's recurring shape,
//! and the `cascadeAgrees` claim below asserts the two engines now give the same answer for the same
//! element rather than leaving it to inference.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  em:nth-of-type(2) { color: rgb(1, 2, 3); }
</style></head><body>
  <ul id="list"><li id="l1">1</li><li id="l2">2</li><li id="l3">3</li><li id="l4">4</li><li id="l5">5</li><li id="l6">6</li></ul>
  <p id="p"><em id="e1">a</em><span id="s1">b</span><em id="e2">c</em><span id="s2">d</span><em id="e3">e</em><b id="b1">f</b></p>
  <div id="solo"><i id="i1">only</i></div>
  <div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    function ids(sel) {
      var n = document.querySelectorAll(sel), a = [];
      for (var i = 0; i < n.length; i++) { a.push(n[i].id); }
      return a.length + (a.length ? ':' + a.join(',') : '');
    }

    // ── 1. THE CONTROL. `:nth-child` was the one member that worked; if this ever moves, the fix
    //    broke the thing it was extending rather than adding to it.
    p('nthChildControl:' + ids('#list li:nth-child(2n)'));

    // ── 2. `:nth-last-child` — the same index counted from the END.
    p('nthLastChild2n:' + ids('#list li:nth-last-child(2n)'));
    p('nthLastChild3n:' + ids('#list li:nth-last-child(3n)'));
    p('nthLastChildNeg:' + ids('#list li:nth-last-child(-n+2)'));

    // ── 3. THE `-of-type` FAMILY on HETEROGENEOUS children — the arm a sibling-counting fix fails.
    p('firstOfType:' + ids('#p :first-of-type'));
    p('lastOfType:' + ids('#p :last-of-type'));
    p('onlyOfType:' + ids('#p :only-of-type'));
    p('nthOfType2:' + ids('#p em:nth-of-type(2)'));
    p('nthOfTypeOdd:' + ids('#p em:nth-of-type(odd)'));
    p('nthLastOfType1:' + ids('#p em:nth-last-of-type(1)'));
    p('nthLastOfType2:' + ids('#p :nth-last-of-type(2)'));

    // ── 4. The homogeneous control: with one type, `-of-type` and `-child` must AGREE.
    p('homogeneousAgrees:' + (ids('#list li:first-of-type') === ids('#list li:first-child')));
    p('soloOnlyOfType:' + ids('#solo :only-of-type'));

    // ── 5. The other matcher entry points share the parser.
    p('matchesApi:' + document.getElementById('e2').matches(':nth-of-type(2)'));
    p('matchesLastOfType:' + document.getElementById('s2').matches(':last-of-type'));
    p('closestApi:' + (document.getElementById('e2').closest('p:only-of-type') || {}).id);
    p('closestMiss:' + (document.getElementById('i1').closest('div:only-of-type') || {}).id);

    // ── 6. ONE RULE, TWO IMPLEMENTATIONS. Stylo's cascade always resolved these; the query engine
    //    did not. Both must now name the SAME element.
    p('cascadeAgrees:' + (getComputedStyle(document.getElementById('e2')).color.indexOf('1, 2, 3') >= 0));

    // ── 7. THE RATCHET CLAUSE — nothing that worked before may work less well after.
    p('firstChildStill:' + ids('#p :first-child'));
    p('lastChildStill:' + ids('#p :last-child'));
    p('nthChildEven:' + ids('#p :nth-child(2n)'));
    // ⚠⚠⚠ **t1346 — THIS PROBE USED TO END HERE, SILENTLY.** `ids()` calls `querySelectorAll`,
    // which now THROWS `SyntaxError` on a genuinely invalid selector (the spec's answer, and
    // Chrome's — measured on this fixture: `THREW:SyntaxError`). The throw aborted the script
    // mid-sentence, so `#out` simply stopped one key early and the assertion for a MISSING key
    // reported "expected `unknownStillDrops:0`" while the real fact was that the whole rest of the
    // probe had vanished. An expectation written as an ABSENT value cannot tell "the value is wrong"
    // from "nothing ran".
    p('unknownThrows:' + (function () {
      try { return 'no:' + ids('#p :totally-unknown-pseudo(x)'); } catch (e) { return e.name; }
    })());
  </script>
</body></html>"##;

#[test]
fn the_of_type_family_and_nth_last_child_match_in_the_query_apis() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://structural.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("STRUCTURAL: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_STRUCTURAL_PSEUDOS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "nthChildControl:3:l2,l4,l6",
        "THE CONTROL. `:nth-child` was the one member of the family that already worked. If this \
         moved, the fix rewrote what it was supposed to extend",
    ),
    (
        "nthLastChild2n:3:l1,l3,l5",
        "`:nth-last-child(2n)` counts from the END: l6 is 1st-from-last, l5 is 2nd, so the EVEN \
         ones from the end are l5, l3, l1 — reported in DOCUMENT order. Before this tick the \
         selector was dropped whole and the answer was an empty list",
    ),
    (
        "nthLastChild3n:2:l1,l4",
        "from the end, positions 3 and 6 are l4 and l1",
    ),
    (
        "nthLastChildNeg:2:l5,l6",
        "the `-n+B` form must survive the from-the-end remapping: the last TWO elements",
    ),
    (
        "firstOfType:3:e1,s1,b1",
        "⚠ THE CLAIM A SIBLING-COUNTING FIX FAILS. `:first-of-type` is not `:first-child` — in a \
         mixed run the first `<span>` and the first `<b>` are each first OF THEIR TYPE while being \
         nobody's first child. A fix that counted all element siblings returns just `e1` here and \
         still passes every homogeneous fixture",
    ),
    (
        "lastOfType:3:s2,e3,b1",
        "the last `<span>`, the last `<em>` and the only `<b>` — in document order, which puts s2 \
         before e3",
    ),
    (
        "onlyOfType:1:b1",
        "`<b>` is the only element of its type in `#p`; `<em>` and `<span>` each have siblings of \
         their type and must NOT match",
    ),
    ("nthOfType2:1:e2", "the second `<em>`, counting only `<em>`s"),
    (
        "nthOfTypeOdd:2:e1,e3",
        "the `odd` keyword must reach the of-type counter too, not only `:nth-child`",
    ),
    (
        "nthLastOfType1:1:e3",
        "the last `<em>` addressed from the end of its own type run",
    ),
    (
        "nthLastOfType2:2:s1,e2",
        "second-from-last of EACH type: the `<span>`s give s1, the `<em>`s give e2, and `<b>` has \
         no second-from-last. Document order puts s1 first",
    ),
    (
        "homogeneousAgrees:true",
        "with one element type the `-of-type` and `-child` families must agree exactly — the \
         degenerate case that makes the two look identical, stated so the difference above is not \
         mistaken for a disagreement",
    ),
    ("soloOnlyOfType:1:i1", "a single child is only-of-type"),
    (
        "matchesApi:true",
        "`matches()`, `closest()` and `querySelectorAll` share one parser — a fix that reaches only \
         the query entry point has not fixed the engine",
    ),
    ("matchesLastOfType:true", "the last `<span>` in `#p`"),
    (
        "closestApi:p",
        "`closest()` walks ancestors through the same matcher. `#p` is the only `<p>` among \
         `<body>`'s children, so `p:only-of-type` finds it",
    ),
    (
        "closestMiss:undefined",
        "⚠ AND IT MUST MISS WHEN IT SHOULD. `#solo` is a `<div>` but NOT the only one — `#out` is a \
         `<div>` too — so `div:only-of-type` matches no ancestor and `closest` returns null. A \
         matcher that answered `:only-of-type` by counting all siblings rather than same-type ones \
         would still pass the claim above and fail this",
    ),
    (
        "cascadeAgrees:true",
        "⚠ ONE RULE, TWO IMPLEMENTATIONS. Stylo's cascade resolved `em:nth-of-type(2)` the whole \
         time, so the page RENDERED correctly while `querySelectorAll` found nothing. Both engines \
         must now name the same element or the disagreement has merely moved",
    ),
    (
        "firstChildStill:1:e1",
        "THE RATCHET. `:first-child` is unchanged and must stay the CHILD answer, not the of-type one",
    ),
    ("lastChildStill:1:b1", "THE RATCHET. `:last-child` unchanged"),
    (
        "nthChildEven:3:s1,s2,b1",
        "THE RATCHET. `:nth-child(2n)` on the heterogeneous run counts ALL element siblings — 2nd, \
         4th and 6th — and must not have acquired a type filter",
    ),
    (
        // ⚠⚠⚠ RE-PINNED AT t1346 — see the identical row in `g_is_where_selectors`. An invalid
        // selector now throws `SyntaxError` (spec, and Chrome-measured on this fixture) instead of
        // returning an empty list. Fail-closed either way; the throw is what feature detection reads.
        "unknownThrows:SyntaxError",
        "THE RATCHET, and the mechanism itself: a genuinely unknown pseudo must STILL drop the \
         selector. This gate exists because five known ones were taking that arm",
    ),
];
