//! **G_SPLIT_TEXT — `Text.prototype.splitText(offset)` and the `wholeText` getter.**
//!
//! `splitText(offset)` splits a Text node at `offset` (UTF-16 units): the node keeps `[0, offset)` and a
//! new Text node — inserted as its next sibling — takes `[offset, len)`. `wholeText` reads back the
//! concatenated data of a contiguous run of Text siblings, so a run `splitText` broke apart reads as one
//! string again. Both were missing: `splitText` was `TypeError` (not a function), `wholeText`
//! `undefined`. Each assertion is one spec guarantee:
//!
//! * **the split** — `"hello world".splitText(5)` leaves `"hello"` and returns a Text `" world"`.
//! * **tree wiring** — the new node is the original's `nextSibling`, so the parent gains a child.
//! * **`wholeText`** — reads the whole contiguous run back as `"hello world"`.
//! * **validity** — `offset > length` is an `IndexSizeError`.
//! * **detached** — splitting a parentless Text node still returns the tail node.
//!
//! Own binary: two SpiderMonkey-backed `Page::load`s in one process reuse the JS runtime and can trip the
//! tracked reflector-teardown UAF (see the flexbox-relayout Bar-0 note). One JS gate = one process.
//!
//! **Falsifiable:** before this tick `splitText` threw `TypeError` at the first call, leaving `#out` at
//! its `-` sentinel — RED. The native turns it GREEN.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="host">hello world</div>
<div id="out">-</div>
<script>
  var R = [];
  function ck(l, g) { R.push(l + ':' + g); }
  function thrown(fn){ try { fn(); return 'NO_THROW'; } catch(e){ return e.name; } }

  var host = document.getElementById('host');
  var t = host.firstChild;
  var s = t.splitText(5);
  ck('orig', t.data);
  ck('new', s.data);
  ck('sType', s.nodeType);
  ck('sibling', t.nextSibling === s);
  ck('kids', host.childNodes.length);
  ck('whole', t.wholeText);
  ck('bad', thrown(function(){ t.splitText(999); }));

  var d = document.createTextNode('abcdef');
  var d2 = d.splitText(2);
  ck('det', d.data + '|' + d2.data);

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn split_text_and_whole_text() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://st.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("orig:hello", "the original node keeps [0, offset)"),
        ("new: world", "the returned node takes [offset, len)"),
        ("sType:3", "the split-off node is a Text node"),
        (
            "sibling:true",
            "the new node is inserted as the original's next sibling",
        ),
        ("kids:2", "the parent gains the split-off child"),
        (
            "whole:hello world",
            "wholeText concatenates the contiguous Text run",
        ),
        (
            "bad:IndexSizeError",
            "offset > length throws IndexSizeError",
        ),
        (
            "det:ab|cdef",
            "splitText works on a detached (parentless) Text node",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_SPLIT_TEXT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
