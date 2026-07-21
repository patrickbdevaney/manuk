//! **G_FORMDATA_ITERATORS — `FormData.keys()` / `values()`.**
//!
//! `for (const name of formData.keys())` / `for (const v of formData.values())` — how a page walks a
//! form's fields. `entries()` and `forEach()` were present but `keys()`/`values()` were absent, an
//! asymmetry that broke exactly those loops (`formData.keys is not a function`).
//!
//! Proven RED: remove `keys` and `present`/`keys` fail while the loop throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
try {
  var fd = new FormData();
  fd.append('a', '1');
  fd.append('b', '2');
  fd.append('a', '3');

  push('present:' + (typeof fd.keys === 'function' && typeof fd.values === 'function'));

  var names = [];
  for (var k of fd.keys()) { names.push(k); }
  push('keys:' + (names.join(',') === 'a,b,a'));

  var vals = [];
  for (var v of fd.values()) { vals.push(v); }
  push('values:' + (vals.join(',') === '1,2,3'));
} catch (e) {
  push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn formdata_keys_and_values_iterators() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fd.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("FORMDATA-ITER RESULT: {got}");

    for claim in [
        "present:true",
        "keys:true",   // field names in order, including the duplicate
        "values:true", // field values in order
    ] {
        assert!(
            got.contains(claim),
            "G_FORMDATA_ITERATORS: expected `{claim}`\n  got: {got}\n\n  \
             `FormData.keys()`/`values()` must iterate the field names/values in insertion order \
             (duplicates preserved), matching `entries()`."
        );
    }
}
