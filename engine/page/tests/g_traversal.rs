//! **G_TRAVERSAL — `NodeIterator` and `TreeWalker`, with the filter protocol the web actually depends on.**
//!
//! What was there: a `createTreeWalker` returning a **plain object** with `nextNode` and nothing else —
//! no `previousNode`, no `firstChild`/`nextSibling`/`parentNode`, and no prototype, so
//! `instanceof TreeWalker` was false. `NodeIterator` did not exist at all. `dom/traversal` scored
//! **11/53**.
//!
//! Both horizons at once, like `Range`:
//!
//! * **far** — 42 failing WPT subtests behind two interfaces;
//! * **near** — traversal is how the real web walks a subtree. **DOMPurify** — the sanitizer half the web
//!   runs untrusted HTML through — is built on `NodeIterator`. Lit finds a template's dynamic holes with
//!   `createTreeWalker`.
//!
//! **The walk is the easy part. The filter protocol is where implementations go wrong, silently.** The
//! assertion below that matters most is `reject:` — and it is a *security* assertion wearing a traversal
//! assertion's clothes:
//!
//! > **`FILTER_REJECT` (2) must skip the whole SUBTREE. `FILTER_SKIP` (3) skips only the node** and still
//! > descends into its children. Swap them, and a sanitizer that rejects `<script>` walks cheerfully
//! > *into* the script and keeps its contents.
//!
//! And the two interfaces differ exactly there: **`NodeIterator` has no notion of a subtree, so it treats
//! `REJECT` as `SKIP`.** Implementing one and aliasing the other is wrong in the way nobody notices until
//! something leaks.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="root">
  <p id="a">one</p>
  <div id="bad"><span id="hidden">SECRET</span></div>
  <p id="b">two</p>
</div>
<script>
  var R = [];
  var root = document.getElementById('root');

  // ── 1. The interfaces exist and instances are instances.
  R.push('ni:' + (typeof document.createNodeIterator === 'function'));
  R.push('tw:' + (typeof document.createTreeWalker === 'function'));
  R.push('inst:' + (document.createTreeWalker(root) instanceof TreeWalker));

  // ── 2. whatToShow is a BITMASK against `1 << (nodeType - 1)`. Off by one and it filters the wrong
  //       node types — producing a walk that visits *something*, which is the worst kind of wrong.
  var w = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
  var ids = [];
  var n;
  while ((n = w.nextNode())) { ids.push(n.id || n.tagName); }
  R.push('els:' + ids.join(','));

  // ── 3. Text nodes only — a different bit, and it must not leak elements.
  var wt = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  var texts = 0;
  while (wt.nextNode()) { texts++; }
  R.push('textNodes:' + (texts > 0));

  // ── 4. **THE ONE THAT MATTERS.** FILTER_REJECT must prune the entire SUBTREE.
  //       A sanitizer rejects <script>; if REJECT behaved as SKIP it would walk INTO it and keep the
  //       contents. This is a security bug shaped like a traversal bug.
  var seen = [];
  var wr = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT, {
    acceptNode: function (node) {
      if (node.id === 'bad') return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    }
  });
  while ((n = wr.nextNode())) { seen.push(n.id); }
  R.push('reject:' + seen.join(','));      // must NOT contain `hidden`

  // ── 5. FILTER_SKIP passes over the node but still DESCENDS into its children.
  var seen2 = [];
  var ws = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT, {
    acceptNode: function (node) {
      if (node.id === 'bad') return NodeFilter.FILTER_SKIP;
      return NodeFilter.FILTER_ACCEPT;
    }
  });
  while ((n = ws.nextNode())) { seen2.push(n.id); }
  R.push('skip:' + seen2.join(','));       // must contain `hidden` but NOT `bad`

  // ── 6. The filter may be a bare FUNCTION as well as an object — both are used in the wild.
  var wf = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT, function (node) {
    return node.tagName === 'P' ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP;
  });
  var ps = [];
  while ((n = wf.nextNode())) { ps.push(n.id); }
  R.push('fnFilter:' + ps.join(','));

  // ── 7. TreeWalker navigates in every direction, not just forwards. The old shim had `nextNode` alone.
  var wn = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
  wn.nextNode();                            // #a
  R.push('nextSib:' + (wn.nextSibling() || {}).id);
  R.push('parent:' + (wn.parentNode() || {}).id);
  R.push('first:' + (wn.firstChild() || {}).id);

  // ── 8. NodeIterator: flat, and REJECT is treated as SKIP because it has no notion of a subtree.
  //       Aliasing the two interfaces is wrong in exactly the way nobody notices.
  var it = document.createNodeIterator(root, NodeFilter.SHOW_ELEMENT, {
    acceptNode: function (node) {
      return node.id === 'bad' ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT;
    }
  });
  var iseen = [];
  while ((n = it.nextNode())) { iseen.push(n.id); }
  // NOTE `root` is first. NodeIterator's reference node STARTS at the root with
  // pointerBeforeReferenceNode = true, so the first nextNode() returns the root itself — whereas
  // TreeWalker starts *at* the root and moves away from it, and never returns it. That asymmetry is
  // real, it is easy to get backwards, and this gate had it backwards before the implementation
  // corrected it.
  R.push('iter:' + iseen.join(','));        // `hidden` IS reached — REJECT means SKIP here

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn traversal_walks_both_ways_and_the_filter_protocol_is_the_specs() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://traversal.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("ni:true", "`document.createNodeIterator` did not exist — and DOMPurify is built on it"),
        ("tw:true", "createTreeWalker existed but returned a plain object"),
        ("inst:true", "…with no prototype, so `instanceof TreeWalker` was false"),
        ("els:a,bad,hidden,b", "whatToShow is a bitmask over `1 << (nodeType - 1)`, in document order"),
        ("textNodes:true", "a different bit must select text nodes, and not leak elements"),
        (
            "reject:a,b",
            "**FILTER_REJECT PRUNES THE SUBTREE.** `hidden` must NOT appear. If REJECT behaved as SKIP, a \
             sanitizer that rejects `<script>` would walk INTO it and keep its contents — a security bug \
             shaped like a traversal bug",
        ),
        (
            "skip:a,hidden,b",
            "FILTER_SKIP passes over the node and STILL DESCENDS. `hidden` appears, `bad` does not. This \
             is the exact opposite of REJECT, and swapping them is silent",
        ),
        ("fnFilter:a,b", "the filter may be a bare function, not only an object with acceptNode"),
        ("nextSib:bad", "TreeWalker navigates sideways — the old shim had `nextNode` and nothing else"),
        ("parent:root", "…and upwards"),
        ("first:a", "…and downwards"),
        (
            "iter:root,a,hidden,b",
            "NodeIterator has NO notion of a subtree, so REJECT is treated as SKIP and `hidden` IS \
             reached — TreeWalker and NodeIterator differ precisely here, and aliasing them is wrong in \
             the way nobody notices until something leaks. And note `root` comes FIRST: NodeIterator's \
             reference node starts AT the root, so the first `nextNode()` returns it, while TreeWalker \
             starts at the root and moves away, never returning it. This gate asserted the wrong thing \
             until the implementation corrected it",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_TRAVERSAL: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
