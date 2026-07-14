//! **G_DISPLAY_CONTENTS — a `display: contents` wrapper vanishes, and its children do not.**
//!
//! `display: contents` means the element generates **no box at all, while its children still do**. It is
//! *not* `display: none` — nothing is hidden. The wrapper simply disappears from the box tree and its
//! children are laid out as though they were the parent's own.
//!
//! Modern CSS leans on it hard, and always for the same reason: a `<div>` wrapping grid items so that a
//! component can own them — **without that `<div>` becoming a grid item itself and collapsing the entire
//! layout into a single cell.** React and friends emit such wrappers constantly.
//!
//! It was not parsed at all. `"contents"` fell through the `match` to `_ => s.display` and stayed
//! **`inline`**, which is the worst available answer:
//!
//! * `display: none` would at least have been *visibly* wrong — the content disappears, and you go
//!   looking.
//! * `inline` keeps the wrapper in the tree as a real inline box that **does** participate in layout. So
//!   its children stop being grid items, the grid sees one anonymous inline child instead of three, and
//!   the whole layout silently collapses into a single cell — with every element still present, still
//!   styled, and in the wrong place.
//!
//! There are two things to prove, and the second is the one that matters:
//!
//! 1. the wrapper's children are **still laid out** (it is not `none`);
//! 2. they are laid out **by the grandparent's formatting context** — i.e. they are the *grid's* items,
//!    not the wrapper's.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>

<!-- Three grid columns. `#wrap` is display:contents, so #a/#b/#c must be the GRID's items — laid out
     side by side in three columns — rather than three children of an inline wrapper stacked in one. -->
<div id="grid" style="display:grid;grid-template-columns:100px 100px 100px;width:300px">
  <div id="wrap" style="display:contents">
    <div id="a" style="height:20px">a</div>
    <div id="b" style="height:20px">b</div>
    <div id="c" style="height:20px">c</div>
  </div>
</div>

<script>
  var R = [];
  function box(id){ var r = document.getElementById(id).getBoundingClientRect(); return [r.x, r.width]; }

  // 1. The wrapper reports `contents` — it used to say `inline`, which is a different property.
  R.push('cs:' + getComputedStyle(document.getElementById('wrap')).display);

  // 2. Its children still exist and still have boxes. (`display: none` would give them width 0.)
  R.push('aw:' + box('a')[1]);

  // 3. AND THE POINT: they are the GRID's items, so they sit in the three columns, side by side.
  //    With the wrapper still in the tree they would all stack at x=0 in a single cell.
  R.push('ax:' + box('a')[0]);
  R.push('bx:' + box('b')[0]);
  R.push('cx:' + box('c')[0]);

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn a_display_contents_wrapper_dissolves_and_its_children_become_the_grandparents_items() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://contents.test/", &fonts, 400.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "cs:contents",
            "`contents` was not parsed at all — it fell through to `_ => s.display` and stayed `inline`, \
             which is a different property with a different meaning",
        ),
        (
            "aw:100",
            "the children must still be LAID OUT. `display: contents` is not `display: none`; nothing is \
             hidden. A width of 0 would mean the wrapper took its children with it",
        ),
        ("ax:0", "…and they are the GRID's items: first column at x=0"),
        (
            "bx:100",
            "second column at x=100. THIS is the whole feature. With the wrapper still in the tree as an \
             inline box, the grid sees ONE anonymous child instead of three — and a/b/c all stack at x=0 \
             in a single cell, every element present, every element styled, and the layout silently \
             collapsed",
        ),
        ("cx:200", "third column at x=200"),
    ] {
        assert!(
            got.contains(claim),
            "G_DISPLAY_CONTENTS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
