//! **`NodeIterator` and `TreeWalker`** — the DOM's two traversal APIs, done properly.
//!
//! What was there: a `document.createTreeWalker` that returned a **plain object** with `nextNode` and
//! little else. No `previousNode`, no `firstChild`/`lastChild`/`nextSibling`/`parentNode`. No
//! `TreeWalker` constructor, so `instanceof` was false. `NodeIterator` did not exist at all.
//! `dom/traversal` scored **11/53**.
//!
//! Like `Range` (tick 71), this is a **both-horizons** target:
//!
//! * **far** — 42 failing WPT subtests behind two interfaces;
//! * **near** — traversal is how the real web walks a subtree. **DOMPurify** (the sanitizer half the web
//!   runs untrusted HTML through) is built on `NodeIterator`. Lit finds a template's dynamic holes with
//!   `createTreeWalker`. Every editor, every diffing library, every "walk the DOM and do X" script.
//!
//! ## The part that is easy, and the part that is not
//!
//! The **walk** is easy — depth-first, document order. The **filter protocol** is where implementations
//! go wrong, and it is silent when they do:
//!
//! * `whatToShow` is a **bitmask**, tested against `1 << (nodeType - 1)`. Getting the shift off by one
//!   filters the wrong node types and produces a walk that visits *something*, which is the worst kind of
//!   bug.
//! * the filter may be a bare **function** *or* an **object with an `acceptNode` method**. Both are used
//!   in the wild, and `filter.acceptNode` is what DOMPurify passes.
//! * **`FILTER_REJECT` (2) skips the entire SUBTREE. `FILTER_SKIP` (3) skips only the node** and still
//!   descends into its children. Swap them and a sanitizer that rejects `<script>` cheerfully walks
//!   *into* the script and keeps its contents. **That is a security bug shaped like a traversal bug.**
//! * and the two interfaces differ precisely here: **`NodeIterator` treats `REJECT` as `SKIP`** (it has
//!   no notion of a subtree). Implementing one and aliasing the other is wrong in exactly the way nobody
//!   notices until a sanitizer leaks.

/// `NodeIterator` + `TreeWalker`, installed into the global. See the module docs for the filter protocol.
pub const TRAVERSAL_JS: &str = r#"
(function () {
  'use strict';
  if (typeof document === 'undefined') { return; }

  var SHOW_ALL = 0xFFFFFFFF;
  var ACCEPT = 1, REJECT = 2, SKIP = 3;

  if (typeof globalThis.NodeFilter === 'undefined') { globalThis.NodeFilter = {}; }
  var NF = globalThis.NodeFilter;
  NF.FILTER_ACCEPT = ACCEPT; NF.FILTER_REJECT = REJECT; NF.FILTER_SKIP = SKIP;
  NF.SHOW_ALL = SHOW_ALL;
  NF.SHOW_ELEMENT = 0x1; NF.SHOW_ATTRIBUTE = 0x2; NF.SHOW_TEXT = 0x4;
  NF.SHOW_CDATA_SECTION = 0x8; NF.SHOW_ENTITY_REFERENCE = 0x10; NF.SHOW_ENTITY = 0x20;
  NF.SHOW_PROCESSING_INSTRUCTION = 0x40; NF.SHOW_COMMENT = 0x80; NF.SHOW_DOCUMENT = 0x100;
  NF.SHOW_DOCUMENT_TYPE = 0x200; NF.SHOW_DOCUMENT_FRAGMENT = 0x400; NF.SHOW_NOTATION = 0x800;

  // The bit for a node type is `1 << (nodeType - 1)`. Off-by-one here filters the WRONG node types and
  // produces a walk that visits *something*, which is the worst kind of wrong.
  function shown(node, whatToShow) {
    return (whatToShow & (1 << (node.nodeType - 1))) !== 0;
  }

  // The filter may be a bare function OR an object with an `acceptNode` method. Both are used in the
  // wild; `filter.acceptNode` is the form DOMPurify passes.
  function callFilter(self, node) {
    if (!shown(node, self.whatToShow)) return SKIP;
    var f = self.filter;
    if (!f) return ACCEPT;
    var fn = (typeof f === 'function') ? f : f.acceptNode;
    if (typeof fn !== 'function') return ACCEPT;
    var v = fn.call(f, node);
    return (v === REJECT || v === SKIP) ? v : ACCEPT;
  }

  // ── NodeIterator ────────────────────────────────────────────────────────────────────────────────
  //
  // A flat, pre-order cursor. **It has no notion of a subtree, so REJECT behaves as SKIP** — which is
  // exactly where TreeWalker differs, and aliasing the two is the bug that makes a sanitizer leak.
  function NodeIterator(root, whatToShow, filter) {
    this.root = root;
    this.whatToShow = whatToShow === undefined || whatToShow === null ? SHOW_ALL : whatToShow >>> 0;
    this.filter = filter || null;
    this.referenceNode = root;
    this.pointerBeforeReferenceNode = true;
  }

  function inTree(root, n) {
    for (var p = n; p; p = p.parentNode) { if (p === root) return true; }
    return false;
  }
  function docNext(root, n) {                 // next node in document order, bounded by root
    if (n.firstChild) return n.firstChild;
    var c = n;
    while (c && c !== root) {
      if (c.nextSibling) return c.nextSibling;
      c = c.parentNode;
    }
    return null;
  }
  function docPrev(root, n) {                 // previous node in document order, bounded by root
    if (n === root) return null;
    var s = n.previousSibling;
    if (!s) return n.parentNode;
    while (s.lastChild) s = s.lastChild;
    return s;
  }

  NodeIterator.prototype.nextNode = function () {
    var node = this.referenceNode, before = this.pointerBeforeReferenceNode;
    for (;;) {
      if (before) { before = false; }
      else {
        node = docNext(this.root, node);
        if (!node) { this.referenceNode = node || this.referenceNode; this.pointerBeforeReferenceNode = before; return null; }
      }
      // REJECT is treated as SKIP here, on purpose. See the module docs.
      if (callFilter(this, node) === ACCEPT) {
        this.referenceNode = node;
        this.pointerBeforeReferenceNode = before;
        return node;
      }
    }
  };
  NodeIterator.prototype.previousNode = function () {
    var node = this.referenceNode, before = this.pointerBeforeReferenceNode;
    for (;;) {
      if (!before) { before = true; }
      else {
        node = docPrev(this.root, node);
        if (!node) return null;
      }
      if (callFilter(this, node) === ACCEPT) {
        this.referenceNode = node;
        this.pointerBeforeReferenceNode = before;
        return node;
      }
    }
  };
  NodeIterator.prototype.detach = function () { /* a no-op since DOM4 */ };

  // ── TreeWalker ──────────────────────────────────────────────────────────────────────────────────
  function TreeWalker(root, whatToShow, filter) {
    this.root = root;
    this.whatToShow = whatToShow === undefined || whatToShow === null ? SHOW_ALL : whatToShow >>> 0;
    this.filter = filter || null;
    this.currentNode = root;
  }

  TreeWalker.prototype.parentNode = function () {
    var node = this.currentNode;
    while (node && node !== this.root) {
      node = node.parentNode;
      if (node && callFilter(this, node) === ACCEPT) {
        this.currentNode = node;
        return node;
      }
    }
    return null;
  };

  // firstChild / lastChild — the spec's "traverse children", and REJECT really does prune the subtree.
  function traverseChildren(self, first) {
    var node = first ? self.currentNode.firstChild : self.currentNode.lastChild;
    while (node) {
      var v = callFilter(self, node);
      if (v === ACCEPT) { self.currentNode = node; return node; }
      if (v === SKIP) {
        var child = first ? node.firstChild : node.lastChild;
        if (child) { node = child; continue; }       // SKIP: the node is passed over, its children are not
      }
      // REJECT (or a SKIP with no children): move on WITHOUT descending.
      for (;;) {
        var sib = first ? node.nextSibling : node.previousSibling;
        if (sib) { node = sib; break; }
        var parent = node.parentNode;
        if (!parent || parent === self.root || parent === self.currentNode) return null;
        node = parent;
      }
    }
    return null;
  }
  TreeWalker.prototype.firstChild = function () { return traverseChildren(this, true); };
  TreeWalker.prototype.lastChild  = function () { return traverseChildren(this, false); };

  function traverseSiblings(self, next) {
    var node = self.currentNode;
    if (node === self.root) return null;
    for (;;) {
      var sib = next ? node.nextSibling : node.previousSibling;
      while (sib) {
        node = sib;
        var v = callFilter(self, node);
        if (v === ACCEPT) { self.currentNode = node; return node; }
        sib = next ? node.firstChild : node.lastChild;   // SKIP: descend
        if (v === REJECT || !sib) {
          sib = next ? node.nextSibling : node.previousSibling;
        }
      }
      node = node.parentNode;
      if (!node || node === self.root) return null;
      if (callFilter(self, node) === ACCEPT) return null;
    }
  }
  TreeWalker.prototype.nextSibling     = function () { return traverseSiblings(this, true); };
  TreeWalker.prototype.previousSibling = function () { return traverseSiblings(this, false); };

  TreeWalker.prototype.previousNode = function () {
    var node = this.currentNode;
    while (node !== this.root) {
      var sib = node.previousSibling;
      while (sib) {
        node = sib;
        var v = callFilter(this, node);
        while (v !== REJECT && node.lastChild) {       // descend to the deepest acceptable last child
          node = node.lastChild;
          v = callFilter(this, node);
        }
        if (v === ACCEPT) { this.currentNode = node; return node; }
        sib = node.previousSibling;
      }
      if (node === this.root || !node.parentNode) return null;
      node = node.parentNode;
      if (callFilter(this, node) === ACCEPT) { this.currentNode = node; return node; }
    }
    return null;
  };

  TreeWalker.prototype.nextNode = function () {
    var node = this.currentNode, result = ACCEPT;
    for (;;) {
      // Descend, unless the last verdict REJECTED this subtree.
      while (result !== REJECT && node.firstChild) {
        node = node.firstChild;
        result = callFilter(this, node);
        if (result === ACCEPT) { this.currentNode = node; return node; }
      }
      // Otherwise move to the next sibling, climbing out as needed — never past the root.
      var sibling = null, temp = node;
      while (temp) {
        if (temp === this.root) return null;
        sibling = temp.nextSibling;
        if (sibling) break;
        temp = temp.parentNode;
      }
      if (!sibling) return null;
      node = sibling;
      result = callFilter(this, node);
      if (result === ACCEPT) { this.currentNode = node; return node; }
    }
  };

  globalThis.NodeIterator = NodeIterator;
  globalThis.TreeWalker = TreeWalker;
  document.createNodeIterator = function (root, whatToShow, filter) {
    return new NodeIterator(root, whatToShow, filter);
  };
  // Replaces the previous plain-object shim, which had `nextNode` and nothing else — no `previousNode`,
  // no `firstChild`/`nextSibling`/`parentNode`, and no prototype, so `instanceof TreeWalker` was false.
  document.createTreeWalker = function (root, whatToShow, filter) {
    return new TreeWalker(root, whatToShow, filter);
  };
})();
"#;
