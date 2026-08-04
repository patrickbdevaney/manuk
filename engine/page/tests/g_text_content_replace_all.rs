//! **G_TEXT_CONTENT_REPLACE_ALL — `node.textContent = ''` must leave the node CHILDLESS.**
//!
//! ⚠⚠⚠ **AN EMPTY TEXT NODE WHERE THERE SHOULD BE NOTHING, AND IT DESTROYS jQUERY'S ELEMENT
//! FACTORY.** The DOM standard's "string replace all" puts the empty case first — *"Let node be null.
//! **If string is not the empty string**, set node to a new Text node whose data is string…"* — so
//! clearing a node creates nothing. We created a Text node unconditionally, and `el.textContent = ''`
//! is the single most common way any page empties a subtree.
//!
//! **The consequence is not a count.** `jQuery.parseHTML` → `buildFragment` finishes like this:
//!
//! ```js
//!   tmp.innerHTML = wrap[1] + html + wrap[2];
//!   jQuery.merge( nodes, tmp.childNodes );
//!   tmp = fragment.firstChild;  tmp.textContent = "";
//!   fragment.textContent = "";                          // <- HERE
//!   while ( ( elem = nodes[ i++ ] ) ) { fragment.appendChild( elem ); }
//! ```
//!
//! One leftover empty Text node and the fragment comes back `[#text, <div>]` instead of `[<div>]`, so
//! **`$('<div class="x"/>')[0]` is a TEXT NODE** — and `$('<div>')` is *the* jQuery element-creation
//! idiom, on a large fraction of the web.
//!
//! Measured live on `beb88run.xyz`, the top site of t888's crossing cohort (t895 unblocked its
//! cross-origin AJAX so Slick reaches `buildOut` at all):
//!
//! ```text
//!   $slides.wrapAll('<div class="slick-track"/>').parent()
//!     -> wrapAll takes .eq(0)  ==  the empty TEXT node
//!     -> descends firstElementChild (null on a text node, so it stays there)
//!     -> .append(this) moves every slide INTO the text node
//!     -> 458 boxes — the whole carousel subtree — gone
//!     -> div.banner-carousel  [0 146 1185x0]   (Chrome: [0 146 1185x380])
//! ```
//!
//! ⚠ **One rule, two implementations, and only one was wrong.** `innerHTML = ''` goes through
//! `set_inner_html`, which parses an empty string into no children and was correct all along — which
//! is exactly why this survived: the idiom sitting next to it behaves properly, so probing either one
//! alone exonerates the pair. Both are asserted below.
//!
//! **Every expectation is Chrome's, captured from a real `google-chrome --headless --dump-dom` run of
//! this fixture**, including the two coercions that are easy to assume backwards: `null` *and*
//! `undefined` both clear (`0` children), while `0` and `false` write the strings `"0"` and `"false"`.
//!
//! **Proven RED**: restore the unconditional `create_text` + `append_child` and `frag-empty`,
//! `el-empty`, `el-emptyfresh`, `el-null`, `el-undefined` and `jq-buildFragment` all fail — the last
//! being the one with teeth, since it is jQuery's own code path transcribed.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
  var R = [];
  var p = function (k, v) { R.push(k + '=' + v); };
  var shape = function (n) {
    var o = [];
    for (var i = 0; i < n.childNodes.length; i++) {
      var c = n.childNodes[i];
      o.push(c.nodeType + ':' + c.nodeName + (c.nodeType === 3 ? '["' + c.data + '"]' : ''));
    }
    return n.childNodes.length + '{' + o.join(',') + '}';
  };

  // ── A DocumentFragment: the exact node jQuery's buildFragment clears.
  var f = document.createDocumentFragment();
  f.appendChild(document.createElement('div'));
  f.textContent = '';
  p('frag-empty', shape(f));
  var f2 = document.createDocumentFragment();
  f2.appendChild(document.createElement('div'));
  f2.textContent = 'x';
  p('frag-nonempty', shape(f2));

  // ── An Element, populated and fresh.
  var e = document.createElement('div');
  e.innerHTML = '<b>a</b><i>b</i>';
  e.textContent = '';
  p('el-empty', shape(e));
  p('el-empty-tc', JSON.stringify(e.textContent));
  var e2 = document.createElement('div');
  e2.innerHTML = '<b>a</b>';
  e2.textContent = 'x';
  p('el-nonempty', shape(e2));
  var e3 = document.createElement('div');
  e3.textContent = '';
  p('el-emptyfresh', shape(e3));

  // ── The sibling rule that was ALREADY right, asserted so a "fix" cannot break it.
  var e4 = document.createElement('div');
  e4.innerHTML = '<b>a</b>';
  e4.innerHTML = '';
  p('el-innerHTML-empty', shape(e4));

  // ── Coercion, Chrome's answers. null/undefined clear; 0 and false do NOT.
  var mk = function (v) {
    var x = document.createElement('div');
    x.innerHTML = '<b>a</b>';
    x.textContent = v;
    return shape(x);
  };
  p('el-null', mk(null));
  p('el-undefined', mk(undefined));
  p('el-zero', mk(0));
  p('el-false', mk(false));

  // ── THE FAILING CODE ITSELF, transcribed from jQuery 3.7.1's buildFragment. This is the claim with
  //    teeth: every row above could be individually right and this one still wrong.
  var frag = document.createDocumentFragment();
  var tmp = frag.appendChild(document.createElement('div'));
  tmp.innerHTML = '<div class="track"/>';
  var nodes = [].slice.call(tmp.childNodes);
  tmp = frag.firstChild; tmp.textContent = '';
  frag.textContent = '';
  for (var i = 0; i < nodes.length; i++) { frag.appendChild(nodes[i]); }
  p('jq-buildFragment', shape(frag));
  p('jq-first', frag.firstChild ? frag.firstChild.nodeName : 'NULL');

  // ── …and what jQuery's wrapAll then does with it: descend firstElementChild and append. A text
  //    node has none, so the slides land inside it and the subtree is destroyed.
  var host = document.createElement('div');
  host.innerHTML = '<div class="s">A</div><div class="s">B</div>';
  var slides = [].slice.call(host.childNodes);
  var wrapper = frag.firstChild;
  while (wrapper && wrapper.firstElementChild) { wrapper = wrapper.firstElementChild; }
  p('wrap-target', wrapper ? wrapper.nodeName : 'NULL');
  if (wrapper && wrapper.nodeType === 1) {
    for (var j = 0; j < slides.length; j++) { wrapper.appendChild(slides[j]); }
    p('wrap-kids', wrapper.childNodes.length);
  } else {
    p('wrap-kids', 'UNREACHABLE-target-is-not-an-element');
  }

  // ── The MutationObserver must not be told a node was added when none was.
  var seen = [];
  var host2 = document.createElement('div');
  host2.innerHTML = '<b>a</b>';
  document.body.appendChild(host2);
  var mo = new MutationObserver(function (recs) {
    for (var k = 0; k < recs.length; k++) { seen.push(recs[k].addedNodes.length + '/' + recs[k].removedNodes.length); }
  });
  mo.observe(host2, { childList: true });
  host2.textContent = '';
  Promise.resolve().then(function () {
    p('mo-records', seen.join(','));
    document.getElementById('out').textContent = R.join(' ');
  });
</script>
</body></html>"##;

#[test]
fn setting_text_content_to_the_empty_string_leaves_no_child() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://tc.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("TEXTCONTENT REPLACE-ALL: {got}");

    for (claim, why) in [
        // ── The rule.
        (
            "frag-empty=0{}",
            "THE DEFECT: a DocumentFragment cleared with `textContent = ''` must hold NOTHING — this \
             is the exact node jQuery's buildFragment clears",
        ),
        (
            "el-empty=0{}",
            "the same rule on an Element, which is where every page's `clear this list` lives",
        ),
        (
            "el-emptyfresh=0{}",
            "…and on an ALREADY-empty node, so a fix that only skips the append when children \
             existed still fails here",
        ),
        (
            "frag-nonempty=1{3:#text[\"x\"]}",
            "THE GUARD: a NON-empty string must still create exactly one Text node. Skipping the \
             append unconditionally would pass every claim above and break every page",
        ),
        ("el-nonempty=1{3:#text[\"x\"]}", "the same guard on an Element"),
        (
            "el-empty-tc=\"\"",
            "and the GETTER still reads the empty string — a childless node's textContent is '', \
             not null",
        ),
        // ── The sibling that was already right.
        (
            "el-innerHTML-empty=0{}",
            "`innerHTML = ''` was ALREADY correct (it parses to no children) — asserted so the pair \
             cannot drift apart again. One rule, two implementations, and only one was wrong",
        ),
        // ── Coercion, measured against Chrome.
        (
            "el-null=0{}",
            "`textContent = null` CLEARS — the IDL is a nullable string with LegacyNullToEmptyString, \
             so writing the literal 'null' is the plausible wrong answer",
        ),
        ("el-undefined=0{}", "Chrome clears for `undefined` too, not the string 'undefined'"),
        (
            "el-zero=1{3:#text[\"0\"]}",
            "…but `0` is NOT empty — a falsiness test instead of an emptiness test fails exactly here",
        ),
        ("el-false=1{3:#text[\"false\"]}", "and neither is `false`"),
        // ── The failing call.
        (
            "jq-buildFragment=1{1:DIV}",
            "THE ACTUAL BROKEN PATH, transcribed from jQuery 3.7.1: an extra empty Text node makes \
             `$('<div class=\"x\"/>')[0]` a TEXT NODE, which is how Slick's wrapAll moved a \
             458-element carousel subtree into a text node on beb88run.xyz",
        ),
        (
            "jq-first=DIV",
            "the node `.eq(0)` hands to wrapAll — a `#text` here is the whole failure",
        ),
        (
            "wrap-target=DIV",
            "wrapAll descends firstElementChild to find its append target; a text node has none, so \
             it stays on the text node and the slides vanish",
        ),
        ("wrap-kids=2", "…and both slides land inside the real wrapper"),
        // ── The observer.
        (
            "mo-records=0/1",
            "a MutationObserver must be told ZERO nodes were added, not one. Telling an observer a \
             node arrived when none did is the same lie one level up",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_TEXT_CONTENT_REPLACE_ALL: missing `{claim}`\n  — {why}\n  got: {got}"
        );
    }
}
