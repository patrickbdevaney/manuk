//! **G_RANGE — `Range` is real: it compares, it extracts across structure, and it puts things back.**
//!
//! `Range` was an **inert stub**. It sat in the interface list, so `typeof Range === 'function'` was
//! `true` — which is precisely why nobody noticed it did nothing. `document.createRange()` did not exist
//! at all. `dom/ranges` scored **2 of 200**.
//!
//! It is worth doing for both horizons at once, which is rare:
//!
//! * **far** — ~198 WPT subtests sit behind this one interface;
//! * **near** — every rich-text editor, every text selection, every copy/paste path and every
//!   `contenteditable` surface is built on it. `Range` is *the* primitive for "a span of the document
//!   between two points".
//!
//! **The assertions that matter are the ones with structure in them.** A `Range` that only handles a
//! single flat `Text` node is easy and useless: the whole point of the API is a span that *starts in the
//! middle of one paragraph and ends in the middle of the next*, and the spec's extract algorithm is
//! fiddly exactly there — the partially-contained ends must be **split**, keeping the outer halves in
//! place, while fully-contained nodes move wholesale. The naive version (move whole nodes) passes a flat
//! test and mangles every real document.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="host"><p id="p1">Hello world</p><p id="p2">Second para</p><p id="p3">Third</p></div>
<div id="surr">wrap me</div>
<script>
  var R = [];
  var host = document.getElementById('host');
  var p1 = document.getElementById('p1'), p2 = document.getElementById('p2');
  var t1 = p1.firstChild, t2 = p2.firstChild;

  // ── 1. It exists, and it is not the stub.
  R.push('create:' + (typeof document.createRange === 'function'));
  var r = document.createRange();
  R.push('proto:' + (r instanceof Range));

  // ── 2. Boundary points — the algorithm everything else is built from.
  r.setStart(t1, 6);            // "Hello |world"
  r.setEnd(t1, 11);             // "Hello world|"
  R.push('str:' + r.toString());
  R.push('collapsed:' + r.collapsed);
  R.push('sc:' + (r.startContainer === t1) + ',' + r.startOffset);
  R.push('common:' + r.commonAncestorContainer.nodeType);   // 3 = the text node itself

  // ── 3. Setting the start PAST the end must collapse the range onto the new point. This is the spec,
  //       and it is not an edge case — it is how a backwards selection gets built.
  var c = document.createRange();
  c.setStart(t1, 5); c.setEnd(t1, 8);
  c.setStart(t1, 9);
  R.push('recollapse:' + c.collapsed + ',' + c.startOffset + ',' + c.endOffset);

  // ── 4. compareBoundaryPoints across DIFFERENT nodes — pure document-order arithmetic.
  var a = document.createRange(); a.setStart(t1, 0); a.setEnd(t1, 5);
  var b = document.createRange(); b.setStart(t2, 0); b.setEnd(t2, 6);
  R.push('cmp:' + a.compareBoundaryPoints(Range.START_TO_START, b));   // a starts before b → -1

  // ── 5. isPointInRange / intersectsNode.
  var span = document.createRange();
  span.setStart(t1, 3); span.setEnd(t2, 3);
  R.push('inRange:' + span.isPointInRange(t1, 8));
  R.push('intersects:' + span.intersectsNode(p2));

  // ── 6. THE ONE THAT MATTERS: extract ACROSS STRUCTURE.
  //
  // From the middle of p1 to the middle of p2. Both paragraphs are only PARTIALLY contained, so both
  // must be SPLIT: p1 keeps "Hel", p2 keeps "ond para", and the fragment carries "lo world" + "Sec"
  // wrapped in clones of their paragraphs. A naive implementation that moves whole nodes produces
  // something that looks right on flat text and destroys every document with structure.
  var x = document.createRange();
  x.setStart(t1, 3);            // "Hel|lo world"
  x.setEnd(t2, 3);              // "Sec|ond para"
  var frag = x.extractContents();
  R.push('fragKids:' + frag.childNodes.length);                 // two paragraph clones
  R.push('fragText:' + frag.textContent);                        // "lo worldSec"
  R.push('p1After:' + document.getElementById('p1').textContent);
  R.push('p2After:' + document.getElementById('p2').textContent);
  R.push('xCollapsed:' + x.collapsed);

  // ── 7. insertNode splits the text node it lands inside.
  var p3 = document.getElementById('p3'), t3 = p3.firstChild;
  var ins = document.createRange();
  ins.setStart(t3, 2); ins.collapse(true);
  var mark = document.createElement('b');
  mark.textContent = 'X';
  ins.insertNode(mark);
  R.push('inserted:' + p3.textContent);                          // "ThXird"

  // ── 8. surroundContents — how every "bold the selection" button in the world works.
  var s = document.getElementById('surr'), ts = s.firstChild;
  var sr = document.createRange();
  sr.setStart(ts, 0); sr.setEnd(ts, 4);   // "wrap"
  var em = document.createElement('em');
  sr.surroundContents(em);
  R.push('surround:' + s.innerHTML);

  // ── 9. createContextualFragment — how sanitizers and jQuery.parseHTML turn a string into nodes.
  var cr = document.createRange();
  cr.selectNodeContents(document.body);
  var f = cr.createContextualFragment('<p class="ccf">hi</p><span>x</span>');
  R.push('ccfKids:' + f.childNodes.length);                      // 2 top-level nodes
  R.push('ccfType:' + (f.nodeType));                             // 11 = DocumentFragment
  R.push('ccfText:' + f.textContent);                            // "hix"
  R.push('ccfTag:' + f.firstChild.nodeName.toLowerCase());       // "p"
  var threw = false;
  try { cr.createContextualFragment(); } catch (e) { threw = (e.name === 'TypeError'); }
  R.push('ccfThrow:' + threw);                                   // required arg → TypeError

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn range_compares_extracts_across_structure_and_puts_things_back() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://range.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("create:true", "`document.createRange()` did not exist at all"),
        ("proto:true", "and the `Range` in scope was the inert stub from the interface list"),
        ("str:world", "toString over a single text node"),
        ("collapsed:false", "a range with content is not collapsed"),
        ("sc:true,6", "the boundary point is kept exactly as set"),
        (
            "recollapse:true,9,9",
            "setting the START past the END must COLLAPSE onto the new point. Not an edge case — it is \
             how a backwards selection is built",
        ),
        (
            "cmp:-1",
            "compareBoundaryPoints across different nodes is pure document-order arithmetic, and every \
             other Range method reduces to it",
        ),
        ("inRange:true", "isPointInRange"),
        ("intersects:true", "intersectsNode"),
        (
            "fragKids:2",
            "EXTRACT ACROSS STRUCTURE. Both paragraphs are only partially contained, so both must be \
             SPLIT — the fragment carries a clone of each",
        ),
        (
            "fragText:lo worldSec",
            "…and it carries exactly the text between the two boundary points, no more",
        ),
        (
            "p1After:Hel",
            "the start paragraph keeps its OUTER half. A naive implementation moves whole nodes: it \
             passes on flat text and destroys every document that has structure",
        ),
        ("p2After:ond para", "…and the end paragraph keeps its outer half"),
        ("xCollapsed:true", "after an extract there is nothing between the points, so the range collapses"),
        ("inserted:ThXird", "insertNode SPLITS the text node it lands inside"),
        (
            "surround:<em>wrap</em> me",
            "surroundContents — this is how every \"bold the selection\" button in the world works",
        ),
        (
            "ccfKids:2",
            "createContextualFragment parses HTML into a DocumentFragment — sanitizers and \
             jQuery.parseHTML route through it; it was entirely absent",
        ),
        ("ccfType:11", "…and the result is a DocumentFragment (nodeType 11), not a stray element"),
        ("ccfText:hix", "…carrying the parsed subtree's text"),
        ("ccfTag:p", "…with the elements actually parsed, not stringified"),
        (
            "ccfThrow:true",
            "calling it with no argument is a TypeError (a required WebIDL arg), not a parse of \
             the string \"undefined\"",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_RANGE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
