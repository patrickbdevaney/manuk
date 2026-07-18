//! **G_ABSPOS_STATIC_IFC — an abs box in an inline-only parent must still GENERATE A BOX.**
//!
//! `position: absolute` with all-`auto` insets sits at its **static position** — the spot it would
//! have occupied in normal flow — and normal flow is the only moment that spot is known. The block
//! child loop records it. The **pure inline formatting context** branch returns before ever reaching
//! that loop, so nothing was recorded, and `position_absolutes` dropped the box on the floor: not
//! mispositioned, *absent*.
//!
//! The shape that hits it is `position: relative` wrapping **only** an absolutely positioned child —
//! the overlay / dropdown / tooltip / portal-root idiom, and the single most common way
//! `position:absolute` is written on the real web.
//!
//! **Why it hid, and why this gate carries the neighbours.** Every adjacent case works, so the bug is
//! invisible unless you test the empty-parent one specifically:
//!
//! * one **block-level sibling** puts the parent on the block path, which records correctly;
//! * a **flex** or **grid** parent returns even earlier, through paths that place abs children by
//!   other means.
//!
//! Those three are asserted here as *controls*. They are not the bug — they are what makes the bug
//! deniable, and a "fix" that made the IFC case work by disturbing them would pass a narrower gate.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<style>
  .rel { position: relative; width: 300px; height: 200px }
  .a   { position: absolute; width: 50px; height: 20px }
</style>
<div class="rel"><div class="a" id="ifc">A</div></div>
<div class="rel">hello <div class="a" id="ifctext">A</div></div>
<div class="rel"><div style="height:7px"></div><div class="a" id="blk">A</div><div id="sib" style="height:5px"></div></div>
<div class="rel" style="display:flex"><div class="a" id="flx">A</div></div>
<div class="rel" style="display:grid"><div class="a" id="grd">A</div></div>
<div id="out">-</div>
<script>
  var R = [];
  var box = function (id) {
    var r = document.getElementById(id).getBoundingClientRect();
    return Math.round(r.width) + 'x' + Math.round(r.height);
  };
  var top = function (id) { return Math.round(document.getElementById(id).getBoundingClientRect().y); };
  ['ifc', 'ifctext', 'blk', 'flx', 'grd'].forEach(function (id) { R.push(id + ':' + box(id)); });
  // The static position is a RELATIONSHIP, not a magic number: #blk follows a 7px block, so it
  // starts 7px into its parent; and being out of flow it must not push #sib, which sits at the
  // same y as #blk rather than below it.
  R.push('blkoff:' + (top('blk') - top('ifc') - 200 - 200));
  R.push('outofflow:' + (top('sib') === top('blk')));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn an_abs_box_in_an_inline_only_parent_still_generates_a_box() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://gasi.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "ifc:50x20",
            "THE BUG: a `position:relative` wrapper whose ONLY child is absolutely positioned is an \
             inline formatting context, and that branch never recorded the static position — so the \
             box was dropped entirely. `0x0` means every overlay, dropdown and portal root written \
             this way renders as nothing at all",
        ),
        (
            "ifctext:50x20",
            "the same parent with text before the abs child is still inline-only, and must also \
             produce the box",
        ),
        (
            "blk:50x20",
            "CONTROL: one block-level sibling routes the parent down the block path, which always \
             worked. If this breaks, the fix damaged the path that was already correct",
        ),
        (
            "flx:50x20",
            "CONTROL: a flex parent returns through an earlier path that places abs children by \
             other means — it must be left alone",
        ),
        (
            "grd:50x20",
            "CONTROL: likewise a grid parent",
        ),
        (
            "blkoff:7",
            "the static position is the WOULD-BE IN-FLOW SPOT, not the containing block's origin: \
             #blk follows a 7px block so it starts 7px down. A `0` here means the box was placed in \
             the top-left corner — which is how a dropdown ends up pinned to the wrong place even \
             when it does render",
        ),
        (
            "outofflow:true",
            "and it must remain OUT OF FLOW: recording a static position must not put the box back \
             into the flow, so #sib ignores it and lands at the same y rather than below it",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_ABSPOS_STATIC_IFC: expected `{claim}` — {why}.\n  got: {got}"
        );
    }
}
