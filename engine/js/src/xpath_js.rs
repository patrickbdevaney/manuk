//! # `document.evaluate` / `XPathEvaluator` — a REAL subset, and an honest throw for the rest
//!
//! **Why this exists (tick 644).** htmx 2.0.4 was completely dead here:
//! `ReferenceError: XPathEvaluator is not defined`, thrown *during its own evaluation*, so
//! `window.htmx` was never defined at all. Its use is one expression at module top level:
//!
//! ```js
//! const Ct = (new XPathEvaluator).createExpression(
//!   './/*[@*[ starts-with(name(), "hx-on:") or starts-with(name(), "data-hx-on:") or … ]]');
//! …
//! const n = Ct.evaluate(el); let r; while (r = n.iterateNext()) { … }
//! ```
//!
//! ## The EME precedent does not transfer, and that decides the shape
//!
//! Tick 641 gave the EME interfaces existence while **granting nothing**, and that was honest
//! because *"no key system is supported"* is a truthful answer a caller can handle. **XPath has no
//! such refusal**: an evaluator either returns the right nodes or it lies, and a caller cannot tell
//! the difference. A stub that returned an empty node-set would make htmx boot and then silently
//! fail to wire up every `hx-on:` handler on the page — strictly worse than the `ReferenceError`,
//! which at least says something is wrong.
//!
//! So this is a **real evaluator over a documented subset**, which **throws for anything outside
//! it**. That is the same discipline `canPlayType` applies to a codec it cannot decode: answer
//! correctly or refuse, never guess. A page using unsupported XPath gets a `SyntaxError` naming the
//! construct, not a wrong node-set.
//!
//! ## The subset, stated so it can be checked
//!
//! * **Paths** — absolute (`/…`), relative, and `//` (descendant-or-self).
//! * **Axes** — `child` (implicit), `descendant-or-self` (`//`), `attribute` (`@`), `self` (`.`),
//!   `parent` (`..`). Named-axis syntax (`ancestor::`, `following-sibling::`, …) **throws**.
//! * **Node tests** — a name, `*`, `node()`, `text()`.
//! * **Predicates** — a position number; an attribute test (`@x`, `@x='v'`); `name()`,
//!   `local-name()`, `string()`, `text()`; the functions `starts-with`, `contains`, `not`; the
//!   operators `and`, `or`, `=`, `!=`; string literals; nested paths (a node-set is true when
//!   non-empty, which is exactly what htmx's `@*[…]` relies on).
//! * Everything else — arithmetic, `|` unions, `position()`, `last()`, numeric comparison,
//!   namespace prefixes, unsupported axes and unknown functions — **throws a `SyntaxError`**.
//!
//! ## Install order
//!
//! Anywhere after `dom_bindings`' install (it needs `document` and `DOMException`). It defines
//! `XPathEvaluator`, `XPathExpression`, `XPathResult` and `document.evaluate`, and it defines each
//! only if absent, so it never clobbers a richer implementation that lands later.

/// The XPath subset evaluator, as a self-installing script.
pub const XPATH_JS: &str = r#"
(function () {
  'use strict';
  var g = globalThis;
  if (typeof g.XPathEvaluator !== 'undefined') { return; }

  function bad(msg) {
    // A construct outside the supported subset is a SYNTAX ERROR, not an empty result. This is the
    // whole honesty contract of this module: a caller that gets a node-set can trust it.
    var e = g.DOMException ? new g.DOMException(msg, 'SyntaxError') : new Error(msg);
    throw e;
  }

  // ── Tokenizer ────────────────────────────────────────────────────────────────────────────────
  function lex(src) {
    var t = [], i = 0, n = src.length;
    while (i < n) {
      var c = src[i];
      if (c === ' ' || c === '\t' || c === '\n' || c === '\r') { i++; continue; }
      if (c === '"' || c === "'") {
        var j = src.indexOf(c, i + 1);
        if (j < 0) { bad('unterminated string literal in XPath'); }
        t.push({ k: 'str', v: src.slice(i + 1, j) }); i = j + 1; continue;
      }
      if (src.startsWith('//', i)) { t.push({ k: '//' }); i += 2; continue; }
      if (src.startsWith('..', i)) { t.push({ k: '..' }); i += 2; continue; }
      if (src.startsWith('!=', i)) { t.push({ k: 'op', v: '!=' }); i += 2; continue; }
      if (src.startsWith('::', i)) { bad('XPath axis syntax (name::) is outside the supported subset'); }
      if ('/[]()@,.*='.indexOf(c) >= 0) { t.push({ k: c }); i++; continue; }
      if (c >= '0' && c <= '9') {
        var d = i; while (i < n && src[i] >= '0' && src[i] <= '9') { i++; }
        t.push({ k: 'num', v: parseInt(src.slice(d, i), 10) }); continue;
      }
      if (/[A-Za-z_]/.test(c)) {
        var s = i; while (i < n && /[A-Za-z0-9_.\-]/.test(src[i])) { i++; }
        var w = src.slice(s, i);
        if (w === 'and' || w === 'or') { t.push({ k: 'op', v: w }); }
        else { t.push({ k: 'name', v: w }); }
        continue;
      }
      if (c === '|') { bad('XPath union (|) is outside the supported subset'); }
      if (c === '+' || c === '-' || c === '<' || c === '>') {
        bad('XPath arithmetic/comparison (' + c + ') is outside the supported subset');
      }
      bad('unexpected character in XPath: ' + c);
    }
    return t;
  }

  // ── Parser ───────────────────────────────────────────────────────────────────────────────────
  function Parser(tokens) { this.t = tokens; this.i = 0; }
  Parser.prototype.peek = function () { return this.t[this.i]; };
  Parser.prototype.next = function () { return this.t[this.i++]; };
  Parser.prototype.eat = function (k) {
    var p = this.peek();
    if (!p || p.k !== k) { bad('expected ' + k + ' in XPath'); }
    return this.next();
  };

  // path := ['/' | '//'] step ( ('/' | '//') step )*
  Parser.prototype.parsePath = function () {
    var steps = [], absolute = false;
    if (this.peek() && this.peek().k === '/') { absolute = true; this.next(); }
    else if (this.peek() && this.peek().k === '//') { absolute = true; this.next(); steps.push({ axis: 'dos' }); }
    for (;;) {
      var p = this.peek();
      if (!p) { break; }
      if (p.k === ']' || p.k === ')' || p.k === 'op' || p.k === ',' || p.k === '=') { break; }
      steps.push(this.parseStep());
      var q = this.peek();
      if (q && q.k === '/') { this.next(); continue; }
      if (q && q.k === '//') { this.next(); steps.push({ axis: 'dos' }); continue; }
      break;
    }
    return { type: 'path', absolute: absolute, steps: steps };
  };

  Parser.prototype.parseStep = function () {
    var p = this.peek(), step;
    if (p.k === '.') { this.next(); step = { axis: 'self', test: '*' }; }
    else if (p.k === '..') { this.next(); step = { axis: 'parent', test: '*' }; }
    else if (p.k === '@') {
      this.next();
      var a = this.peek();
      if (a && a.k === '*') { this.next(); step = { axis: 'attr', test: '*' }; }
      else { step = { axis: 'attr', test: this.eat('name').v }; }
    } else if (p.k === '*') { this.next(); step = { axis: 'child', test: '*' }; }
    else if (p.k === 'name') {
      var nm = this.next().v;
      if (this.peek() && this.peek().k === '(') {
        // A step that is a node test with parens: node() / text().
        this.next(); this.eat(')');
        if (nm !== 'node' && nm !== 'text') {
          bad('XPath node test ' + nm + '() is outside the supported subset');
        }
        step = { axis: 'child', test: nm === 'text' ? '#text' : '#node' };
      } else {
        if (nm.indexOf(':') >= 0) { bad('XPath namespace prefixes are outside the supported subset'); }
        step = { axis: 'child', test: nm };
      }
    } else { bad('unexpected token in XPath step'); }

    step.preds = [];
    while (this.peek() && this.peek().k === '[') {
      this.next();
      step.preds.push(this.parseExpr());
      this.eat(']');
    }
    return step;
  };

  // expr := and-expr ( 'or' and-expr )*
  Parser.prototype.parseExpr = function () {
    var left = this.parseAnd();
    while (this.peek() && this.peek().k === 'op' && this.peek().v === 'or') {
      this.next();
      left = { type: 'or', l: left, r: this.parseAnd() };
    }
    return left;
  };
  Parser.prototype.parseAnd = function () {
    var left = this.parseEq();
    while (this.peek() && this.peek().k === 'op' && this.peek().v === 'and') {
      this.next();
      left = { type: 'and', l: left, r: this.parseEq() };
    }
    return left;
  };
  Parser.prototype.parseEq = function () {
    var left = this.parsePrimary();
    var p = this.peek();
    if (p && (p.k === '=' || (p.k === 'op' && p.v === '!='))) {
      var neg = p.k === 'op';
      this.next();
      return { type: 'eq', neg: neg, l: left, r: this.parsePrimary() };
    }
    return left;
  };
  Parser.prototype.parsePrimary = function () {
    var p = this.peek();
    if (!p) { bad('unexpected end of XPath expression'); }
    if (p.k === 'str') { this.next(); return { type: 'str', v: p.v }; }
    if (p.k === 'num') { this.next(); return { type: 'num', v: p.v }; }
    if (p.k === '(') { this.next(); var e = this.parseExpr(); this.eat(')'); return e; }
    if (p.k === 'name' && this.t[this.i + 1] && this.t[this.i + 1].k === '(') {
      var fn = this.next().v; this.next();
      var args = [];
      if (!(this.peek() && this.peek().k === ')')) {
        args.push(this.parseExpr());
        while (this.peek() && this.peek().k === ',') { this.next(); args.push(this.parseExpr()); }
      }
      this.eat(')');
      if (['starts-with', 'contains', 'not', 'name', 'local-name', 'string', 'text', 'node']
            .indexOf(fn) < 0) {
        bad('XPath function ' + fn + '() is outside the supported subset');
      }
      return { type: 'call', fn: fn, args: args };
    }
    return this.parsePath();
  };

  function parse(src) {
    var p = new Parser(lex(String(src)));
    var e = p.parseExpr();
    if (p.i !== p.t.length) { bad('trailing tokens in XPath expression'); }
    return e;
  }

  // ── Evaluation ───────────────────────────────────────────────────────────────────────────────
  // An attribute is not a Node here, so it is modelled explicitly. `name()` on it is the attribute
  // name — which is the entire point of htmx's `@*[starts-with(name(), "hx-on:")]`.
  function attrNodes(el) {
    var out = [];
    if (!el || el.nodeType !== 1 || !el.attributes) { return out; }
    for (var i = 0; i < el.attributes.length; i++) {
      var a = el.attributes[i];
      out.push({ __attr: true, name: a.name, value: a.value, ownerElement: el });
    }
    return out;
  }

  function nameOf(n) {
    if (!n) { return ''; }
    if (n.__attr) { return n.name; }
    if (n.nodeType === 1) { return n.nodeName ? n.nodeName.toLowerCase() : ''; }
    return n.nodeName || '';
  }
  function stringOf(n) {
    if (!n) { return ''; }
    if (n.__attr) { return n.value; }
    return n.textContent != null ? n.textContent : '';
  }

  function matchesTest(n, test) {
    if (test === '*') { return n.__attr ? true : n.nodeType === 1; }
    if (test === '#node') { return true; }
    if (test === '#text') { return n.nodeType === 3; }
    if (n.__attr) { return n.name === test; }
    return n.nodeType === 1 && n.nodeName && n.nodeName.toLowerCase() === String(test).toLowerCase();
  }

  function descendantsOrSelf(node) {
    var out = [], stack = [node];
    // Document order, iterative — a recursive walk blows the stack on a deep real page.
    while (stack.length) {
      var cur = stack.shift();
      out.push(cur);
      var kids = cur.childNodes;
      if (kids && kids.length) {
        var front = [];
        for (var i = 0; i < kids.length; i++) { front.push(kids[i]); }
        stack = front.concat(stack);
      }
    }
    return out;
  }

  function stepNodes(node, step) {
    if (step.axis === 'dos') { return descendantsOrSelf(node); }
    if (step.axis === 'self') { return [node]; }
    if (step.axis === 'parent') { return node.parentNode ? [node.parentNode] : []; }
    if (step.axis === 'attr') { return attrNodes(node).filter(function (a) { return matchesTest(a, step.test); }); }
    var out = [], kids = node.childNodes || [];
    for (var i = 0; i < kids.length; i++) {
      if (matchesTest(kids[i], step.test)) { out.push(kids[i]); }
    }
    return out;
  }

  function evalPath(ast, ctx, root) {
    var cur = ast.absolute ? [root] : [ctx];
    for (var s = 0; s < ast.steps.length; s++) {
      var step = ast.steps[s], next = [], seen = [];
      for (var i = 0; i < cur.length; i++) {
        var got = stepNodes(cur[i], step);
        for (var j = 0; j < got.length; j++) {
          if (seen.indexOf(got[j]) < 0) { seen.push(got[j]); next.push(got[j]); }
        }
      }
      // A `dos` step is a pure axis with no test of its own; the following step filters it.
      if (step.axis === 'dos' && step.test) {
        next = next.filter(function (n) { return matchesTest(n, step.test); });
      }
      for (var p = 0; p < (step.preds || []).length; p++) {
        var pred = step.preds[p];
        next = next.filter(function (n, idx) {
          var v = evalExpr(pred, n, root);
          if (typeof v === 'number') { return idx + 1 === v; }
          return truthy(v);
        });
      }
      cur = next;
    }
    return cur;
  }

  function truthy(v) {
    if (Array.isArray(v)) { return v.length > 0; }
    if (typeof v === 'string') { return v.length > 0; }
    return !!v;
  }
  function asString(v) {
    if (Array.isArray(v)) { return v.length ? stringOf(v[0]) : ''; }
    return v == null ? '' : String(v);
  }

  function evalExpr(ast, ctx, root) {
    switch (ast.type) {
      case 'str': return ast.v;
      case 'num': return ast.v;
      case 'or':  return truthy(evalExpr(ast.l, ctx, root)) || truthy(evalExpr(ast.r, ctx, root));
      case 'and': return truthy(evalExpr(ast.l, ctx, root)) && truthy(evalExpr(ast.r, ctx, root));
      case 'eq': {
        var a = evalExpr(ast.l, ctx, root), b = evalExpr(ast.r, ctx, root);
        // A node-set compared to a string is true if ANY member matches (XPath 1.0 §3.4).
        var eq;
        if (Array.isArray(a)) {
          eq = a.some(function (n) { return stringOf(n) === asString(b); });
        } else if (Array.isArray(b)) {
          eq = b.some(function (n) { return stringOf(n) === asString(a); });
        } else { eq = asString(a) === asString(b); }
        return ast.neg ? !eq : eq;
      }
      case 'call': {
        var fn = ast.fn, args = ast.args;
        if (fn === 'name' || fn === 'local-name') {
          if (args.length === 0) { return nameOf(ctx); }
          var ns = evalExpr(args[0], ctx, root);
          return Array.isArray(ns) && ns.length ? nameOf(ns[0]) : '';
        }
        if (fn === 'string' || fn === 'text') {
          if (args.length === 0) { return stringOf(ctx); }
          return asString(evalExpr(args[0], ctx, root));
        }
        if (fn === 'not')  { return !truthy(evalExpr(args[0], ctx, root)); }
        if (fn === 'starts-with') {
          return asString(evalExpr(args[0], ctx, root)).indexOf(asString(evalExpr(args[1], ctx, root))) === 0;
        }
        if (fn === 'contains') {
          return asString(evalExpr(args[0], ctx, root)).indexOf(asString(evalExpr(args[1], ctx, root))) >= 0;
        }
        return bad('XPath function ' + fn + '() is outside the supported subset');
      }
      case 'path': return evalPath(ast, ctx, root);
    }
    return bad('unsupported XPath expression node');
  }

  // ── The DOM interfaces ───────────────────────────────────────────────────────────────────────
  function XPathResult(nodes, type) {
    this.__nodes = nodes; this.__i = 0;
    this.resultType = typeof type === 'number' ? type : 4; // UNORDERED_NODE_ITERATOR_TYPE
  }
  XPathResult.ANY_TYPE = 0;
  XPathResult.NUMBER_TYPE = 1;
  XPathResult.STRING_TYPE = 2;
  XPathResult.BOOLEAN_TYPE = 3;
  XPathResult.UNORDERED_NODE_ITERATOR_TYPE = 4;
  XPathResult.ORDERED_NODE_ITERATOR_TYPE = 5;
  XPathResult.UNORDERED_NODE_SNAPSHOT_TYPE = 6;
  XPathResult.ORDERED_NODE_SNAPSHOT_TYPE = 7;
  XPathResult.ANY_UNORDERED_NODE_TYPE = 8;
  XPathResult.FIRST_ORDERED_NODE_TYPE = 9;
  XPathResult.prototype.iterateNext = function () {
    return this.__i < this.__nodes.length ? this.__nodes[this.__i++] : null;
  };
  XPathResult.prototype.snapshotItem = function (i) {
    return i >= 0 && i < this.__nodes.length ? this.__nodes[i] : null;
  };
  Object.defineProperty(XPathResult.prototype, 'snapshotLength', {
    get: function () { return this.__nodes.length; }, configurable: true,
  });
  Object.defineProperty(XPathResult.prototype, 'singleNodeValue', {
    get: function () { return this.__nodes.length ? this.__nodes[0] : null; }, configurable: true,
  });
  Object.defineProperty(XPathResult.prototype, 'booleanValue', {
    get: function () { return this.__nodes.length > 0; }, configurable: true,
  });
  Object.defineProperty(XPathResult.prototype, 'stringValue', {
    get: function () { return this.__nodes.length ? stringOf(this.__nodes[0]) : ''; }, configurable: true,
  });
  Object.defineProperty(XPathResult.prototype, 'numberValue', {
    get: function () {
      return this.__nodes.length ? parseFloat(stringOf(this.__nodes[0])) : NaN;
    }, configurable: true,
  });

  function rootOf(node) {
    var n = node;
    while (n && n.parentNode) { n = n.parentNode; }
    return n || node;
  }

  function runExpression(ast, contextNode, type) {
    var ctx = contextNode || g.document;
    var v = evalExpr(ast, ctx, rootOf(ctx));
    if (Array.isArray(v)) { return new XPathResult(v, type); }
    // A non-node-set result still has to be a well-formed XPathResult, so it is carried as a
    // one-value snapshot and read through the scalar accessors.
    var r = new XPathResult([], type);
    r.__scalar = v;
    Object.defineProperty(r, 'booleanValue', { value: truthy(v), configurable: true });
    Object.defineProperty(r, 'stringValue', { value: asString(v), configurable: true });
    Object.defineProperty(r, 'numberValue', { value: parseFloat(asString(v)), configurable: true });
    return r;
  }

  function XPathExpression(src) { this.__ast = parse(src); this.__src = String(src); }
  XPathExpression.prototype.evaluate = function (contextNode, type) {
    return runExpression(this.__ast, contextNode, type);
  };

  function XPathEvaluator() {}
  XPathEvaluator.prototype.createExpression = function (src) { return new XPathExpression(src); };
  XPathEvaluator.prototype.createNSResolver = function (node) {
    // Namespace resolution is not in the subset; the object exists so a caller can pass it, and any
    // expression that actually uses a prefix throws at parse time rather than resolving wrongly.
    return { lookupNamespaceURI: function () { return null; }, __node: node };
  };
  XPathEvaluator.prototype.evaluate = function (src, contextNode, resolver, type) {
    return runExpression(parse(src), contextNode, type);
  };

  g.XPathEvaluator = XPathEvaluator;
  g.XPathExpression = XPathExpression;
  g.XPathResult = XPathResult;
  if (g.document && typeof g.document.evaluate !== 'function') {
    g.document.evaluate = function (src, contextNode, resolver, type) {
      return runExpression(parse(src), contextNode, type);
    };
  }
})();
"#;
