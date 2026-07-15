//! **G_CLASSLIST — `classList` is an ordered SET, and a no-op must not rewrite the attribute.**
//!
//! Our `DOMTokenList` did naive string work: it split the `class` attribute without **deduplicating**,
//! and it re-serialized on **every** call — including no-ops. Two spec violations fell out of that, and
//! both broke real, high-usage code (`classList` is on every framework's hot path):
//!
//!   1. **No dedup.** `class="a b a"` → `remove('a')` stripped only the FIRST `a`, leaving `"b a"`; and
//!      any modification of `class="a a b"` serialized back `"a a b"` instead of the set `"a b"`.
//!   2. **No-ops rewrote the attribute.** `toggle('x', false)` when `x` is absent must leave the raw
//!      text — `"a  b"`, double space and all — untouched; ours collapsed it to `"a b"`. And `value` /
//!      the stringifier must return the RAW attribute, while `length` / indexing use the deduped set.
//!
//! `dom/nodes/Element-classlist.html`'s "wrong class after modification" cluster (~180 subtests, five
//! node types) hung on exactly this. The set semantics are DOMTokenList §7.1 (ordered set parser/
//! serializer + the per-method "update steps").

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<span id="dedup" class="a a b"></span>
<span id="rmall" class="a b a c a"></span>
<span id="noop"  class="a  b"></span>
<span id="raw"   class="a  b"></span>
<span id="len"   class="a a b"></span>
<span id="tog"   class="x y"></span>
<script>
  var R = [], $ = function (id) { return document.getElementById(id); };

  // (1) modifying an operation serializes the DEDUPED set.
  var d = $('dedup'); d.classList.add('c');
  R.push('dedup:' + d.getAttribute('class'));            // want "a b c", not "a a b c"

  // (2) remove strips EVERY occurrence (a set has at most one).
  var r = $('rmall'); r.classList.remove('a');
  R.push('rmall:' + r.getAttribute('class'));            // want "b c", not "b a c a"

  // (3) a NO-OP toggle must not touch the raw attribute text (whitespace preserved).
  var n = $('noop'); var ret = n.classList.toggle('z', false);
  R.push('noop:' + n.getAttribute('class') + ',' + ret); // want "a  b,false"

  // (4) value / stringifier return the RAW attribute; length/indexing use the deduped set.
  var v = $('raw');
  R.push('value:' + v.classList.value);                  // want "a  b" (raw)
  R.push('str:' + String(v.classList));                  // want "a  b" (raw)

  var l = $('len');
  R.push('len:' + l.classList.length + ',' + l.classList[0] + ',' + l.classList[1]); // want "2,a,b"

  // (5) a toggle that DOES change the set runs the update steps (and dedup-serializes).
  var t = $('tog'); var got = t.classList.toggle('x');
  R.push('tog:' + t.getAttribute('class') + ',' + got);  // want "y,false"

  $('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn classlist_is_an_ordered_set_and_no_ops_preserve_the_raw_attribute() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cls.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "dedup:a b c",
            "a modifying op must serialize the DEDUPED token set — `class=\"a a b\"` + add('c') is \
             `\"a b c\"`, not `\"a a b c\"`",
        ),
        (
            "rmall:b c",
            "remove strips EVERY occurrence — `class=\"a b a c a\"` remove('a') is `\"b c\"`. Ours \
             spliced only the first index because the set was never deduped",
        ),
        (
            "noop:a  b,false",
            "a no-op `toggle('z', false)` must leave the raw attribute — double space and all — \
             untouched, and return false. Ours re-serialized on every call, collapsing whitespace",
        ),
        (
            "value:a  b",
            "`classList.value` (and the stringifier) return the RAW attribute string, not a normalized \
             join of the token set",
        ),
        ("str:a  b", "String(classList) is the stringifier → the raw attribute"),
        (
            "len:2,a,b",
            "`length` and indexed access use the DEDUPED ordered set — `class=\"a a b\"` has length 2",
        ),
        (
            "tog:y,false",
            "a toggle that removes the token runs the update steps and returns false",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_CLASSLIST: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
