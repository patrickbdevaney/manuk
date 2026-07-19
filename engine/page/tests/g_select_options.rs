//! **G_SELECT_OPTIONS — `s.options[i]` was a TypeError, and a throw takes the page with it.**
//!
//! Tick 253's named residue. `select.options` did not exist, so `s.options[i]` threw and
//! `s.options.length` threw — which is the worst shape a missing feature takes in this engine: a
//! page that merely *enumerates its own options* to relabel, filter or count them **stopped
//! executing at that line**. Reading as empty would have been better; reading correctly is better
//! still.
//!
//! ## How each assertion here can go RED
//!
//! - **`options` spans `<optgroup>`.** RED, run: walk children instead of descendants and a grouped
//!   select reports zero options.
//! - **`selectedOptions` honours the IMPLICIT first selection.** A single-select with nothing marked
//!   still has a selected option. RED, run: filter `selectedOptions` on the `selected` attribute for
//!   the single-select case too, and `s.selectedOptions.length` is 0 on a perfectly ordinary
//!   untouched select — which is precisely what pages guard on.
//! - **`<select multiple>` reports every marked option.** RED, run: take the single-select path for
//!   both and a multi-select reports exactly one.
//! - **`option.index` is its position within the owning select**, counted across optgroups. RED,
//!   run: return the child index within its immediate parent and every option in the second
//!   optgroup reports 0.

use manuk_page::Page;
use manuk_text::FontContext;

const W: f32 = 800.0;

/// One test — a `PageContext` is per-process, see `g_mouse_actuation.rs`.
#[test]
fn a_select_exposes_its_options_as_an_indexable_collection() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<select id="s">
  <option value="a">A</option>
  <option value="b" selected>B</option>
  <option value="c">C</option>
</select>

<!-- nothing marked: still has a selected option -->
<select id="implicit"><option value="x">X</option><option value="y">Y</option></select>

<!-- options live inside optgroups, and `index` counts across them -->
<select id="grouped">
  <optgroup label="Warm"><option>Red</option><option>Orange</option></optgroup>
  <optgroup label="Cool"><option>Blue</option></optgroup>
</select>

<select id="multi" multiple>
  <option value="p" selected>P</option>
  <option value="q">Q</option>
  <option value="r" selected>R</option>
</select>

<div id="log"></div>
<script>
  window.__report = function () {
    var s = document.getElementById('s');
    var im = document.getElementById('implicit');
    var g = document.getElementById('grouped');
    var m = document.getElementById('multi');

    // Enumerating your own options — the pattern that used to THROW.
    var labels = [];
    for (var i = 0; i < s.options.length; i++) { labels.push(s.options[i].value); }

    var gIdx = [];
    for (var j = 0; j < g.options.length; j++) { gIdx.push(g.options[j].index); }

    var mSel = [];
    for (var k = 0; k < m.selectedOptions.length; k++) { mSel.push(m.selectedOptions[k].value); }

    document.getElementById('log').textContent =
      'len=' + s.options.length + ' labels=' + labels.join(',') +
      ' opt1=' + s.options[1].value + ' opt1sel=' + s.options[1].selected +
      ' selOpts=' + s.selectedOptions.length + ' selOptVal=' + s.selectedOptions[0].value +
      ' implicitSel=' + im.selectedOptions.length + ' implicitVal=' + im.selectedOptions[0].value +
      ' gLen=' + g.options.length + ' gIdx=' + gIdx.join(',') + ' gLast=' + g.options[2].value +
      ' mSel=' + mSel.join(',');
  };
</script></body>"#,
        "https://options.test/",
        &fonts,
        W,
    );

    let root = p.dom().root();
    let lg = manuk_css::query_selector_all(p.dom(), root, "#log")[0];
    p.eval_for_test("window.__report();");
    let out = p.dom().text_content(lg);

    // If `options` still threw, the script dies before writing anything at all.
    assert!(
        !out.is_empty(),
        "the report never ran — `s.options` threw and took the script with it, which is the exact \
         failure this gate exists for"
    );

    assert!(out.contains("len=3"), "got: {out}");
    assert!(
        out.contains("labels=a,b,c"),
        "enumerating a select's own options is the commonest thing a page does with them; got: {out}"
    );
    assert!(
        out.contains("opt1=b") && out.contains("opt1sel=true"),
        "got: {out}"
    );

    assert!(
        out.contains("selOpts=1") && out.contains("selOptVal=b"),
        "a single-select's selectedOptions is its one selected option; got: {out}"
    );
    assert!(
        out.contains("implicitSel=1") && out.contains("implicitVal=x"),
        "an untouched single-select STILL has a selected option — its first. Filtering on the \
         `selected` attribute reports 0 here, on a perfectly ordinary select, and pages guard on \
         exactly that. got: {out}"
    );

    assert!(
        out.contains("gLen=3") && out.contains("gLast=Blue"),
        "options inside <optgroup> belong to the select; got: {out}"
    );
    assert!(
        out.contains("gIdx=0,1,2"),
        "`option.index` is the position within the OWNING SELECT, counted across optgroups — a \
         child-index-within-parent answer makes the second group restart at 0. got: {out}"
    );

    assert!(
        out.contains("mSel=p,r"),
        "<select multiple> reports EVERY marked option; got: {out}"
    );
}
