//! **G_COLLECTIONS — a live `HTMLCollection`, and the infinite loop a dead one causes.**
//!
//! `element.children` and `getElementsByTagName()` returned **plain arrays** — a snapshot, taken once.
//! Append a child and the collection's `length` did not move. `dom/collections` scored **3/48**.
//!
//! **That is a Bar 0 hang, not merely a conformance gap**, and it hides in the most common DOM idiom
//! there is:
//!
//! ```js
//! while (el.children.length) { el.removeChild(el.firstChild); }   // "empty this element"
//! ```
//!
//! With a *live* collection this terminates: each removal shortens it. With a **dead** one, `length` is
//! frozen at its initial value, the condition is true forever, and **the tab locks up.** A dead collection
//! does not fail loudly — it *spins*. The `drain:` assertion below is that test, and it is the reason this
//! gate exists.
//!
//! It landed cheaply for one reason worth recording: **tick 64 gave the DOM real prototypes**, so
//! `children` could be *wrapped* rather than reimplemented. Before that tick, patching the prototype did
//! nothing at all, silently. Second capability to land almost free on the back of that one fix.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="h"><p id="p1">a</p><p id="p2">b</p></div>
<div id="drain"><i></i><i></i><i></i><i></i><i></i></div>
<script>
  var R = [];
  var h = document.getElementById('h');

  R.push('isHC:' + (h.children instanceof HTMLCollection));
  R.push('item:' + (h.children.item(0).id));
  R.push('named:' + (h.children.namedItem('p2').id));
  R.push('index:' + h.children[1].id);

  // ── LIVE. Hold the collection, mutate the tree, and the collection must move with it.
  var c = h.children;
  var before = c.length;
  h.appendChild(document.createElement('p'));
  R.push('live:' + before + '->' + c.length);

  // ── …and shrink.
  h.removeChild(h.lastChild);
  R.push('shrink:' + c.length);

  // ── getElementsByTagName is live too.
  var ps = document.getElementsByTagName('p');
  var n0 = ps.length;
  h.appendChild(document.createElement('p'));
  R.push('gebtn:' + n0 + '->' + ps.length);

  // ── **BAR 0.** The universal "empty this element" idiom MUST TERMINATE.
  //
  // With a dead collection, `length` never changes, the condition is true forever, and the tab locks up.
  // The guard is here only so that a REGRESSION reports a failure instead of hanging the test suite —
  // a hang is a worse way to learn this than an assertion.
  var d = document.getElementById('drain');
  var spins = 0;
  while (d.children.length && spins < 1000) {
    d.removeChild(d.firstChild);
    spins++;
  }
  R.push('drain:' + d.children.length + ',' + spins);

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn collections_are_live_and_the_universal_drain_idiom_terminates() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://coll.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("isHC:true", "`children` was a plain Array — not an HTMLCollection at all"),
        ("item:p1", "`.item()` did not exist"),
        ("named:p2", "`.namedItem()` did not exist"),
        ("index:p2", "indexing must resolve against the tree as it is NOW"),
        (
            "live:2->3",
            "THE POINT: a collection held across a mutation must MOVE with the tree. It was a snapshot",
        ),
        ("shrink:2", "…and shrink when the tree shrinks"),
        // 2->3, not 3->4: the `shrink` step above removed the <p> that `live` appended. The engine was
        // right and this expectation was arithmetic. A gate that miscounts its own fixture is measuring
        // itself.
        ("gebtn:2->3", "getElementsByTagName is live too, not just `children`"),
        (
            "drain:0,5",
            "**BAR 0.** `while (el.children.length) el.removeChild(el.firstChild)` — the universal \
             \"empty this element\" idiom — must TERMINATE, in exactly 5 spins for 5 children. With a \
             dead collection `length` never changes, the condition is true forever, and THE TAB LOCKS \
             UP. A dead collection does not fail loudly; it spins",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_COLLECTIONS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
