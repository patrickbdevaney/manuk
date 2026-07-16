//! **A real `MutationObserver`** — it was an inert stub that observed nothing.
//!
//! `new MutationObserver(cb)` constructed, `observe()` returned, `takeRecords()` returned `[]`, and the
//! callback **never fired**. `typeof MutationObserver === 'function'` was `true` the whole time, which is
//! why it survived: *a check that only asks whether a name exists is satisfied by a stub.*
//!
//! It is not a niche API. Vue, Alpine and lit use it to react to DOM changes they did not make; every
//! analytics and consent script uses it to notice injected content; every "auto-resize this textarea"
//! helper uses it. And a **stub is worse than an absence here**, because the library feature-detects,
//! finds it, registers, and then silently never reacts.
//!
//! ## How it observes, and the honest limit
//!
//! By **wrapping the mutating methods on the DOM prototypes** — `setAttribute`, `removeAttribute`,
//! `appendChild`, `insertBefore`, `removeChild`, `replaceChild`, `remove`, and the `textContent` /
//! `innerHTML` setters. Every mutation a *script* makes goes through one of those.
//!
//! That is the fourth capability to land on the back of tick 64's real prototypes, and it could not be
//! done at all without them.
//!
//! **The limit, stated rather than discovered later:** mutations made by the **engine itself** — the
//! parser, layout-driven changes, the deferred-script pass — do not go through these wrappers and are not
//! observed. That is *mostly* the right behaviour (an observer registered after parsing is not supposed to
//! see the parse), but it means an observer cannot see engine-internal edits. Wiring the natives to emit
//! records directly is the complete answer, and it is a later tick.
//!
//! ## Delivery is a MICROTASK, and that matters
//!
//! Records are queued and delivered at a microtask checkpoint, not synchronously. Code depends on the
//! batching: a loop that appends 100 nodes must produce **one** callback with 100 records, not 100
//! callbacks. Deliver synchronously and every observer on the page runs 100× per frame — which is not a
//! conformance bug, it is a performance collapse.

/// A real `MutationObserver`, installed into the global.
pub const MUTATION_JS: &str = r#"
(function () {
  'use strict';
  if (typeof document === 'undefined') { return; }

  var EP = (typeof Element !== 'undefined' && Element.prototype) || null;
  var NP = (typeof Node !== 'undefined' && Node.prototype) || null;
  if (!EP || !NP) { return; }

  function ownerOf(start, name) {
    for (var p = start; p; p = Object.getPrototypeOf(p)) {
      if (Object.getOwnPropertyDescriptor(p, name)) return p;
    }
    return null;
  }

  // Every registered (observer, target, options) triple.
  var registry = [];
  var pending = [];        // observers with records waiting for the microtask checkpoint
  var scheduled = false;

  function isAncestor(a, b) {
    for (var n = b; n; n = n.parentNode) { if (n === a) return true; }
    return false;
  }

  function record(r) {
    for (var i = 0; i < registry.length; i++) {
      var reg = registry[i], o = reg.options, t = reg.target;
      // Does this registration cover the mutated node?
      var covers = (r.target === t) || (o.subtree && isAncestor(t, r.target));
      if (!covers) continue;
      if (r.type === 'attributes' && !o.attributes) continue;
      if (r.type === 'childList' && !o.childList) continue;
      if (r.type === 'characterData' && !o.characterData) continue;
      if (r.type === 'attributes' && o.attributeFilter &&
          o.attributeFilter.indexOf(r.attributeName) < 0) continue;

      var rec = Object.create(MutationRecord.prototype);
      rec.type = r.type;
      rec.target = r.target;
      rec.attributeName = r.attributeName || null;
      rec.attributeNamespace = null;
      rec.addedNodes = r.addedNodes || [];
      rec.removedNodes = r.removedNodes || [];
      rec.previousSibling = r.previousSibling || null;
      rec.nextSibling = r.nextSibling || null;
      // `oldValue` is only supplied when the registration ASKED for it. Handing it over unasked is a
      // conformance failure that looks like generosity.
      rec.oldValue =
        (r.type === 'attributes' && o.attributeOldValue) ||
        (r.type === 'characterData' && o.characterDataOldValue)
          ? (r.oldValue === undefined ? null : r.oldValue)
          : null;

      reg.observer._records.push(rec);
      if (pending.indexOf(reg.observer) < 0) pending.push(reg.observer);
    }
    if (pending.length && !scheduled) {
      scheduled = true;
      // A MICROTASK, not a synchronous call. A loop that appends 100 nodes must produce ONE callback with
      // 100 records, not 100 callbacks — deliver synchronously and every observer on the page runs 100
      // times per frame, which is a performance collapse rather than a conformance bug.
      var flush = function () {
        scheduled = false;
        var list = pending.slice();
        pending.length = 0;
        for (var i = 0; i < list.length; i++) {
          var obs = list[i];
          var recs = obs._records;
          obs._records = [];
          if (recs.length) {
            try { obs._cb.call(obs, recs, obs); } catch (e) {
              if (typeof __reportError === 'function') __reportError(e);
            }
          }
        }
      };
      if (typeof queueMicrotask === 'function') queueMicrotask(flush);
      else Promise.resolve().then(flush);
    }
  }

  function MutationRecord() {}
  globalThis.MutationRecord = MutationRecord;

  function MutationObserver(cb) {
    if (typeof cb !== 'function') {
      throw new TypeError('MutationObserver requires a callback');
    }
    this._cb = cb;
    this._records = [];
  }
  MutationObserver.prototype.observe = function (target, options) {
    options = options || {};
    var o = {
      childList: !!options.childList,
      attributes: options.attributes !== undefined ? !!options.attributes : !!options.attributeFilter,
      characterData: !!options.characterData,
      subtree: !!options.subtree,
      attributeOldValue: !!options.attributeOldValue,
      characterDataOldValue: !!options.characterDataOldValue,
      attributeFilter: options.attributeFilter || null,
    };
    if (o.attributeOldValue || o.attributeFilter) o.attributes = true;
    if (o.characterDataOldValue) o.characterData = true;
    if (!o.childList && !o.attributes && !o.characterData) {
      throw new TypeError('observe() requires childList, attributes or characterData');
    }
    // Re-observing the same target REPLACES the previous registration, per spec.
    for (var i = registry.length - 1; i >= 0; i--) {
      if (registry[i].observer === this && registry[i].target === target) registry.splice(i, 1);
    }
    registry.push({ observer: this, target: target, options: o });
  };
  MutationObserver.prototype.disconnect = function () {
    for (var i = registry.length - 1; i >= 0; i--) {
      if (registry[i].observer === this) registry.splice(i, 1);
    }
    this._records = [];
  };
  MutationObserver.prototype.takeRecords = function () {
    var r = this._records;
    this._records = [];
    return r;
  };
  globalThis.MutationObserver = MutationObserver;

  // ── The wrappers. Every mutation a SCRIPT makes goes through one of these.
  function wrapMethod(start, name, before, after) {
    var proto = ownerOf(start, name);
    if (!proto || typeof proto[name] !== 'function') return;
    var orig = proto[name];
    proto[name] = function () {
      var pre = before ? before.call(this, arguments) : null;
      var out = orig.apply(this, arguments);
      if (after) after.call(this, arguments, pre, out);
      return out;
    };
  }

  wrapMethod(EP, 'setAttribute', function (a) {
    return this.getAttribute ? this.getAttribute(String(a[0])) : null;
  }, function (a, old) {
    record({ type: 'attributes', target: this, attributeName: String(a[0]), oldValue: old });
  });
  wrapMethod(EP, 'removeAttribute', function (a) {
    return this.getAttribute ? this.getAttribute(String(a[0])) : null;
  }, function (a, old) {
    if (old !== null) record({ type: 'attributes', target: this, attributeName: String(a[0]), oldValue: old });
  });

  wrapMethod(NP, 'appendChild', null, function (a, _p, out) {
    record({ type: 'childList', target: this, addedNodes: [out || a[0]],
             previousSibling: (out && out.previousSibling) || null });
  });
  wrapMethod(NP, 'insertBefore', null, function (a, _p, out) {
    record({ type: 'childList', target: this, addedNodes: [out || a[0]], nextSibling: a[1] || null });
  });
  wrapMethod(NP, 'removeChild', function (a) {
    var n = a[0];
    return n ? { prev: n.previousSibling, next: n.nextSibling } : null;
  }, function (a, pre, out) {
    record({ type: 'childList', target: this, removedNodes: [out || a[0]],
             previousSibling: pre && pre.prev, nextSibling: pre && pre.next });
  });
  wrapMethod(NP, 'replaceChild', null, function (a, _p, out) {
    record({ type: 'childList', target: this, addedNodes: [a[0]], removedNodes: [out || a[1]] });
  });
  wrapMethod(EP, 'remove', function () {
    return { parent: this.parentNode, prev: this.previousSibling, next: this.nextSibling };
  }, function (a, pre) {
    if (pre && pre.parent) {
      record({ type: 'childList', target: pre.parent, removedNodes: [this],
               previousSibling: pre.prev, nextSibling: pre.next });
    }
  });

  // The setters. `el.innerHTML = ''` and `el.textContent = ''` are how most code clears a subtree, and an
  // observer that misses them misses most of what happens on a page.
  ['innerHTML', 'textContent'].forEach(function (name) {
    var proto = ownerOf(NP, name) || ownerOf(EP, name);
    if (!proto) return;
    var d = Object.getOwnPropertyDescriptor(proto, name);
    if (!d || typeof d.set !== 'function') return;
    var set = d.set, get = d.get;
    Object.defineProperty(proto, name, {
      configurable: true,
      enumerable: d.enumerable,
      get: get,
      set: function (v) {
        var removed = this.childNodes ? Array.prototype.slice.call(this.childNodes) : [];
        set.call(this, v);
        var added = this.childNodes ? Array.prototype.slice.call(this.childNodes) : [];
        record({ type: 'childList', target: this, addedNodes: added, removedNodes: removed });
      },
    });
  });
})();
"#;
