//! **Live `HTMLCollection` / `NodeList`** — and the infinite loop that a dead one causes.
//!
//! `element.children` and `getElementsByTagName()` returned **plain arrays**: a snapshot, taken once.
//! Append a child and the collection's `length` did not move. `dom/collections` scored **3/48**.
//!
//! That is not merely a conformance gap. It is a **Bar 0 hang**, hiding in the most common DOM idiom
//! there is:
//!
//! ```js
//! while (el.children.length) { el.removeChild(el.firstChild); }   // "empty this element"
//! ```
//!
//! With a *live* collection this terminates: each removal shortens it. With a **dead** one, `length` is
//! frozen at its initial value, the condition is true forever, and **the tab locks up.** Every
//! `while (list.length)` on the web is this shape. A dead collection does not fail loudly — it *spins*.
//!
//! The inverse trap is just as real and just as common:
//!
//! ```js
//! const items = document.getElementsByTagName('li');
//! for (let i = 0; i < items.length; i++) { items[i].remove(); }   // skips every other item, correctly
//! ```
//!
//! …which is *supposed* to skip, because the list shrinks under the loop. Code in the wild is written
//! against that behaviour, and a snapshot silently changes what it does.
//!
//! ## How it is done, and why it is cheap
//!
//! A `Proxy` whose traps **recompute** the underlying node list on every access. `length` is a getter, not
//! a stored number; `coll[3]` resolves against the tree as it is *now*.
//!
//! It lands cheaply for one reason: **tick 64 gave the DOM real prototypes.** `children` is an accessor on
//! `Element.prototype`, so it can be *wrapped* — take the original getter, and return a live view over it.
//! Before that tick the accessor was an own-property of every element and patching the prototype did
//! nothing at all, silently. This is the second capability to land almost for free on the back of that
//! one, and it is the argument for fixing foundations rather than symptoms.
//!
//! **The cost, stated honestly:** recomputing per access makes `for (i = 0; i < c.length; i++) c[i]`
//! quadratic in the collection's size. For the collections real pages hold (tens of nodes) that is
//! nothing. For a `getElementsByTagName('div')` over a huge document inside a hot loop it would not be —
//! and the fix, when it is needed, is a DOM mutation counter to invalidate a cache, not a return to
//! snapshots. Correct and occasionally slow beats fast and wrong, and *fast and wrong here means a locked
//! tab.*

/// Live collections, installed into the global. See the module docs for the hang a dead one causes.
pub const COLLECTIONS_JS: &str = r#"
(function () {
  'use strict';
  if (typeof document === 'undefined' || typeof Proxy !== 'function') { return; }

  function HTMLCollection() {}
  function NodeList() {}
  globalThis.HTMLCollection = HTMLCollection;
  globalThis.NodeList = NodeList;

  // A live view over `compute()`, which re-reads the tree EVERY time. That is the whole point: a
  // collection whose length is a stored number is a snapshot, and a snapshot turns
  // `while (el.children.length) el.removeChild(el.firstChild)` into an infinite loop.
  var HTML_NS = 'http://www.w3.org/1999/xhtml';
  function isIndex(k) { return typeof k === 'string' && /^(0|[1-9]\d*)$/.test(k); }
  function hasOwn(o, k) { return Object.prototype.hasOwnProperty.call(o, k); }

  function live(compute, proto) {
    var target = Object.create(proto.prototype);
    // Only HTMLCollection is a legacy platform object with NAMED properties (id / HTML `name`).
    // NodeList (childNodes) is indexed-only — exposing names on it would invent properties the spec
    // does not have and break `Object.getOwnPropertyNames(node.childNodes)`.
    var hasNamed = (proto === HTMLCollection);

    // The HTMLCollection "supported property names": the `id` of every element, plus the `name` of
    // every HTML-namespace element, in tree order, no empty strings, deduped (HTML §HTMLCollection).
    function supportedNames(a) {
      var names = [], seen = Object.create(null);
      for (var i = 0; i < a.length; i++) {
        var el = a[i];
        if (!el.getAttribute) continue;
        var id = el.getAttribute('id');
        if (id && !seen[id]) { seen[id] = 1; names.push(id); }
        if (el.namespaceURI === HTML_NS) {
          var nm = el.getAttribute('name');
          if (nm && !seen[nm]) { seen[nm] = 1; names.push(nm); }
        }
      }
      return names;
    }
    // The named property for `key`, or null. `key` is coerced to a string first (so `namedItem(-2)`
    // finds `id="-2"`), and the empty string is never a supported name.
    function namedProp(key) {
      if (!hasNamed || key == null) return null;
      key = String(key);
      if (key === '') return null;
      var a = compute();
      for (var i = 0; i < a.length; i++) {
        var el = a[i];
        if (!el.getAttribute) continue;
        if (el.getAttribute('id') === key) return el;
        if (el.namespaceURI === HTML_NS && el.getAttribute('name') === key) return el;
      }
      return null;
    }

    var methods = {
      item: function (i) {
        var a = compute();
        i = i | 0;
        return (i >= 0 && i < a.length) ? a[i] : null;
      },
    };
    if (hasNamed) {
      // HTMLCollection is NOT declared `iterable<>` — it has `item`/`namedItem` and a default
      // `@@iterator` (from the get trap), but NOT `values`/`entries`/`keys`/`forEach`
      // (dom/collections/HTMLCollection-iterator asserts `"values" in coll === false`).
      methods.namedItem = function (name) { return namedProp(name); };
    } else {
      // NodeList IS `iterable<Node>` — it carries the four generated iterable methods.
      methods.forEach = function (fn, thisArg) {
        var a = compute();
        for (var i = 0; i < a.length; i++) { fn.call(thisArg, a[i], i, this); }
      };
      methods.entries = function () { return compute().map(function (v, i) { return [i, v]; })[Symbol.iterator](); };
      methods.keys = function () { return compute().map(function (_, i) { return i; })[Symbol.iterator](); };
      methods.values = function () { return compute()[Symbol.iterator](); };
    }

    // The legacy-platform-object surface (named properties, expando-shadowing, unenumerable names) is
    // HTMLCollection-only. NodeList (`childNodes`) is the engine's HOTTEST proxy — kept byte-for-byte on
    // its original trap bodies so this tick adds ZERO heap churn to that path. (An earlier version routed
    // NodeList through the richer traps; the extra allocation shifted the shared-batch-runtime heap enough
    // to surface the tracked cross-file UAF on unrelated ranges/traversal files — see docs/wiki/js-engine.md.)
    if (!hasNamed) {
      return new Proxy(target, {
        get: function (t, k, recv) {
          if (k === 'length') return compute().length;
          if (k === Symbol.iterator) return function () { return compute()[Symbol.iterator](); };
          if (isIndex(k)) {
            var a = compute();
            return (+k < a.length) ? a[+k] : undefined;
          }
          if (methods[k]) return methods[k];
          return t[k];
        },
        has: function (t, k) {
          if (k === 'length' || methods[k]) return true;
          if (isIndex(k)) return +k < compute().length;
          return k in t;
        },
        ownKeys: function () {
          var a = compute(), keys = [];
          for (var i = 0; i < a.length; i++) keys.push(String(i));
          keys.push('length');
          return keys;
        },
        getOwnPropertyDescriptor: function (t, k) {
          if (k === 'length') {
            return { value: compute().length, writable: false, enumerable: false, configurable: true };
          }
          if (isIndex(k)) {
            var a = compute();
            if (+k < a.length) {
              return { value: a[+k], writable: false, enumerable: true, configurable: true };
            }
          }
          return Object.getOwnPropertyDescriptor(t, k);
        },
      });
    }

    var proxy = new Proxy(target, {
      get: function (t, k, recv) {
        // `length` is an IDL attribute with a brand check: reading it on an object that merely
        // INHERITS from the collection (`Object.create(coll).length`) is a TypeError, not the count.
        if (k === 'length') {
          if (recv !== proxy) throw new TypeError("'length' called on an object that is not an HTMLCollection");
          return compute().length;
        }
        if (k === Symbol.iterator) return function () { return compute()[Symbol.iterator](); };
        if (isIndex(k)) {
          var a = compute();
          return (+k < a.length) ? a[+k] : undefined;
        }
        if (methods[k]) return methods[k];
        // An own expando shadows a named property (WebIDL named-property visibility), so it wins here.
        if (typeof k === 'string' && hasOwn(t, k)) return t[k];
        // A NAMED property: `form.username` and `collection.someId` both work on a real collection.
        // The named getter is exotic — it resolves for any receiver (so an inheriting object sees it).
        if (typeof k === 'string' && k !== 'constructor') {
          var named = namedProp(k);
          if (named) return named;
        }
        return t[k];
      },
      has: function (t, k) {
        if (k === 'length' || k === Symbol.iterator || methods[k]) return true;
        if (isIndex(k)) return +k < compute().length;
        if (typeof k === 'string' && hasOwn(t, k)) return true;
        if (typeof k === 'string' && namedProp(k)) return true;
        return k in t;
      },
      set: function (t, k, v, receiver) {
        // Assigning through an object that only INHERITS from the collection must land as an ordinary
        // own property on that receiver — WebIDL's [[Set]] passes `ownDesc = undefined` in this case,
        // so the read-only index/named descriptors never block it (dom/collections as-prototype).
        if (receiver !== proxy) {
          var ex = Object.getOwnPropertyDescriptor(receiver, k);
          if (ex) {
            if ('value' in ex) { if (!ex.writable) return false; Object.defineProperty(receiver, k, { value: v }); return true; }
            if (ex.set) { ex.set.call(receiver, v); return true; }
            return false;
          }
          Object.defineProperty(receiver, k, { value: v, writable: true, enumerable: true, configurable: true });
          return true;
        }
        // Read-only index / named property: an expando may not shadow it. Reject → silent in sloppy
        // mode, TypeError in strict.
        if (isIndex(k) && +k < compute().length) return false;
        if (typeof k === 'string' && !hasOwn(t, k) && namedProp(k)) return false;
        t[k] = v;
        return true;
      },
      defineProperty: function (t, k, desc) {
        if (isIndex(k) && +k < compute().length) return false;
        if (typeof k === 'string' && !hasOwn(t, k) && namedProp(k)) return false;
        return Reflect.defineProperty(t, k, desc);
      },
      deleteProperty: function (t, k) {
        if (isIndex(k) && +k < compute().length) return false;
        if (typeof k === 'string' && !hasOwn(t, k) && namedProp(k)) return false;
        return Reflect.deleteProperty(t, k);
      },
      ownKeys: function (t) {
        var a = compute(), keys = [];
        for (var i = 0; i < a.length; i++) keys.push(String(i));
        var names = supportedNames(a);
        for (var j = 0; j < names.length; j++) {
          if (keys.indexOf(names[j]) === -1) keys.push(names[j]);
        }
        // Expando own string keys, then own symbols — `length` lives on the prototype, so it is
        // deliberately NOT an own key (matching Object.getOwnPropertyNames in real browsers).
        var own = Reflect.ownKeys(t);
        for (var m = 0; m < own.length; m++) {
          if (typeof own[m] === 'string' && keys.indexOf(own[m]) === -1) keys.push(own[m]);
        }
        for (var s = 0; s < own.length; s++) {
          if (typeof own[s] === 'symbol') keys.push(own[s]);
        }
        return keys;
      },
      getOwnPropertyDescriptor: function (t, k) {
        if (isIndex(k)) {
          var a = compute();
          if (+k < a.length) {
            return { value: a[+k], writable: false, enumerable: true, configurable: true };
          }
        }
        if (typeof k === 'string' && hasOwn(t, k)) {
          return Reflect.getOwnPropertyDescriptor(t, k);
        }
        // Named properties are [LegacyUnenumerableNamedProperties]: present but NOT enumerable.
        if (typeof k === 'string') {
          var named = namedProp(k);
          if (named) return { value: named, writable: false, enumerable: false, configurable: true };
        }
        return Reflect.getOwnPropertyDescriptor(t, k);
      },
    });
    return proxy;
  }

  function asArray(v) {
    if (!v) return [];
    if (Array.isArray(v)) return v;
    var out = [];
    for (var i = 0; i < v.length; i++) out.push(v[i]);
    return out;
  }

  // ── Wrap the EXISTING accessors rather than reimplementing them.
  //
  // This works only because the DOM has real prototypes (tick 64). Before that, `children` was an
  // own-property of every element, and patching the prototype did nothing — silently. Two capabilities
  // have now landed almost free on the back of that one fix.
  // Find the prototype that actually OWNS `name`, walking up the chain.
  //
  // This matters because of tick 64's stated limit: every DOM member is an own-property of
  // `Node.prototype`, and `Element.prototype` is an empty link in the chain that merely *inherits* them.
  // So `getOwnPropertyDescriptor(Element.prototype, 'children')` is `undefined`, and wrapping there
  // silently does nothing — which is exactly what the first version of this did.
  function ownerOf(start, name) {
    for (var p = start; p; p = Object.getPrototypeOf(p)) {
      var d = Object.getOwnPropertyDescriptor(p, name);
      if (d) return p;
    }
    return null;
  }

  function wrapAccessor(start, name, kind) {
    if (!start) return;
    var proto = ownerOf(start, name);
    if (!proto) return;
    var d = Object.getOwnPropertyDescriptor(proto, name);
    if (!d || typeof d.get !== 'function') return;
    var orig = d.get;
    Object.defineProperty(proto, name, {
      configurable: true,
      enumerable: d.enumerable,
      get: function () {
        var el = this;
        return live(function () { return asArray(orig.call(el)); }, kind);
      },
    });
  }

  function wrapMethod(start, name, kind) {
    if (!start || typeof start[name] !== 'function') return;
    var obj = ownerOf(start, name) || start;
    var orig = obj[name];
    obj[name] = function () {
      var self = this, args = arguments;
      return live(function () { return asArray(orig.apply(self, args)); }, kind);
    };
  }

  var EP = (typeof Element !== 'undefined' && Element.prototype) || null;
  var NP = (typeof Node !== 'undefined' && Node.prototype) || null;

  wrapAccessor(EP, 'children', HTMLCollection);
  wrapAccessor(NP, 'childNodes', NodeList);

  wrapMethod(EP, 'getElementsByTagName', HTMLCollection);
  wrapMethod(EP, 'getElementsByTagNameNS', HTMLCollection);
  wrapMethod(EP, 'getElementsByClassName', HTMLCollection);
  wrapMethod(document, 'getElementsByTagName', HTMLCollection);
  wrapMethod(document, 'getElementsByTagNameNS', HTMLCollection);
  wrapMethod(document, 'getElementsByClassName', HTMLCollection);

  // `querySelectorAll` returns a STATIC NodeList — and that is the spec, not an oversight. Code relies on
  // it not moving under a loop, which is exactly why it exists alongside the live ones. Left alone.
})();
"#;
