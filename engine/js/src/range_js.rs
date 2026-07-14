//! **A real `Range`** — the DOM's boundary-point API, implemented on top of the DOM we already have.
//!
//! `Range` was an **inert stub**: `typeof Range === 'function'` was true (it was in the interface list),
//! `document.createRange()` did not exist, and nothing did anything. `dom/ranges` scored **2/200**.
//!
//! It is worth doing for both horizons at once, which is rare:
//!
//! * **far** — ~198 WPT subtests sit behind this one interface;
//! * **near** — every rich-text editor, every selection, every copy/paste path and every
//!   `contenteditable` surface on the web is built on `Range`. It is not an exotic API; it is *the*
//!   primitive for "a span of the document between two points".
//!
//! ## Why JavaScript and not Rust
//!
//! A `Range` is **pure tree arithmetic** — compare two boundary points, find a common ancestor, splice a
//! subtree. It touches no engine internals that JS cannot already reach: `parentNode`, `childNodes`,
//! `insertBefore`, `cloneNode`, `splitText`, `data`. Writing it in Rust would mean re-crossing the FFI
//! for every one of those, and re-implementing tree walks that already exist above the boundary.
//!
//! The one thing it *does* need from the engine is a **correct DOM**, and that is exactly what the last
//! several ticks built: real prototypes, a working `insertBefore` with the spec's reference-child step,
//! CharacterData in UTF-16 code units. This lands cheaply *because* of them.
//!
//! ## The two algorithms everything else is built from
//!
//! 1. **Boundary-point comparison** (`cmp`) — given `(nodeA, offsetA)` and `(nodeB, offsetB)`, which
//!    comes first in document order? Every `compareBoundaryPoints`, `isPointInRange`, `intersectsNode`
//!    and range-validity check reduces to this. Get it wrong and *everything* built on it is subtly
//!    wrong in a way no single test names.
//!
//! 2. **`extract` / `clone`** — the spec's own algorithm, and it is fiddly on purpose: the partially
//!    contained start and end nodes must be *split* (a `Text` node cut in half; an element cloned with
//!    only the contained part of its children), while fully contained nodes move wholesale. Doing the
//!    easy thing — moving whole nodes — produces a range that "works" on flat text and mangles every
//!    document with structure, which is every real document.

/// The `Range` implementation, installed into the global. See the module docs for why it is JS.
pub const RANGE_JS: &str = r#"
(function () {
  'use strict';
  if (typeof document === 'undefined') { return; }

  var TEXT = 3, CDATA = 4, COMMENT = 8, PI = 7, DOCFRAG = 11, ELEMENT = 1, DOC = 9, DOCTYPE = 10;

  function isCharacterData(n) {
    return n && (n.nodeType === TEXT || n.nodeType === COMMENT || n.nodeType === CDATA || n.nodeType === PI);
  }

  // The node's "length" as the spec means it: for CharacterData the number of UTF-16 code units, for
  // everything else the number of children. Confusing the two puts every offset in the wrong space.
  function nodeLength(n) {
    if (!n) return 0;
    if (isCharacterData(n)) return (n.data || '').length;
    if (n.nodeType === DOCTYPE) return 0;
    return n.childNodes ? n.childNodes.length : 0;
  }

  function parentOf(n) { return n ? n.parentNode : null; }

  function indexOf(n) {
    var p = parentOf(n);
    if (!p) return 0;
    var kids = p.childNodes;
    for (var i = 0; i < kids.length; i++) { if (kids[i] === n) return i; }
    return 0;
  }

  // Root of the tree `n` sits in (its topmost ancestor, which for an attached node is the document).
  function rootOf(n) {
    var r = n;
    while (r && r.parentNode) { r = r.parentNode; }
    return r;
  }

  function isInclusiveAncestor(a, b) {   // is `a` an inclusive ancestor of `b`?
    for (var n = b; n; n = n.parentNode) { if (n === a) return true; }
    return false;
  }

  // ── THE ALGORITHM EVERYTHING ELSE IS BUILT FROM.
  //
  // Compare boundary points (nodeA, offsetA) and (nodeB, offsetB) in document order:
  // -1 = A before B, 0 = same, 1 = A after B.
  //
  // Every compareBoundaryPoints, isPointInRange, intersectsNode and validity check reduces to this. Get
  // it wrong and everything built on it is subtly wrong in a way that no single test names.
  function cmp(nodeA, offsetA, nodeB, offsetB) {
    if (nodeA === nodeB) {
      return offsetA < offsetB ? -1 : offsetA > offsetB ? 1 : 0;
    }
    // A is an ancestor of B: compare A's offset against B's ancestor-chain index within A.
    if (isInclusiveAncestor(nodeA, nodeB)) {
      var child = nodeB;
      while (child && child.parentNode !== nodeA) { child = child.parentNode; }
      if (child) {
        var ci = indexOf(child);
        return offsetA <= ci ? -1 : 1;
      }
    }
    if (isInclusiveAncestor(nodeB, nodeA)) {
      return -cmp(nodeB, offsetB, nodeA, offsetA);
    }
    // Neither contains the other: walk both up to the first common parent and compare child indices.
    var ancA = [], n;
    for (n = nodeA; n; n = n.parentNode) { ancA.push(n); }
    for (n = nodeB; n; n = n.parentNode) {
      var i = ancA.indexOf(n.parentNode);
      if (i > 0) {
        // ancA[i-1] and n are siblings under n.parentNode.
        return indexOf(ancA[i - 1]) < indexOf(n) ? -1 : 1;
      }
    }
    return 0;   // different trees: the spec says this is not comparable; 0 is the least-harmful answer
  }

  function Range() {
    this._sc = document; this._so = 0;
    this._ec = document; this._eo = 0;
  }

  Range.START_TO_START = 0;
  Range.START_TO_END   = 1;
  Range.END_TO_END     = 2;
  Range.END_TO_START   = 3;
  Range.prototype.START_TO_START = 0;
  Range.prototype.START_TO_END   = 1;
  Range.prototype.END_TO_END     = 2;
  Range.prototype.END_TO_START   = 3;

  function def(name, get) {
    Object.defineProperty(Range.prototype, name, { get: get, configurable: true, enumerable: true });
  }
  def('startContainer', function () { return this._sc; });
  def('startOffset',    function () { return this._so; });
  def('endContainer',   function () { return this._ec; });
  def('endOffset',      function () { return this._eo; });
  def('collapsed',      function () { return this._sc === this._ec && this._so === this._eo; });
  def('commonAncestorContainer', function () {
    var n = this._sc;
    while (n && !isInclusiveAncestor(n, this._ec)) { n = n.parentNode; }
    return n || this._sc;
  });

  function badOffset(node, offset) {
    return offset < 0 || offset > nodeLength(node);
  }
  function throwIndex() {
    var e = new Error('offset is out of range');
    e.name = 'IndexSizeError';
    throw e;
  }

  // Setting one boundary past the other COLLAPSES the range onto the new point. That is the spec, and it
  // is not an edge case: `setStart` after `setEnd` is the ordinary way to build a backwards selection.
  Range.prototype.setStart = function (node, offset) {
    offset = offset | 0;
    if (badOffset(node, offset)) throwIndex();
    if (rootOf(node) !== rootOf(this._ec) || cmp(node, offset, this._ec, this._eo) > 0) {
      this._ec = node; this._eo = offset;
    }
    this._sc = node; this._so = offset;
  };
  Range.prototype.setEnd = function (node, offset) {
    offset = offset | 0;
    if (badOffset(node, offset)) throwIndex();
    if (rootOf(node) !== rootOf(this._sc) || cmp(node, offset, this._sc, this._so) < 0) {
      this._sc = node; this._so = offset;
    }
    this._ec = node; this._eo = offset;
  };
  Range.prototype.setStartBefore = function (n) { this.setStart(parentOf(n), indexOf(n)); };
  Range.prototype.setStartAfter  = function (n) { this.setStart(parentOf(n), indexOf(n) + 1); };
  Range.prototype.setEndBefore   = function (n) { this.setEnd(parentOf(n), indexOf(n)); };
  Range.prototype.setEndAfter    = function (n) { this.setEnd(parentOf(n), indexOf(n) + 1); };

  Range.prototype.collapse = function (toStart) {
    if (toStart) { this._ec = this._sc; this._eo = this._so; }
    else         { this._sc = this._ec; this._so = this._eo; }
  };
  Range.prototype.selectNode = function (n) {
    var p = parentOf(n), i = indexOf(n);
    this._sc = p; this._so = i; this._ec = p; this._eo = i + 1;
  };
  Range.prototype.selectNodeContents = function (n) {
    this._sc = n; this._so = 0; this._ec = n; this._eo = nodeLength(n);
  };
  Range.prototype.cloneRange = function () {
    var r = new Range();
    r._sc = this._sc; r._so = this._so; r._ec = this._ec; r._eo = this._eo;
    return r;
  };
  Range.prototype.detach = function () { /* a no-op since DOM4, and deliberately so */ };

  Range.prototype.compareBoundaryPoints = function (how, other) {
    switch (how) {
      case 0: return cmp(this._sc, this._so, other._sc, other._so);  // START_TO_START
      case 1: return cmp(this._ec, this._eo, other._sc, other._so);  // START_TO_END
      case 2: return cmp(this._ec, this._eo, other._ec, other._eo);  // END_TO_END
      case 3: return cmp(this._sc, this._so, other._ec, other._eo);  // END_TO_START
    }
    var e = new Error('invalid comparison type');
    e.name = 'NotSupportedError';
    throw e;
  };

  Range.prototype.isPointInRange = function (node, offset) {
    if (rootOf(node) !== rootOf(this._sc)) return false;
    if (badOffset(node, offset)) throwIndex();
    return cmp(node, offset, this._sc, this._so) >= 0 && cmp(node, offset, this._ec, this._eo) <= 0;
  };
  Range.prototype.comparePoint = function (node, offset) {
    if (badOffset(node, offset)) throwIndex();
    if (cmp(node, offset, this._sc, this._so) < 0) return -1;
    if (cmp(node, offset, this._ec, this._eo) > 0) return 1;
    return 0;
  };
  Range.prototype.intersectsNode = function (node) {
    if (rootOf(node) !== rootOf(this._sc)) return false;
    var p = parentOf(node);
    if (!p) return true;                       // the root always intersects
    var i = indexOf(node);
    return cmp(p, i, this._ec, this._eo) < 0 && cmp(p, i + 1, this._sc, this._so) > 0;
  };

  // Is `node` FULLY contained — both its boundaries strictly inside the range?
  function contained(range, node) {
    var p = parentOf(node);
    if (!p) return false;
    var i = indexOf(node);
    return cmp(p, i, range._sc, range._so) >= 0 && cmp(p, i + 1, range._ec, range._eo) <= 0;
  }
  // Partially contained: an ancestor of exactly one of the two boundary nodes.
  function partiallyContained(range, node) {
    var a = isInclusiveAncestor(node, range._sc);
    var b = isInclusiveAncestor(node, range._ec);
    return (a && !b) || (b && !a);
  }

  // ── extract / clone / delete. The spec's algorithm, and it is fiddly ON PURPOSE.
  //
  // The naive version moves whole nodes and "works" on flat text. It mangles every document with
  // structure, which is every real document: a range from the middle of one paragraph to the middle of
  // the next must SPLIT both, keep the outer halves in place, and take the inner halves.
  function doExtract(range, mode) {   // mode: 'extract' | 'clone' | 'delete'
    var frag = document.createDocumentFragment();
    if (range.collapsed) return frag;

    var sc = range._sc, so = range._so, ec = range._ec, eo = range._eo;

    // Both ends in the SAME character-data node: pure string surgery.
    if (sc === ec && isCharacterData(sc)) {
      var text = sc.data.slice(so, eo);
      if (mode !== 'delete') {
        var clone = sc.cloneNode(false);
        clone.data = text;
        frag.appendChild(clone);
      }
      if (mode !== 'clone') {
        sc.data = sc.data.slice(0, so) + sc.data.slice(eo);
        range._ec = sc; range._eo = so;
      }
      return frag;
    }

    var common = range.commonAncestorContainer;

    // The first and last nodes that are *partially* contained — the ones that must be split.
    var firstPartial = null, lastPartial = null, n;
    for (n = sc; n && n !== common; n = n.parentNode) {
      if (partiallyContained(range, n)) firstPartial = n;
    }
    for (n = ec; n && n !== common; n = n.parentNode) {
      if (partiallyContained(range, n)) lastPartial = n;
    }
    if (isCharacterData(sc)) firstPartial = sc;
    if (isCharacterData(ec)) lastPartial = ec;

    // Everything fully contained, in tree order, at the top level of the common ancestor.
    var top = [];
    var kids = common.childNodes;
    for (var i = 0; i < kids.length; i++) {
      if (contained(range, kids[i])) top.push(kids[i]);
    }

    // 1. The start half of the partially-contained start node.
    if (firstPartial && isCharacterData(firstPartial)) {
      if (mode !== 'delete') {
        var c1 = firstPartial.cloneNode(false);
        c1.data = firstPartial.data.slice(so);
        frag.appendChild(c1);
      }
      if (mode !== 'clone') firstPartial.data = firstPartial.data.slice(0, so);
    } else if (firstPartial) {
      var sub = firstPartial.cloneNode(false);
      var inner = range.cloneRange();
      inner._sc = sc; inner._so = so;
      inner._ec = firstPartial; inner._eo = nodeLength(firstPartial);
      var subFrag = doExtract(inner, mode);
      while (subFrag.firstChild) sub.appendChild(subFrag.firstChild);
      if (mode !== 'delete') frag.appendChild(sub);
    }

    // 2. Everything fully inside.
    for (var k = 0; k < top.length; k++) {
      var node = top[k];
      if (mode === 'clone') frag.appendChild(node.cloneNode(true));
      else if (mode === 'extract') frag.appendChild(node);
      else node.parentNode.removeChild(node);
    }

    // 3. The end half of the partially-contained end node.
    if (lastPartial && isCharacterData(lastPartial)) {
      if (mode !== 'delete') {
        var c2 = lastPartial.cloneNode(false);
        c2.data = lastPartial.data.slice(0, eo);
        frag.appendChild(c2);
      }
      if (mode !== 'clone') lastPartial.data = lastPartial.data.slice(eo);
    } else if (lastPartial) {
      var sub2 = lastPartial.cloneNode(false);
      var inner2 = range.cloneRange();
      inner2._sc = lastPartial; inner2._so = 0;
      inner2._ec = ec; inner2._eo = eo;
      var subFrag2 = doExtract(inner2, mode);
      while (subFrag2.firstChild) sub2.appendChild(subFrag2.firstChild);
      if (mode !== 'delete') frag.appendChild(sub2);
    }

    if (mode !== 'clone') {
      // The range collapses to its start — there is nothing between the points any more.
      range._ec = range._sc; range._eo = range._so;
    }
    return frag;
  }

  Range.prototype.cloneContents   = function () { return doExtract(this, 'clone'); };
  Range.prototype.extractContents = function () { return doExtract(this, 'extract'); };
  Range.prototype.deleteContents  = function () { doExtract(this, 'delete'); };

  Range.prototype.insertNode = function (node) {
    var sc = this._sc, so = this._so;
    if (isCharacterData(sc)) {
      // Split the text node at the offset and insert between the halves.
      var after = sc.cloneNode(false);
      after.data = sc.data.slice(so);
      sc.data = sc.data.slice(0, so);
      var p = sc.parentNode;
      p.insertBefore(node, sc.nextSibling);
      p.insertBefore(after, node.nextSibling);
    } else {
      var ref = sc.childNodes[so] || null;
      sc.insertBefore(node, ref);
    }
  };

  Range.prototype.surroundContents = function (newParent) {
    var frag = this.extractContents();
    while (newParent.firstChild) newParent.removeChild(newParent.firstChild);
    this.insertNode(newParent);
    newParent.appendChild(frag);
    this.selectNode(newParent);
  };

  Range.prototype.toString = function () {
    var sc = this._sc, so = this._so, ec = this._ec, eo = this._eo;
    if (sc === ec && isCharacterData(sc)) return sc.data.slice(so, eo);

    var s = '';
    if (isCharacterData(sc)) s += sc.data.slice(so);

    // Every Text node fully inside the range, in document order.
    var common = this.commonAncestorContainer, range = this;
    (function walk(n) {
      for (var i = 0; i < n.childNodes.length; i++) {
        var c = n.childNodes[i];
        if (c.nodeType === TEXT) {
          if (c !== sc && c !== ec && contained(range, c)) s += c.data;
        } else if (c.nodeType === ELEMENT) {
          walk(c);
        }
      }
    })(common);

    if (isCharacterData(ec)) s += ec.data.slice(0, eo);
    return s;
  };

  Object.defineProperty(Range.prototype, 'toString', { enumerable: false });
  Object.defineProperty(Range, 'name', { value: 'Range' });

  // Replace the inert stub. It was in the interface list, so `typeof Range === 'function'` was already
  // true — which is precisely why nobody noticed it did nothing.
  globalThis.Range = Range;
  document.createRange = function () {
    var r = new Range();
    r.selectNodeContents(document);
    r.collapse(true);
    return r;
  };
})();
"#;
