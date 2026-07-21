//! **G_URLSEARCHPARAMS_COMPLETE — `URLSearchParams` `sort()` + 2-arg `has`/`delete`.**
//!
//! `sort()` was absent; `has(name, value)` and `delete(name, value)` ignored their value argument (so
//! `has('tab','x')` returned true even for `?tab=y`). These are how routers and query-normalisers
//! compare and canonicalise query strings. The teeth are the resulting param order/membership.
//!
//! Proven RED: remove `sort` and the call throws; revert `has` to one-arg and the value check fails.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
try {
  var u = new URLSearchParams('c=3&a=1&b=2&a=0');
  push('sort-present:' + (typeof u.sort === 'function'));
  u.sort();
  // stable by key: a=1,a=0 keep order; then b, then c.
  push('sorted:' + (u.toString() === 'a=1&a=0&b=2&c=3'));

  var h = new URLSearchParams('tab=x&mode=full');
  push('has-value-yes:' + (h.has('tab', 'x') === true));
  push('has-value-no:' + (h.has('tab', 'y') === false));
  push('has-name:' + (h.has('mode') === true));

  var d = new URLSearchParams('k=1&k=2&k=3');
  d['delete']('k', '2');    // remove only k=2
  push('delete-value:' + (d.toString() === 'k=1&k=3'));
} catch (e) {
  push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn urlsearchparams_sort_and_two_arg_has_delete() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://usp.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("USP-COMPLETE RESULT: {got}");

    for claim in [
        "sort-present:true",
        "sorted:true", // stable sort by key
        "has-value-yes:true",
        "has-value-no:true", // the 2-arg value check actually discriminates
        "has-name:true",
        "delete-value:true", // delete(name, value) removes only the matching pair
    ] {
        assert!(
            got.contains(claim),
            "G_URLSEARCHPARAMS_COMPLETE: expected `{claim}`\n  got: {got}\n\n  \
             `URLSearchParams.sort()` must stably sort by key; `has`/`delete` must honour the optional \
             value argument (match/remove only the pair with that exact value)."
        );
    }
}
