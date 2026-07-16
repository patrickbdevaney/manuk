//! **G_COLLECTION_NAMED_PROPS — `HTMLCollection` is a WebIDL legacy platform object.**
//!
//! `getElementsByTagName(...)` returns a live `HTMLCollection`, and the spec makes it a *legacy platform
//! object* with **indexed** and **named** properties (HTML §HTMLCollection + WebIDL §legacy platform
//! object). Our proxy exposed the indices and a bare `namedItem`, but got the object-model surface wrong
//! in ways an entire cluster of `dom/collections/` tests checks:
//!
//!   1. **`Object.getOwnPropertyNames`** must return `[...indices, ...supported names, ...expandos]` and
//!      must **NOT** include `length` (it lives on the prototype). The supported names are every `id` plus
//!      every HTML-namespace `name`, tree order, deduped, no empty strings.
//!   2. **Named properties are `[LegacyUnenumerableNamedProperties]`** → present but `enumerable: false`,
//!      `writable: false`, `configurable: true`.
//!   3. **An expando may not shadow a read-only index/named property** — `coll["some-id"] = 5` and
//!      `Object.defineProperty(coll, "some-id", …)` and `delete coll["some-id"]` are all rejected (silent
//!      in sloppy mode, `TypeError` in strict), and the named value survives.
//!   4. **An expando on a name that is NOT yet a supported name is a real own property** and *shadows* a
//!      named property that later appears (WebIDL named-property visibility).
//!   5. **The empty string is never a supported name**: `"" in coll` is false and `coll[""]` is undefined.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<span id="a1"></span>
<span id="a2" name="n2"></span>
<script>
  var R = [], push = function (k, v) { R.push(k + '=' + v); };
  var spans = document.getElementsByTagName('span');

  // (1) own property names: indices, then id/name in tree order, then expando — NO 'length'.
  spans.expo = 7;
  push('names', Object.getOwnPropertyNames(spans).join(','));  // 0,1,a1,a2,n2,expo

  // (2) a named property is present-but-unenumerable, read-only, configurable.
  var d = Object.getOwnPropertyDescriptor(spans, 'a1');
  push('desc', (d && d.value === document.getElementById('a1')) + ',' +
               d.enumerable + ',' + d.writable + ',' + d.configurable); // true,false,false,true

  // (3) an expando cannot shadow an existing named property.
  var item = spans['a1'];
  spans['a1'] = 5;                       // sloppy → silent no-op
  push('shadow', spans['a1'] === item);  // true (named survives)
  var threwSet = false;
  try { (function () { 'use strict'; spans['a1'] = 5; })(); } catch (e) { threwSet = (e instanceof TypeError); }
  push('strictset', threwSet);           // true
  var threwDef = false;
  try { Object.defineProperty(spans, 'a1', { value: 5 }); } catch (e) { threwDef = (e instanceof TypeError); }
  push('def', threwDef);                 // true

  // (4) an expando on a not-yet-supported name shadows a name added later.
  spans['later'] = 9;
  var s = document.createElement('span'); s.id = 'later'; document.body.appendChild(s);
  push('expandowins', spans['later']);   // 9 (expando shadows the new named element)

  // (5) empty string is never a supported name.
  push('empty', ('' in spans) + ',' + (spans[''] === undefined)); // false,true

  // (6) `length` is an IDL attribute with a brand check: reading it on an object that only INHERITS
  //     from the collection is a TypeError. But the named getter is exotic and resolves for any
  //     receiver, and assigning through the inheriting object lands as its OWN property.
  var proto = Object.create(spans);
  var threwLen = false;
  try { proto.length; } catch (e) { threwLen = (e instanceof TypeError); }
  push('protolen', threwLen);              // true
  push('protonamed', proto['a1'] === document.getElementById('a1')); // true (inherited named getter)
  proto['a1'] = 'own';
  push('protoset', proto['a1'] + ',' + (spans['a1'] === document.getElementById('a1'))); // own,true

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn htmlcollection_is_a_legacy_platform_object() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://coll.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "names=0,1,a1,a2,n2,expo",
            "Object.getOwnPropertyNames = indices, then supported names (id + HTML name, deduped, tree \
             order), then expandos — and NEVER `length` (it is a prototype accessor, not an own prop)",
        ),
        (
            "desc=true,false,false,true",
            "a named property is [LegacyUnenumerableNamedProperties]: value present, enumerable:false, \
             writable:false, configurable:true",
        ),
        (
            "shadow=true",
            "a sloppy `coll['a1'] = 5` over a read-only named property is a silent no-op; the element \
             survives",
        ),
        ("strictset=true", "the same assignment in strict mode throws TypeError"),
        (
            "def=true",
            "Object.defineProperty over an existing named property throws TypeError (the legacy \
             platform object rejects the redefinition)",
        ),
        (
            "expandowins=9",
            "an expando set BEFORE the name existed is a real own property and shadows the named \
             property that appears later (WebIDL named-property visibility)",
        ),
        (
            "empty=false,true",
            "the empty string is never a supported name: `'' in coll` is false and `coll['']` is \
             undefined",
        ),
        (
            "protolen=true",
            "`length` has a brand check: `Object.create(coll).length` throws TypeError, it does not \
             return the count",
        ),
        (
            "protonamed=true",
            "the named getter is exotic and resolves for any receiver, so an object inheriting from \
             the collection still sees the named element",
        ),
        (
            "protoset=own,true",
            "assigning a named key through an inheriting object lands as that object's OWN property \
             (WebIDL [[Set]] with a non-collection receiver) and does not disturb the collection",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_COLLECTION_NAMED_PROPS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
