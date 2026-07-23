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

  // ── `form.elements` — the HTMLFormControlsCollection, and the RadioNodeList a radio group needs.
  //
  // `form.elements` was `undefined` ENTIRELY: `for (var i=0;i<form.elements.length;i++)` — the shape
  // every form-serialization and validation library uses — threw `can't access property "length",
  // form.elements is undefined`, and `form.elements['fieldname']` / `.namedItem('fieldname')` were the
  // same throw. It is a legacy platform object like `HTMLCollection`, but with two differences that
  // matter and are the reason it gets its own builder rather than routing through `live()` (which is the
  // hot childNodes/children proxy — deliberately left untouched, see the UAF note above):
  //   * its members are the LISTED elements in tree order — button / fieldset / input (EXCEPT
  //     `type=image`, which is a submit button the collection omits) / object / output / select /
  //     textarea — not every descendant.
  //   * its NAMED getter returns a `RadioNodeList` (not a single element) when more than one control
  //     shares a name. That is exactly a radio group: `form.elements['plan']` must yield a list whose
  //     `.value` is the CHECKED radio's value, or code that reads `form.elements.plan.value` silently
  //     gets the first radio instead of the selected one.
  //
  // KNOWN LIMIT, stated honestly: association is by SUBTREE (a control the form contains), not by the
  // `form=` attribute reassociating a control that lives elsewhere in the document. That is the ~99%
  // case; the reassociation edge is a separate follow-on.
  function HTMLFormControlsCollection() {}
  function RadioNodeList() {}
  globalThis.HTMLFormControlsCollection = HTMLFormControlsCollection;
  globalThis.RadioNodeList = RadioNodeList;

  function isRadio(el) {
    return el.tagName === 'INPUT' && (el.getAttribute('type') || '').toLowerCase() === 'radio';
  }
  // The form's (or fieldset's) listed controls, in tree order, minus image inputs.
  function listedControls(form) {
    var all = form.querySelectorAll('button,fieldset,input,object,output,select,textarea'), out = [];
    for (var i = 0; i < all.length; i++) {
      var el = all[i];
      if (el.tagName === 'INPUT' && (el.getAttribute('type') || '').toLowerCase() === 'image') continue;
      out.push(el);
    }
    return out;
  }

  // A RadioNodeList over `compute()` (the same-named controls). `.value` reads/writes the checked radio.
  function radioNodeList(compute) {
    var target = Object.create(RadioNodeList.prototype);
    function get(k) {
      var a = compute();
      if (k === 'length') return a.length;
      if (k === Symbol.iterator) return function () { return compute()[Symbol.iterator](); };
      if (k === 'item') return function (i) { i = i | 0; return (i >= 0 && i < a.length) ? a[i] : null; };
      if (k === 'value') {
        for (var i = 0; i < a.length; i++) { if (isRadio(a[i]) && a[i].checked) return a[i].value; }
        return '';
      }
      if (isIndex(k)) return (+k < a.length) ? a[+k] : undefined;
      return undefined;
    }
    return new Proxy(target, {
      get: function (t, k) { var v = get(k); return v === undefined ? t[k] : v; },
      set: function (t, k, v) {
        if (k === 'value') {
          var a = compute(), s = String(v);
          for (var i = 0; i < a.length; i++) { if (isRadio(a[i]) && a[i].value === s) a[i].checked = true; }
          return true;
        }
        t[k] = v; return true;
      },
      has: function (t, k) {
        if (k === 'length' || k === 'value' || k === 'item' || k === Symbol.iterator) return true;
        if (isIndex(k)) return +k < compute().length;
        return k in t;
      },
    });
  }

  // The controls collection for one form/fieldset. Named lookup by `name` (HTML ns) then `id`; a single
  // match returns the element, multiple returns a RadioNodeList (HTMLFormControlsCollection §named).
  function controlsCollection(form) {
    var target = Object.create(HTMLFormControlsCollection.prototype);
    function compute() { return listedControls(form); }
    function matches(key) {
      if (key == null) return [];
      key = String(key); if (key === '') return [];
      var a = compute(), out = [];
      for (var i = 0; i < a.length; i++) {
        var el = a[i];
        if (!el.getAttribute) continue;
        if (el.namespaceURI === HTML_NS && el.getAttribute('name') === key) { out.push(el); continue; }
        if (el.getAttribute('id') === key) out.push(el);
      }
      return out;
    }
    function named(key) {
      var m = matches(key);
      if (m.length === 0) return null;
      if (m.length === 1) return m[0];
      return radioNodeList(function () { return matches(key); });
    }
    var methods = {
      item: function (i) { var a = compute(); i = i | 0; return (i >= 0 && i < a.length) ? a[i] : null; },
      namedItem: function (n) { return named(n); },
    };
    return new Proxy(target, {
      get: function (t, k) {
        if (k === 'length') return compute().length;
        if (k === Symbol.iterator) return function () { return compute()[Symbol.iterator](); };
        if (isIndex(k)) { var a = compute(); return (+k < a.length) ? a[+k] : undefined; }
        if (methods[k]) return methods[k];
        if (typeof k === 'string' && hasOwn(t, k)) return t[k];
        if (typeof k === 'string' && k !== 'constructor') { var nm = named(k); if (nm) return nm; }
        return t[k];
      },
      has: function (t, k) {
        if (k === 'length' || k === Symbol.iterator || methods[k]) return true;
        if (isIndex(k)) return +k < compute().length;
        if (typeof k === 'string' && hasOwn(t, k)) return true;
        if (typeof k === 'string' && matches(k).length) return true;
        return k in t;
      },
      ownKeys: function (t) {
        var a = compute(), keys = [], seen = Object.create(null), names = [];
        for (var i = 0; i < a.length; i++) keys.push(String(i));
        for (var j = 0; j < a.length; j++) {
          var el = a[j]; if (!el.getAttribute) continue;
          var id = el.getAttribute('id'); if (id && !seen[id]) { seen[id] = 1; names.push(id); }
          if (el.namespaceURI === HTML_NS) { var nm = el.getAttribute('name'); if (nm && !seen[nm]) { seen[nm] = 1; names.push(nm); } }
        }
        for (var n = 0; n < names.length; n++) { if (keys.indexOf(names[n]) === -1) keys.push(names[n]); }
        return keys;
      },
      getOwnPropertyDescriptor: function (t, k) {
        if (isIndex(k)) { var a = compute(); if (+k < a.length) return { value: a[+k], writable: false, enumerable: true, configurable: true }; }
        if (typeof k === 'string' && matches(k).length) return { value: named(k), writable: false, enumerable: false, configurable: true };
        return Reflect.getOwnPropertyDescriptor(t, k);
      },
    });
  }

  // The accessor lives on the shared element prototype with a tag guard — the same device the form
  // methods (`requestSubmit`, `checkValidity`) already use, since forms have no distinct reflector
  // prototype here. A non-form element reads `undefined`, exactly as it has no such property.
  if (EP) {
    Object.defineProperty(EP, 'elements', {
      configurable: true,
      enumerable: false,
      get: function () {
        return (this.tagName === 'FORM' || this.tagName === 'FIELDSET') ? controlsCollection(this) : undefined;
      },
    });
  }

  // ── `control.labels` and `label.control` — the label⇄control association a11y needs.
  //
  // Both were `undefined`. `input.labels` is how every accessibility helper and form library finds the
  // text that names a control (`input.labels[0].textContent`), and `label.control` is the inverse. A
  // `<label for=x>` with no live `.control`, and an `<input>` with no `.labels`, means a screen-reader
  // shim or a "focus the field when its label is clicked" handler has nothing to walk.
  var LABELABLE = { BUTTON: 1, INPUT: 1, METER: 1, OUTPUT: 1, PROGRESS: 1, SELECT: 1, TEXTAREA: 1 };
  function isLabelable(el) {
    if (!el || !LABELABLE[el.tagName]) return false;
    // A hidden input is NOT labelable (HTML §labelable elements).
    if (el.tagName === 'INPUT' && (el.getAttribute('type') || '').toLowerCase() === 'hidden') return false;
    return true;
  }
  // `label.control`: the `for=` target if it is labelable, else the FIRST labelable descendant.
  function labelControl(label) {
    var f = label.getAttribute ? label.getAttribute('for') : null;
    if (f !== null && f !== undefined) {
      var doc = label.ownerDocument || (typeof document !== 'undefined' ? document : null);
      var t = doc && doc.getElementById ? doc.getElementById(f) : null;
      return (t && isLabelable(t)) ? t : null;
    }
    var desc = label.querySelectorAll ? label.querySelectorAll('button,input,meter,output,progress,select,textarea') : [];
    for (var i = 0; i < desc.length; i++) { if (isLabelable(desc[i])) return desc[i]; }
    return null;
  }
  // The labels associated with `el`, in tree order: every `<label>` whose `.control` resolves to `el`.
  function labelsFor(el) {
    var doc = el.ownerDocument || (typeof document !== 'undefined' ? document : null);
    var all = (doc && doc.getElementsByTagName) ? doc.getElementsByTagName('label') : [];
    var out = [];
    for (var i = 0; i < all.length; i++) { if (labelControl(all[i]) === el) out.push(all[i]); }
    return out;
  }
  // A STATIC NodeList — deliberately NOT routed through `live()` (the hot childNodes proxy whose heap
  // sensitivity is documented above); `.labels` is read far too rarely to earn a proxy per access.
  function staticNodeList(arr) {
    var nl = Object.create(NodeList.prototype);
    for (var i = 0; i < arr.length; i++) { nl[i] = arr[i]; }
    Object.defineProperty(nl, 'length', { value: arr.length, enumerable: false, configurable: true });
    nl.item = function (i) { i = i | 0; return (i >= 0 && i < arr.length) ? arr[i] : null; };
    nl.forEach = function (fn, t) { for (var j = 0; j < arr.length; j++) { fn.call(t, arr[j], j, this); } };
    nl[Symbol.iterator] = function () { return arr[Symbol.iterator](); };
    return nl;
  }
  if (EP) {
    Object.defineProperty(EP, 'labels', {
      configurable: true,
      enumerable: false,
      get: function () {
        if (isLabelable(this)) { return staticNodeList(labelsFor(this)); }
        // A labelable-in-general element that is currently non-labelable (a hidden input) → null,
        // per HTMLInputElement.labels. Everything else simply has no such property.
        return (this.tagName === 'INPUT') ? null : undefined;
      },
    });
    Object.defineProperty(EP, 'control', {
      configurable: true,
      enumerable: false,
      get: function () { return this.tagName === 'LABEL' ? labelControl(this) : undefined; },
    });
  }

  // ── The `<table>` DOM: `table.rows`/`tBodies`/`tHead`/`tFoot`, section `.rows`, `tr.cells`,
  //    `tr.rowIndex`/`sectionRowIndex`, `td.cellIndex`.
  //
  // The whole read surface was `undefined` — `table.rows` and `tr.cells` are how a data-grid library, a
  // sortable-table widget, and every accessibility "what row/column is this cell in" walk read a table,
  // and `rowIndex`/`cellIndex` are the coordinates they report. `table.rows` is NOT document order: it is
  // **thead rows, then tbody + direct `<tr>` rows in tree order, then tfoot rows** — a sort widget that
  // reads document order silently mis-numbers a table whose `<tfoot>` is written before its `<tbody>`.
  function childRowsOf(section) {
    var out = [], c = section.children;
    for (var i = 0; i < c.length; i++) { if (c[i].tagName === 'TR') out.push(c[i]); }
    return out;
  }
  function tableRows(table) {
    var head = [], body = [], foot = [], c = table.children;
    for (var i = 0; i < c.length; i++) {
      var ch = c[i], tn = ch.tagName;
      if (tn === 'THEAD') { head = head.concat(childRowsOf(ch)); }
      else if (tn === 'TFOOT') { foot = foot.concat(childRowsOf(ch)); }
      else if (tn === 'TBODY') { body = body.concat(childRowsOf(ch)); }
      else if (tn === 'TR') { body.push(ch); }
    }
    return head.concat(body, foot);
  }
  function rowCells(tr) {
    var out = [], c = tr.children;
    for (var i = 0; i < c.length; i++) { var tn = c[i].tagName; if (tn === 'TD' || tn === 'TH') out.push(c[i]); }
    return out;
  }
  function ancestorTag(el, tag) {
    for (var p = el.parentNode; p; p = p.parentNode) { if (p.tagName === tag) return p; }
    return null;
  }
  function firstChildTag(el, tag) {
    var c = el.children;
    for (var i = 0; i < c.length; i++) { if (c[i].tagName === tag) return c[i]; }
    return null;
  }
  if (EP) {
    Object.defineProperty(EP, 'rows', {
      configurable: true, enumerable: false,
      get: function () {
        var t = this.tagName, self = this;
        if (t === 'TABLE') { return live(function () { return tableRows(self); }, HTMLCollection); }
        if (t === 'THEAD' || t === 'TBODY' || t === 'TFOOT') { return live(function () { return childRowsOf(self); }, HTMLCollection); }
        return undefined;
      },
    });
    Object.defineProperty(EP, 'tBodies', {
      configurable: true, enumerable: false,
      get: function () {
        if (this.tagName !== 'TABLE') return undefined;
        var self = this;
        return live(function () {
          var out = [], c = self.children;
          for (var i = 0; i < c.length; i++) { if (c[i].tagName === 'TBODY') out.push(c[i]); }
          return out;
        }, HTMLCollection);
      },
    });
    Object.defineProperty(EP, 'tHead', {
      configurable: true, enumerable: false,
      get: function () { return this.tagName === 'TABLE' ? firstChildTag(this, 'THEAD') : undefined; },
    });
    Object.defineProperty(EP, 'tFoot', {
      configurable: true, enumerable: false,
      get: function () { return this.tagName === 'TABLE' ? firstChildTag(this, 'TFOOT') : undefined; },
    });
    Object.defineProperty(EP, 'cells', {
      configurable: true, enumerable: false,
      get: function () { var self = this; return this.tagName === 'TR' ? live(function () { return rowCells(self); }, HTMLCollection) : undefined; },
    });
    Object.defineProperty(EP, 'rowIndex', {
      configurable: true, enumerable: false,
      get: function () {
        if (this.tagName !== 'TR') return undefined;
        var table = ancestorTag(this, 'TABLE');
        if (!table) return -1;
        var rows = tableRows(table);
        for (var i = 0; i < rows.length; i++) { if (rows[i] === this) return i; }
        return -1;
      },
    });
    Object.defineProperty(EP, 'sectionRowIndex', {
      configurable: true, enumerable: false,
      get: function () {
        if (this.tagName !== 'TR') return undefined;
        var section = this.parentNode;
        if (!section) return -1;
        var st = section.tagName;
        // A `<tr>` that is a direct child of `<table>` belongs to the implicit tbody — its siblings are
        // the table's direct `<tr>` children, which `childRowsOf` returns for a TABLE just as for a section.
        var rows = (st === 'THEAD' || st === 'TBODY' || st === 'TFOOT' || st === 'TABLE') ? childRowsOf(section) : [];
        for (var i = 0; i < rows.length; i++) { if (rows[i] === this) return i; }
        return -1;
      },
    });
    Object.defineProperty(EP, 'cellIndex', {
      configurable: true, enumerable: false,
      get: function () {
        if (this.tagName !== 'TD' && this.tagName !== 'TH') return undefined;
        var tr = this.parentNode;
        if (!tr || tr.tagName !== 'TR') return -1;
        var cells = rowCells(tr);
        for (var i = 0; i < cells.length; i++) { if (cells[i] === this) return i; }
        return -1;
      },
    });
  }

  // ── The `<table>` WRITE API — insertRow/deleteRow/insertCell/deleteCell + the section/caption
  //    convenience methods a data-grid library uses to BUILD a table programmatically.
  //
  // `table.insertRow()` and `tr.insertCell()` were `undefined`, so any code that constructs rows in JS
  // (the classic non-framework pattern, and what many grid/spreadsheet widgets still emit) threw. The
  // index rules are the spec's and they are exact: an out-of-range index is an IndexSizeError (not a
  // clamp — code branches on the throw), `-1` means "at the end", and inserting into an empty table
  // MATERIALISES a `<tbody>` rather than dropping a bare `<tr>` into the table.
  function indexSizeError(msg) {
    var e = new Error(msg || 'Index or size is negative or greater than the allowed amount');
    e.name = 'IndexSizeError';
    return e;
  }
  function ownerDoc(el) { return el.ownerDocument || (typeof document !== 'undefined' ? document : null); }
  function lastChildTag(el, tag) {
    var c = el.children, found = null;
    for (var i = 0; i < c.length; i++) { if (c[i].tagName === tag) found = c[i]; }
    return found;
  }
  if (EP) {
    EP.insertRow = function (index) {
      if (this.tagName !== 'TABLE' && this.tagName !== 'THEAD' && this.tagName !== 'TBODY' && this.tagName !== 'TFOOT') {
        return undefined;
      }
      if (index === undefined) index = -1;
      index = index | 0;
      var isTable = this.tagName === 'TABLE';
      var rows = isTable ? tableRows(this) : childRowsOf(this);
      if (index < -1 || index > rows.length) { throw indexSizeError(); }
      var doc = ownerDoc(this), tr = doc.createElement('tr');
      if (!isTable) {
        if (index === -1 || index === rows.length) { this.appendChild(tr); }
        else { this.insertBefore(tr, rows[index]); }
        return tr;
      }
      if (rows.length === 0) {
        var tbody = lastChildTag(this, 'TBODY');
        if (!tbody) { tbody = doc.createElement('tbody'); this.appendChild(tbody); }
        tbody.appendChild(tr);
      } else if (index === -1 || index === rows.length) {
        var lastRow = rows[rows.length - 1];
        lastRow.parentNode.appendChild(tr);
      } else {
        var ref = rows[index];
        ref.parentNode.insertBefore(tr, ref);
      }
      return tr;
    };
    EP.deleteRow = function (index) {
      if (this.tagName !== 'TABLE' && this.tagName !== 'THEAD' && this.tagName !== 'TBODY' && this.tagName !== 'TFOOT') {
        return undefined;
      }
      index = index | 0;
      var isTable = this.tagName === 'TABLE';
      var rows = isTable ? tableRows(this) : childRowsOf(this);
      if (index === -1) { index = rows.length - 1; }
      if (index < 0 || index >= rows.length) { throw indexSizeError(); }
      var row = rows[index];
      row.parentNode.removeChild(row);
    };
    EP.insertCell = function (index) {
      if (this.tagName !== 'TR') { return undefined; }
      if (index === undefined) index = -1;
      index = index | 0;
      var cells = rowCells(this);
      if (index < -1 || index > cells.length) { throw indexSizeError(); }
      var td = ownerDoc(this).createElement('td');
      if (index === -1 || index === cells.length) { this.appendChild(td); }
      else { this.insertBefore(td, cells[index]); }
      return td;
    };
    EP.deleteCell = function (index) {
      if (this.tagName !== 'TR') { return undefined; }
      index = index | 0;
      var cells = rowCells(this);
      if (index === -1) { index = cells.length - 1; }
      if (index < 0 || index >= cells.length) { throw indexSizeError(); }
      this.removeChild(cells[index]);
    };
    // createTHead/createTFoot REUSE an existing section; createTBody always makes a new one;
    // createCaption reuses. thead is inserted before the first tbody/tfoot/tr; tfoot and tbody append.
    function createOrGetSection(table, tag) {
      var existing = firstChildTag(table, tag);
      if (existing) return existing;
      var doc = ownerDoc(table), sec = doc.createElement(tag.toLowerCase());
      if (tag === 'THEAD') {
        var before = null, c = table.children;
        for (var i = 0; i < c.length; i++) {
          var tn = c[i].tagName;
          if (tn === 'TBODY' || tn === 'TFOOT' || tn === 'TR') { before = c[i]; break; }
        }
        if (before) { table.insertBefore(sec, before); } else { table.appendChild(sec); }
      } else {
        table.appendChild(sec);
      }
      return sec;
    }
    EP.createTHead = function () { return this.tagName === 'TABLE' ? createOrGetSection(this, 'THEAD') : undefined; };
    EP.createTFoot = function () { return this.tagName === 'TABLE' ? createOrGetSection(this, 'TFOOT') : undefined; };
    EP.createTBody = function () {
      if (this.tagName !== 'TABLE') return undefined;
      var doc = ownerDoc(this), tb = doc.createElement('tbody'), last = lastChildTag(this, 'TBODY');
      if (last && last.nextSibling) { this.insertBefore(tb, last.nextSibling); }
      else { this.appendChild(tb); }
      return tb;
    };
    EP.createCaption = function () {
      if (this.tagName !== 'TABLE') return undefined;
      var existing = firstChildTag(this, 'CAPTION');
      if (existing) return existing;
      var doc = ownerDoc(this), cap = doc.createElement('caption');
      if (this.firstChild) { this.insertBefore(cap, this.firstChild); } else { this.appendChild(cap); }
      return cap;
    };
    function deleteFirstSection(table, tag) {
      if (table.tagName !== 'TABLE') return undefined;
      var s = firstChildTag(table, tag);
      if (s) { table.removeChild(s); }
    }
    EP.deleteTHead = function () { return deleteFirstSection(this, 'THEAD'); };
    EP.deleteTFoot = function () { return deleteFirstSection(this, 'TFOOT'); };
    EP.deleteCaption = function () { return deleteFirstSection(this, 'CAPTION'); };
  }

  // ── `element.form` — the FORM OWNER of a form-associated element.
  //
  // `input.form` was `undefined`, so every form library that groups controls by their owning form
  // (`input.form === thisForm`), and every framework that reads `el.form` to find where to submit, got
  // nothing — including the `form=` REASSOCIATION case, where a control lives OUTSIDE the `<form>` and
  // names it by id. This also silently broke `ElementInternals.form`, which delegates to `host.form`.
  //
  // The owner is: if the element carries a `form=` attribute, the element with that id **iff it is a
  // `<form>`** (an id pointing at a non-form yields NO owner, per spec — not the nearest ancestor); else
  // the nearest ancestor `<form>`. An `<option>` reports its `<select>`'s owner; a `<label>` reports its
  // labeled control's owner.
  var FORM_ASSOCIATED = { INPUT: 1, SELECT: 1, TEXTAREA: 1, BUTTON: 1, FIELDSET: 1, OBJECT: 1, OUTPUT: 1 };
  function formOwner(el) {
    var fa = el.getAttribute ? el.getAttribute('form') : null;
    if (fa !== null && fa !== undefined) {
      var doc = ownerDoc(el);
      var f = (doc && doc.getElementById) ? doc.getElementById(fa) : null;
      return (f && f.tagName === 'FORM') ? f : null;
    }
    return ancestorTag(el, 'FORM');
  }
  if (EP) {
    Object.defineProperty(EP, 'form', {
      configurable: true,
      enumerable: false,
      get: function () {
        var t = this.tagName;
        if (FORM_ASSOCIATED[t]) { return formOwner(this); }
        if (t === 'OPTION') { var sel = ancestorTag(this, 'SELECT'); return sel ? formOwner(sel) : null; }
        if (t === 'LABEL') { var c = this.control; return c ? formOwner(c) : null; }
        return undefined; // not a form-associated element — no such property
      },
    });
  }
})();
"#;
