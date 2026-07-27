//! # G_XPATH_SUBSET — XPath answers correctly over a documented subset, and REFUSES the rest
//!
//! **The failure this gate exists for.** htmx 2.0.4 was completely dead:
//! `ReferenceError: XPathEvaluator is not defined`, thrown during its own evaluation, so
//! `window.htmx` was never defined. It builds one expression at module top level —
//! `(new XPathEvaluator).createExpression('.//*[@*[ starts-with(name(), "hx-on:") or … ]]')` — to
//! find every element carrying an `hx-on:` attribute.
//!
//! ## Half of this gate asserts that it THROWS, and that is the design
//!
//! Tick 641 gave the EME interfaces existence while granting nothing, because *"no key system is
//! supported"* is a truthful answer. **XPath has no such refusal.** An evaluator either returns the
//! right nodes or it lies, and the caller cannot tell. A stub returning an empty node-set would
//! make htmx boot and then silently fail to wire up every `hx-on:` handler on the page — **strictly
//! worse than the ReferenceError**, which at least said something was wrong.
//!
//! So the contract is: **correct over a documented subset, `SyntaxError` outside it.** The refusal
//! claims (`count()`, unions, named axes, `position()`, arithmetic) are what make the positive
//! claims trustworthy — without them, "it returned some nodes" is not evidence of anything.
//!
//! ## The load-bearing positive claim is htmx's own expression
//!
//! `hxFinds` runs the real thing against a fixture holding two matching elements and one
//! non-matching one. Finding *two* is the claim; a stub that returned everything would find three,
//! and one that returned nothing would find zero. Both are caught.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! | the attribute axis drops its inner predicate (the "returns everything" stub shape) | **RED — `hxFinds:3:BUTTON,SPAN,I`.** The plain `<i>` joins the set, which is exactly the failure a stub produces and exactly what the count in this claim exists to catch |
//! | `bad()` returns an empty node-set instead of throwing | **NOT RUN.** Two attempts were killed mid-build by the harness and the mutation was reverted; the expectation (the blanket `RESOLVED-BAD` assertion fires) is REASONED, not measured. Recorded as unrun rather than written up as a probe, because a tabulated mutation that did not actually run is the failure t633 paid for |

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <div id="app">
    <button id="b1" hx-on:click="void 0">x</button>
    <span data-hx-on:click="1">y</span>
    <i id="plain">z</i>
  </div>
  <ul id="list"><li class="a">one</li><li class="b">two</li><li class="a">three</li></ul>
  <div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    function nodes(res) { var o = [], n; while ((n = res.iterateNext())) { o.push(n.nodeName); } return o; }

    // ── 1. htmx's OWN expression, verbatim.
    var ex = (new XPathEvaluator).createExpression(
      './/*[@*[ starts-with(name(), "hx-on:") or starts-with(name(), "data-hx-on:") ]]');
    var got = nodes(ex.evaluate(document.getElementById('app')));
    p('hxFinds:' + got.length + ':' + got.join(','));

    // ── 2. The ordinary shapes real code uses.
    p('byTag:' + nodes(document.evaluate('//li', document, null, 0)).length);
    p('byAttr:' + nodes(document.evaluate('//*[@id]', document, null, 0)).length);
    p('byAttrVal:' + nodes(document.evaluate("//li[@class='a']", document, null, 0)).length);
    p('byPos:' + (document.evaluate('//li[2]', document, null, 0).iterateNext() || {}).textContent);
    p('rel:' + nodes(document.evaluate('.//li', document.getElementById('list'), null, 0)).length);
    p('notFn:' + nodes(document.evaluate("//li[not(@class='a')]", document, null, 0)).length);
    p('contains:' + nodes(document.evaluate("//*[contains(name(), 'utton')]", document, null, 0)).length);
    p('star:' + (nodes(document.evaluate('//*', document, null, 0)).length > 5));

    // ── 3. The XPathResult surface a caller actually reads.
    var r = document.evaluate('//li', document, null, 7);
    p('snapLen:' + r.snapshotLength + ' snap0:' + (r.snapshotItem(0) || {}).textContent);
    p('single:' + (document.evaluate('//li', document, null, 9).singleNodeValue || {}).textContent);

    // ── 4. THE REFUSALS. These are what make everything above trustworthy.
    function refuses(src) {
      try { document.evaluate(src, document, null, 0); return 'RESOLVED-BAD'; }
      catch (e) { return (e && e.name) ? e.name : 'threw'; }
    }
    p('noCount:' + refuses('count(//li)'));
    p('noUnion:' + refuses('//li | //i'));
    p('noAxis:' + refuses('ancestor::div'));
    p('noPosition:' + refuses('//li[position()=1]'));
    p('noArith:' + refuses('//li[1+1]'));
  </script>
</body></html>"##;

#[test]
fn xpath_answers_its_subset_and_refuses_the_rest() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://xpath.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("XPATH SUBSET: {got}");

    assert!(
        !got.contains("RESOLVED-BAD"),
        "an expression outside the supported subset must THROW, never return a node-set. Returning \
         `[]` for XPath the engine cannot parse is worse than the ReferenceError this replaced: htmx \
         would boot and then silently wire up no handlers at all.\n  got: {got}"
    );

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_XPATH_SUBSET: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "hxFinds:2:BUTTON,SPAN",
        "THE LOAD-BEARING CLAIM — htmx 2.0.4's own expression, verbatim, against a fixture with two \
         matching elements and one non-matching. TWO is the answer: a stub returning everything \
         finds three, one returning nothing finds zero, and both are caught here. Without this, \
         htmx does not define `window.htmx` at all",
    ),
    ("byTag:3", "`//li` — the commonest shape there is"),
    (
        "byAttr:5",
        "`//*[@id]` — an attribute-existence predicate, which is the shape htmx's outer test is",
    ),
    ("byAttrVal:2", "`//li[@class='a']` — attribute VALUE comparison, two of the three li"),
    ("byPos:two", "`//li[2]` — a positional predicate is 1-based and selects the second"),
    (
        "rel:3",
        "a RELATIVE path (`.//li`) from a context node rather than the document — htmx evaluates \
         relative to an element, so this is its actual calling convention",
    ),
    ("notFn:1", "`not()` inverts, leaving the one li that is not class 'a'"),
    ("contains:1", "`contains(name(), …)` over the node name"),
    ("star:true", "`//*` walks the whole tree rather than one level"),
    ("snapLen:3 snap0:one", "the SNAPSHOT surface: length and indexed access"),
    ("single:one", "and `singleNodeValue` for FIRST_ORDERED_NODE_TYPE"),
    (
        "noCount:SyntaxError",
        "THE HONESTY HALF. `count()` is outside the subset and must THROW. An evaluator that \
         answered `[]` for expressions it cannot parse would make every caller silently wrong, and \
         a caller cannot tell a wrong node-set from a right one — which is exactly why the EME \
         precedent (interfaces that exist and grant nothing) does NOT transfer here",
    ),
    ("noUnion:SyntaxError", "`|` unions are not implemented, so they are refused rather than guessed"),
    (
        "noAxis:SyntaxError",
        "named axes (`ancestor::`) are refused at the tokenizer, before anything can be returned",
    ),
    ("noPosition:SyntaxError", "`position()` is not in the subset"),
    ("noArith:SyntaxError", "and arithmetic in a predicate"),
];
