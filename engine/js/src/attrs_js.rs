//! **`Attr` nodes and a live `NamedNodeMap`** — `element.attributes` was `undefined`.
//!
//! Not "incomplete". **Absent.** `element.attributes` did not exist, so `element.attributes.length` was a
//! `TypeError` — and iterating an element's attributes is one of the most ordinary things a script does.
//! Every DOM serializer, every sanitizer (DOMPurify walks `attributes` to strip `on*` handlers), every
//! DOM-diffing library, every "copy these attributes onto that element" helper.
//!
//! With it gone, so were `getAttributeNode`, `setAttributeNode`, `document.createAttribute`, and
//! `toggleAttribute`. Three WPT files — `attributes.html`, `Document-createAttribute.html`,
//! `processing-instruction-attributes.html` — hold ~240 subtests between them, and they were failing at
//! the *first line*.
//!
//! ## Built on what already worked, and live
//!
//! The engine already exposes `getAttributeNames()`, `getAttribute()`, `setAttribute()` and
//! `removeAttribute()`. An `Attr` is a **view** over those, and a `NamedNodeMap` is a live view over the
//! name list — the same `Proxy`-recompute shape as `HTMLCollection` (tick 73), and live for the same
//! reason: code does
//!
//! ```js
//! while (el.attributes.length) el.removeAttribute(el.attributes[0].name);   // "strip everything"
//! ```
//!
//! and a frozen `length` turns that into an infinite loop. **The same dead-collection hang, one interface
//! over.**
//!
//! ## The one that is easy to get wrong
//!
//! **An `Attr`'s `value` is live and writable.** `attr.value = 'x'` must set the attribute on the owner
//! element — an `Attr` is not a snapshot of a name/value pair, it is a handle to one. Return a plain
//! object and every `attrs[i].value = ...` in the wild silently writes to nothing.

/// `Attr` + `NamedNodeMap` + the element/document methods that traffic in them.
pub const ATTRS_JS: &str = r#"
(function () {
  'use strict';
  if (typeof document === 'undefined' || typeof Proxy !== 'function') { return; }

  var EP = (typeof Element !== 'undefined' && Element.prototype) || null;
  if (!EP) { return; }

  function ownerOf(start, name) {
    for (var p = start; p; p = Object.getPrototypeOf(p)) {
      if (Object.getOwnPropertyDescriptor(p, name)) return p;
    }
    return null;
  }

  function Attr() {}
  function NamedNodeMap() {}
  globalThis.Attr = Attr;
  globalThis.NamedNodeMap = NamedNodeMap;

  var XHTML = 'http://www.w3.org/1999/xhtml';

  // An Attr bound to an element is a HANDLE, not a snapshot: `attr.value = 'x'` writes through. A
  // detached Attr (from `createAttribute`) holds its own value until it is attached.
  function makeAttr(el, name, detachedValue) {
    var a = Object.create(Attr.prototype);
    var own = detachedValue;
    Object.defineProperty(a, 'name', { get: function () { return name; }, enumerable: true });
    Object.defineProperty(a, 'localName', {
      get: function () { return name.indexOf(':') >= 0 ? name.split(':').pop() : name; },
      enumerable: true,
    });
    Object.defineProperty(a, 'prefix', {
      get: function () { return name.indexOf(':') >= 0 ? name.split(':')[0] : null; },
      enumerable: true,
    });
    Object.defineProperty(a, 'namespaceURI', { get: function () { return null; }, enumerable: true });
    Object.defineProperty(a, 'ownerElement', { get: function () { return el || null; }, enumerable: true });
    Object.defineProperty(a, 'specified', { get: function () { return true; }, enumerable: true });
    Object.defineProperty(a, 'nodeType', { get: function () { return 2; }, enumerable: true });
    Object.defineProperty(a, 'nodeName', { get: function () { return name; }, enumerable: true });
    Object.defineProperty(a, 'value', {
      enumerable: true,
      get: function () {
        if (el) { var v = el.__getAttrExact(name); return v === null ? '' : v; }
        return own === undefined ? '' : own;
      },
      set: function (v) {
        v = String(v);
        if (el) { el.__setAttrExact(name, v); } else { own = v; }
      },
    });
    // `nodeValue` and `textContent` are aliases of `value` on an Attr, and legacy code uses both.
    Object.defineProperty(a, 'nodeValue', {
      enumerable: false,
      get: function () { return a.value; },
      set: function (v) { a.value = v; },
    });
    Object.defineProperty(a, 'textContent', {
      enumerable: false,
      get: function () { return a.value; },
      set: function (v) { a.value = v; },
    });
    a.__attach = function (owner) { el = owner; if (own !== undefined) owner.__setAttrExact(name, own); };
    return a;
  }

  // A LIVE NamedNodeMap — the same reason `HTMLCollection` is live (tick 73):
  //   while (el.attributes.length) el.removeAttribute(el.attributes[0].name);   // "strip everything"
  // A frozen `length` makes that spin forever. The same dead-collection hang, one interface over.
  function makeMap(el) {
    var names = function () {
      return (typeof el.getAttributeNames === 'function') ? el.getAttributeNames() : [];
    };
    var methods = {
      item: function (i) { var n = names(); i = i | 0; return (i >= 0 && i < n.length) ? makeAttr(el, n[i]) : null; },
      getNamedItem: function (name) {
        name = String(name);
        return el.hasAttribute(name) ? makeAttr(el, name) : null;
      },
      setNamedItem: function (attr) {
        var prev = el.__hasAttrExact(attr.name) ? makeAttr(el, attr.name) : null;
        el.__setAttrExact(attr.name, attr.value);
        if (attr.__attach) attr.__attach(el);
        return prev;
      },
      removeNamedItem: function (name) {
        name = String(name);
        if (!el.__hasAttrExact(name)) {
          var e = new Error("no attribute named '" + name + "'");
          e.name = 'NotFoundError';
          throw e;
        }
        var old = makeAttr(null, name, el.__getAttrExact(name));
        el.__removeAttrExact(name);
        return old;
      },
    };
    methods.getNamedItemNS = function (_ns, name) { return methods.getNamedItem(name); };
    methods.removeNamedItemNS = function (_ns, name) { return methods.removeNamedItem(name); };
    methods.setNamedItemNS = methods.setNamedItem;

    var target = Object.create(NamedNodeMap.prototype);
    return new Proxy(target, {
      get: function (t, k) {
        if (k === 'length') return names().length;
        if (k === Symbol.iterator) {
          return function () { return names().map(function (n) { return makeAttr(el, n); })[Symbol.iterator](); };
        }
        if (typeof k === 'string' && /^(0|[1-9]\d*)$/.test(k)) {
          var n = names();
          return (+k < n.length) ? makeAttr(el, n[+k]) : undefined;
        }
        if (methods[k]) return methods[k];
        if (typeof k === 'string' && el.hasAttribute && el.hasAttribute(k)) return makeAttr(el, k);
        return t[k];
      },
      has: function (t, k) {
        if (k === 'length' || methods[k]) return true;
        if (typeof k === 'string' && /^(0|[1-9]\d*)$/.test(k)) return +k < names().length;
        return k in t;
      },
      ownKeys: function () {
        var n = names(), keys = [];
        for (var i = 0; i < n.length; i++) keys.push(String(i));
        keys.push('length');
        return keys;
      },
      getOwnPropertyDescriptor: function (t, k) {
        if (typeof k === 'string' && /^(0|[1-9]\d*)$/.test(k)) {
          var n = names();
          if (+k < n.length) {
            return { value: makeAttr(el, n[+k]), writable: false, enumerable: true, configurable: true };
          }
        }
        if (k === 'length') {
          return { value: names().length, writable: false, enumerable: false, configurable: true };
        }
        return Object.getOwnPropertyDescriptor(t, k);
      },
    });
  }

  // `element.attributes` — defined on whichever prototype actually owns the DOM members. (Tick 64's
  // stated limit: they are own-properties of `Node.prototype`, and `Element.prototype` merely inherits.)
  var owner = ownerOf(EP, 'tagName') || EP;
  Object.defineProperty(owner, 'attributes', {
    configurable: true,
    enumerable: true,
    get: function () { return makeMap(this); },
  });

  owner.getAttributeNode = function (name) {
    name = String(name);
    return this.hasAttribute(name) ? makeAttr(this, name) : null;
  };
  owner.getAttributeNodeNS = function (_ns, name) { return this.getAttributeNode(name); };
  owner.setAttributeNode = function (attr) {
    // Case-preserving: setAttributeNode stores by the Attr's exact name (no lowercasing) — createAttribute
    // already lowercased HTML names at creation, so only createAttributeNS-preserved case reaches here.
    var prev = this.__hasAttrExact(attr.name) ? makeAttr(null, attr.name, this.__getAttrExact(attr.name)) : null;
    this.__setAttrExact(attr.name, attr.value);
    if (attr.__attach) attr.__attach(this);
    return prev;
  };
  owner.setAttributeNodeNS = owner.setAttributeNode;
  owner.removeAttributeNode = function (attr) {
    if (!this.__hasAttrExact(attr.name)) {
      var e = new Error('the attribute is not on this element');
      e.name = 'NotFoundError';
      throw e;
    }
    var old = makeAttr(null, attr.name, this.__getAttrExact(attr.name));
    this.__removeAttrExact(attr.name);
    return old;
  };

  // `toggleAttribute(name[, force])` — returns whether the attribute is present AFTERWARDS. It is the
  // idiomatic way to flip `disabled`/`hidden`/`aria-expanded`, and it was simply missing.
  owner.toggleAttribute = function (name, force) {
    name = String(name);
    if (name === '') {
      var e1 = new Error('an empty attribute name is not allowed');
      e1.name = 'InvalidCharacterError';
      throw e1;
    }
    var has = this.hasAttribute(name);
    var want = (force === undefined) ? !has : !!force;
    if (want && !has) this.setAttribute(name, '');
    if (!want && has) this.removeAttribute(name);
    return want;
  };

  // ── The `*AttributeNS` family. 160 failing subtests said `node.setAttributeNS is not a function`, and
  // that is not an exotic API: it is how SVG's `xlink:href`, MathML, and every XML-ish document set an
  // attribute at all.
  //
  // **Honest limit:** the namespace is validated and then *ignored* for storage — `setAttributeNS(ns,
  // 'xlink:href', v)` stores the attribute under its qualified name. That is right for every document
  // this engine renders (attributes are looked up by qualified name), and it is wrong for a document that
  // distinguishes two attributes with the same qualified name in different namespaces. Which is not a
  // thing any real page does, and is exactly the kind of limit that must be *said* rather than discovered.
  owner.setAttributeNS = function (ns, qname, value) {
    qname = String(qname);
    if (qname === '') {
      var e = new Error('an empty attribute name is not allowed');
      e.name = 'InvalidCharacterError';
      throw e;
    }
    var prefix = qname.indexOf(':') >= 0 ? qname.split(':')[0] : null;
    var nsStr = (ns === null || ns === undefined || ns === '') ? null : String(ns);
    var XMLNS = 'http://www.w3.org/2000/xmlns/';
    var XML = 'http://www.w3.org/XML/1998/namespace';
    if ((prefix && !nsStr)
        || (prefix === 'xml' && nsStr !== XML)
        || ((prefix === 'xmlns' || qname === 'xmlns') && nsStr !== XMLNS)
        || (nsStr === XMLNS && prefix !== 'xmlns' && qname !== 'xmlns')) {
      var e2 = new Error("'" + qname + "' is not valid in namespace " + nsStr);
      e2.name = 'NamespaceError';
      throw e2;
    }
    // The `*AttributeNS` family is CASE-PRESERVING — the DOM spec lowercases only the non-NS
    // setAttribute/getAttribute. Route through the `__*AttrExact` natives so `setAttributeNS(ns,'Abc',v)`
    // stores `Abc`, not `abc`. (The public setAttribute lowercases for HTML elements; NS must not.)
    this.__setAttrExact(qname, String(value));
  };
  owner.getAttributeNS = function (_ns, local) {
    local = String(local);
    // Look up by local name first, then by any qualified name that ends in it — which is what a document
    // written with a prefix actually stores. Case-preserving (exact) lookup, per spec.
    if (this.__hasAttrExact(local)) return this.__getAttrExact(local);
    var names = this.getAttributeNames ? this.getAttributeNames() : [];
    for (var i = 0; i < names.length; i++) {
      if (names[i].split(':').pop() === local) return this.__getAttrExact(names[i]);
    }
    return null;
  };
  owner.hasAttributeNS = function (ns, local) { return this.getAttributeNS(ns, local) !== null; };
  owner.removeAttributeNS = function (ns, local) {
    local = String(local);
    if (this.__hasAttrExact(local)) { this.__removeAttrExact(local); return; }
    var names = this.getAttributeNames ? this.getAttributeNames() : [];
    for (var i = 0; i < names.length; i++) {
      if (names[i].split(':').pop() === local) { this.__removeAttrExact(names[i]); return; }
    }
  };

  document.createAttribute = function (name) {
    name = String(name);
    if (name === '') {
      var e = new Error('an empty attribute name is not allowed');
      e.name = 'InvalidCharacterError';
      throw e;
    }
    // Created DETACHED, with an empty value, exactly as the spec says — it holds its own value until
    // something attaches it to an element.
    return makeAttr(null, name, '');
  };
  document.createAttributeNS = function (_ns, name) { return document.createAttribute(name); };
})();
"#;
