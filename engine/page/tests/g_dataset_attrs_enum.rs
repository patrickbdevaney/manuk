//! **G_DATASET_ATTRS_ENUM — `DOMStringMap` (`dataset`) and `NamedNodeMap` (`attributes`) enumerate.**
//!
//! Both are WebIDL legacy platform objects backed by a `Proxy`, and both were missing the
//! `ownKeys`/`getOwnPropertyDescriptor` traps — so `Object.getOwnPropertyNames(el.dataset)` saw the empty
//! target (`[]`) and `Object.getOwnPropertyNames(el.attributes)` returned `['0','1','length']` instead of
//! the attribute names. `dom/collections/{domstringmap,namednodemap}-supported-property-names` check this.
//!
//!   - **`dataset`** supported names = each `data-*` attribute, prefix stripped and dash→camel-cased
//!     (`data-date-of-birth` → `dateOfBirth`, `data-` → `""`, `data-id-` → `"id-"`). Enumerable.
//!   - **`attributes`** supported names = indices ++ the attribute qualified names, no `length` (a
//!     prototype accessor). Named props are `[LegacyUnenumerableNamedProperties]`.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="ds" data-id="1" data-user="jd" data-date-of-birth></div>
<div id="edge" data-="012"></div>
<div id="simple" class="fancy">x</div>
<script>
  var R = [], push = function (k, v) { R.push(k + '=' + v); };

  // DOMStringMap: data-* → camelCase supported names, in order.
  var ds = document.getElementById('ds').dataset;
  push('dsnames', Object.getOwnPropertyNames(ds).join(','));      // id,user,dateOfBirth
  ds.middleName = 'm';                                            // creates data-middle-name
  push('dsafterset', Object.getOwnPropertyNames(ds).join(','));   // id,user,dateOfBirth,middleName
  push('dsedge', Object.getOwnPropertyNames(document.getElementById('edge').dataset).join(',')); // "" (empty)
  var dsd = Object.getOwnPropertyDescriptor(ds, 'id');
  push('dsdesc', dsd.value + ',' + dsd.enumerable + ',' + dsd.writable);   // 1,true,true

  // NamedNodeMap: indices ++ attribute names, no 'length'; names are unenumerable.
  var at = document.getElementById('simple').attributes;
  push('atnames', Object.getOwnPropertyNames(at).join(','));      // 0,1,id,class
  var ad = Object.getOwnPropertyDescriptor(at, 'class');
  push('atdesc', (ad.value && ad.value.name === 'class' && ad.value.value === 'fancy') + ',' + ad.enumerable + ',' + ad.writable); // true,false,false
  push('atlen', Object.getOwnPropertyNames(at).indexOf('length')); // -1 (length is not an own key)

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn dataset_and_namednodemap_enumerate_their_supported_names() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://enum.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("dsnames=id,user,dateOfBirth", "dataset supported names = each data-* stripped + dash→camel-cased, in order"),
        ("dsafterset=id,user,dateOfBirth,middleName", "a name set via `dataset.middleName` (→ data-middle-name) appears"),
        ("dsedge=", "`data-` maps to the empty-string property name"),
        ("dsdesc=1,true,true", "dataset named props are enumerable and writable data properties"),
        ("atnames=0,1,id,class", "NamedNodeMap own names = indices ++ attribute qualified names"),
        ("atdesc=true,false,false", "NamedNodeMap named props are [LegacyUnenumerableNamedProperties]: unenumerable, read-only Attr"),
        ("atlen=-1", "`length` is a prototype accessor, never an own key of the NamedNodeMap"),
    ] {
        assert!(
            got.contains(claim),
            "G_DATASET_ATTRS_ENUM: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
