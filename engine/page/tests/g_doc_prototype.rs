//! **G_DOC_PROTOTYPE — a `document` method belongs to `Document.prototype`, not to the one document
//! that happened to exist when the shim ran.**
//!
//! Every JS-side `document` shim in this engine was installed as `document.createRange = …`, making
//! it an OWN property of the singleton. A probe counted nineteen. `Document.prototype` is real here
//! (`__protoDocument`, built in Rust) and every document — the singleton, an `<iframe>`'s, and one
//! from `DOMImplementation.createHTMLDocument()` — genuinely inherits from it. The methods simply
//! were not on it, so **the second document in a page got a `TypeError` for the whole family**.
//!
//! The corpus named it, not a spec list: the tick-776 CrUX sweep log carries `TypeError:
//! b.createRange is not a function` out of Google's CSE `dynamic.js`, a direct call on a document
//! that is not the singleton.
//!
//! ## ⚠ WHY THE GATE THAT COVERS THIS GROUND MISSED IT — read before writing another one
//!
//! `G_SECOND_DOCUMENT_IS_REAL` exists for exactly this territory and passes. Its central line is
//!
//! ```js
//! document.createNodeIterator.call(b.ownerDocument || b, d.body, …)
//! ```
//!
//! transcribed — correctly — from DOMPurify, which destructures `createNodeIterator` off the
//! *original* document and `.call`s it with the parsed root's document as `this`. That idiom takes
//! the **function from the singleton** and supplies only the receiver, so it exercises the algorithm
//! over a second document while **never performing the property lookup on it**. It passes for
//! precisely as long as `otherDoc.createNodeIterator` is `undefined`.
//!
//! Two things follow, and both are the point of this gate:
//!
//! * **DOMPurify was never broken by this** — stated plainly, because the tempting version of this
//!   tick's story is that it was. The affected callers are the ones that write `doc.createX(…)`.
//! * **A gate that reaches a method off a known-good receiver and `.call`s it onto the subject has
//!   tested the algorithm and skipped the lookup.** So every claim below calls through the
//!   *subject's own property*.
//!
//! ## What makes this more than a presence check
//!
//! **A `this`-blind promotion would be worse than the throw.** Hanging a shim that internally says
//! `document` off the prototype makes it operate on the WRONG document — a range built from an inert
//! parsed copy would point into the live page, which for a sanitiser is a security property inverted
//! rather than a bug degraded. So the load-bearing claims are the **ownership** ones: the assertions
//! check the identity of the document reached, not merely that a call returned something.
//!
//! The other half is the `G_PROTOTYPE` lesson one interface over: `Document.prototype.createRange =
//! wrapper` used to be a silent no-op, because the wrapper landed on an object no document consulted.
//! That is how every error tracker, ad-blocker and polyfill hooks the DOM, so it is asserted too.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<p id="p">live page text</p>
<script>
  var R = [];
  function t(n, f) { try { R.push(n + ':' + f()); } catch (e) { R.push(n + ':THROW(' + e + ')'); } }
  function fresh() { return document.implementation.createHTMLDocument(''); }

  // ── 1. THE FAMILY IS ON THE PROTOTYPE. Each of these was an own property of the singleton, and
  // each was therefore a TypeError for every other document in the page.
  var FAM = ['createRange','createNodeIterator','createTreeWalker','createAttribute',
             'createAttributeNS','createEvent','evaluate'];
  var offProto = FAM.filter(function (n) { return typeof Document.prototype[n] !== 'function'; });
  R.push('offProto:' + (offProto.length ? offProto.join(',') : 'none'));

  // ── 2. …AND THEY WORK ON A DOCUMENT THAT IS NOT THE SINGLETON. Presence on a prototype and
  // reachability from a second document are different claims; this is the one the page makes.
  var absentOnFresh = FAM.filter(function (n) { return typeof fresh()[n] !== 'function'; });
  R.push('absentOnFresh:' + (absentOnFresh.length ? absentOnFresh.join(',') : 'none'));

  // ── 3. THE LOOKUP, NOT THE ALGORITHM. `d.createNodeIterator(…)` — the subject's OWN property,
  // which is what `G_SECOND_DOCUMENT_IS_REAL`'s `.call(…)` transcription never asks for and what
  // Google's `dynamic.js` did when it threw. Before t776 this was
  // `TypeError: d.createNodeIterator is not a function`.
  t('ownLookup', function () {
    var d = fresh();
    d.body.innerHTML = '<p>hi<img src=x onerror=boom()></p>';
    var it = d.createNodeIterator(d.body, 0xFFFFFFFF, null, false);
    var n, seen = [];
    while ((n = it.nextNode())) { seen.push(n.nodeName); }
    return seen.join(',');
  });

  // ── 4. **OWNERSHIP — the claim that a `this`-blind promotion would fail.** A range made from the
  // fresh document must be rooted in the FRESH document. If `createRange` still closed over the
  // singleton it would return a range over the live page, which is the sanitiser's guarantee turned
  // inside out — and every "does it return a Range" check would still pass.
  t('rangeOwner', function () {
    var d = fresh();
    var r = d.createRange();
    return (r.commonAncestorContainer === d) + ',' + (r.commonAncestorContainer === document);
  });
  t('mainRangeOwner', function () {
    var r = document.createRange();
    return (r.commonAncestorContainer === document);
  });
  // `createAttributeNS` delegates to `createAttribute` — it used to delegate through the captured
  // `document`, so it must not start reaching back to the singleton now that it is shared.
  t('attrNs', function () {
    var a = fresh().createAttributeNS(null, 'data-x');
    return a.name + '/' + a.value;
  });

  // ── 5. THE TREE WALKER ON THE PROTOTYPE IS THE **REAL** ONE. The prelude carries a plain-object
  // fallback and `traversal.js` replaces it; had the fallback stayed an own property of the
  // singleton it would have SHADOWED the real one for the main document — fixing the second
  // document by regressing the first. `previousNode` exists only on the real implementation.
  t('walkerIsReal', function () {
    var w = document.createTreeWalker(document.body, 0xFFFFFFFF, null);
    return (w instanceof TreeWalker) + ',' + (typeof w.previousNode);
  });
  t('iterIsReal', function () {
    return (document.createNodeIterator(document.body, 0xFFFFFFFF, null) instanceof NodeIterator);
  });

  // ── 6. PATCHABILITY — `Document.prototype.X = wrapper` must actually be called. This was a silent
  // no-op, the G_PROTOTYPE failure one interface over, and it is how the whole ecosystem of error
  // trackers and polyfills hooks the DOM.
  t('patch', function () {
    var real = Document.prototype.createRange, hits = 0;
    Document.prototype.createRange = function () { hits++; return real.call(this); };
    document.createRange();
    fresh().createRange();
    Document.prototype.createRange = real;
    return hits;
  });

  document.getElementById('out').textContent = R.join('  ');
</script></body></html>"##;

#[test]
fn document_methods_live_on_document_prototype() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://docproto.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "offProto:none",
            "every promoted method must be an own property of `Document.prototype` — installing it \
             on the singleton is what made it invisible to every other document",
        ),
        (
            "absentOnFresh:none",
            "…and reachable from a document that is NOT the singleton. \
             `DOMImplementation.createHTMLDocument()` inherits from the same prototype, so a method \
             on that prototype is a method every document has",
        ),
        (
            // The root itself comes first — a `NodeIterator` starts *before* its root, so the first
            // `nextNode()` is the root (spec, and Chrome agrees). Asserting the whole sequence rather
            // than a count is what makes this a walk of the FRESH tree and not of some tree.
            "ownLookup:BODY,P,#text,IMG",
            "the created document's OWN `createNodeIterator` property must resolve and walk its own \
             tree. `G_SECOND_DOCUMENT_IS_REAL` proves the same algorithm via \
             `document.createNodeIterator.call(otherDoc, …)`, which takes the function off the \
             singleton and therefore passes while this is `undefined` — the lookup is the claim",
        ),
        (
            "rangeOwner:true,false",
            "⚠ THE OWNERSHIP CLAIM. A range made from the fresh document is rooted in the FRESH \
             document and NOT in the live page. A promotion that ignored `this` would return a range \
             over the live DOM — the sanitiser's guarantee inverted — and would still satisfy every \
             `is it a Range` check",
        ),
        (
            "mainRangeOwner:true",
            "…and the singleton keeps its own behaviour: `document.createRange()` is still rooted in \
             the live document",
        ),
        (
            "attrNs:data-x/",
            "createAttributeNS delegates through `this`, not through a captured `document`, and \
             still yields a detached attr with an empty value",
        ),
        (
            "walkerIsReal:true,function",
            "the prototype must carry the REAL TreeWalker (traversal.js), not the prelude's \
             plain-object fallback — an own-property fallback on the singleton would shadow it and \
             silently downgrade the main document while fixing created ones",
        ),
        (
            "iterIsReal:true",
            "…same for NodeIterator",
        ),
        (
            "patch:2",
            "`Document.prototype.createRange = wrapper` must be CALLED — for the singleton and for a \
             created document. It used to be a silent no-op, which is how every error tracker, \
             ad-blocker and polyfill hooks the DOM",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_DOC_PROTOTYPE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
