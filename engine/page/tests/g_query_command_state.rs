//! **G_QUERY_COMMAND_STATE — `document.queryCommandState('bold'|'italic')` reports whether the current
//! selection/caret is already bold/italic, so a toolbar can render its Bold/Italic button pressed.**
//!
//! The read-back half of the formatting brick (`execCommand('bold')` wrap landed t481). A rich-text editor
//! calls this on every `selectionchange` to keep its toolbar in sync; before this the method did not exist,
//! so the call was a `TypeError` that took the toolbar's render path down (the aljazeera "referenced name
//! that does not exist is a crash, not a false" lesson). It works with a COLLAPSED caret (that is how a
//! button lights up as the caret moves through bold text) and mirrors what `execCommand('bold')` produces:
//! a `<b>`/`<strong>` (bold) or `<i>`/`<em>` (italic) ancestor of the selection anchor inside the editable.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `isFn:true` — the method exists (calling it does not throw).
//!   * `inBold:true` — after bolding "world", a caret placed inside the resulting `<b>` reports bold. RED:
//!     drop the `queryCommandState` method / make it always false → `inBold:false`.
//!   * `inPlain:false` — a caret in the un-formatted "hello " run reports NOT bold.
//!   * `inItal:true` — the italic path returns true for a caret inside an `<i>`.

use manuk_page::Page;
use manuk_text::FontContext;

const W: f32 = 800.0;

fn node(p: &Page, sel: &str) -> manuk_dom::NodeId {
    let root = p.dom().root();
    manuk_css::query_selector_all(p.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"))
}

#[test]
fn query_command_state_reports_bold_and_italic() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<div id="ed" contenteditable="true">hello world</div>
<div id="it" contenteditable="true">italic me</div>
<div id="log"></div>
<script>
  window.__run = function () {
    var ed = document.getElementById('ed');
    // Bold "world" (offset 6..11), then read state with the caret INSIDE the new <b>.
    var t = ed.firstChild;
    window.getSelection().setBaseAndExtent(t, 6, t, 11);
    document.execCommand('bold');
    var b = ed.querySelector('b');
    window.getSelection().collapse(b.firstChild, 2);
    var inBold = document.queryCommandState('bold');
    // Caret in the un-formatted "hello " run → not bold.
    window.getSelection().collapse(ed.firstChild, 2);
    var inPlain = document.queryCommandState('bold');
    // Italic editable: italicize all, caret inside the <i>.
    var it = document.getElementById('it');
    window.getSelection().selectAllChildren(it);
    document.execCommand('italic');
    var i = it.querySelector('i');
    window.getSelection().collapse(i.firstChild, 1);
    var inItal = document.queryCommandState('italic');

    document.getElementById('log').textContent =
      'isFn:' + (typeof document.queryCommandState === 'function') +
      ' inBold:' + inBold + ' inPlain:' + inPlain + ' inItal:' + inItal;
  };
</script></body>"#,
        "https://qcs.test/",
        &fonts,
        W,
    );

    p.eval_for_test("window.__run();");
    let out = p.dom().text_content(node(&p, "#log"));
    println!("QUERY-COMMAND-STATE RESULT: {out}");

    for claim in ["isFn:true", "inBold:true", "inPlain:false", "inItal:true"] {
        assert!(
            out.contains(claim),
            "G_QUERY_COMMAND_STATE: expected `{claim}` in {out:?}\n  \
             queryCommandState('bold'|'italic') must exist and report whether the selection anchor is \
             inside a <b>/<strong> or <i>/<em> in the editable."
        );
    }
}
