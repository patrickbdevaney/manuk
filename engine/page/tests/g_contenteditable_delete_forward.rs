//! **G_CONTENTEDITABLE_DELETE_FORWARD — the Delete key removes the grapheme AFTER the caret.**
//!
//! The symmetric partner to Backspace (t473): the Delete key deletes forward. Before this, a `Delete`
//! keydown in a `<div contenteditable>` fired the `KeyboardEvent` and removed nothing. This wires the
//! default `deleteContentForward` action through the SAME `__deleteAtCaret` helper Backspace uses (now
//! direction-parameterised), firing `beforeinput`→`input` (`inputType:'deleteContentForward'`).
//!
//! ## Each claim, and how it goes RED
//!
//!   * `edtext=llo` — with a caret at the START of "Hello", two `Delete` presses remove "H" then "e".
//!     RED: drop the `Delete` handling (or the forward arm of `__deleteAtCaret`) and the key fires but
//!     nothing deletes — the text stays "Hello".
//!   * `evs=bi:deleteContentForward|in:deleteContentForward|bi:deleteContentForward|in:deleteContentForward`
//!     — each delete fires `beforeinput` then `input`, `inputType` `deleteContentForward`; two pairs.
//!   * `dcount=2` — a `Delete` with the caret at the END fires no `input` (nothing after it to delete),
//!     so exactly the two real deletes fired `input`.
//!   * `vtext=Keep` — a `contenteditable` whose `beforeinput` handler `preventDefault()`s vetoes the
//!     forward delete: Delete leaves its text "Keep".

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

/// One page per test — the `PageContext` is per-process (a second `Page::load` SIGSEGVs).
#[test]
fn the_delete_key_removes_the_grapheme_after_the_caret_in_a_contenteditable() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<div id="ed" contenteditable="true">Hello</div>
<div id="veto" contenteditable="true">Keep</div>
<div id="log"></div>
<script>
  var evs = [], dcount = 0;
  var ed = document.getElementById('ed');
  ['beforeinput', 'input'].forEach(function (ty) {
    ed.addEventListener(ty, function (e) {
      evs.push((ty === 'beforeinput' ? 'bi:' : 'in:') + e.inputType);
      if (ty === 'input') { dcount++; }
    });
  });
  // Caret at the START of "Hello".
  window.getSelection().collapse(ed.firstChild, 0);

  var veto = document.getElementById('veto');
  veto.addEventListener('beforeinput', function (e) { e.preventDefault(); });

  window.__caretEdEnd = function () {
    var t = ed.firstChild; window.getSelection().collapse(t, t.data.length);
  };
  window.__caretVetoStart = function () { window.getSelection().collapse(veto.firstChild, 0); };
  window.__report = function () {
    document.getElementById('log').textContent =
      'edtext=' + ed.textContent + ' evs=' + evs.join('|') +
      ' dcount=' + dcount + ' vtext=' + veto.textContent;
  };
</script></body>"#,
        "https://delf.test/",
        &fonts,
        W,
    );

    let ed = node(&p, "#ed");
    // Two forward deletes: "Hello" → "ello" → "llo".
    p.dispatch_key(ed, "keydown", "Delete", &fonts, W);
    p.dispatch_key(ed, "keyup", "Delete", &fonts, W);
    p.dispatch_key(ed, "keydown", "Delete", &fonts, W);
    p.dispatch_key(ed, "keyup", "Delete", &fonts, W);

    // A Delete with the caret at the END must be a no-op — no `input`.
    p.eval_for_test("window.__caretEdEnd();");
    p.dispatch_key(ed, "keydown", "Delete", &fonts, W);
    p.dispatch_key(ed, "keyup", "Delete", &fonts, W);

    // The veto editable ignores the delete.
    p.eval_for_test("window.__caretVetoStart();");
    let veto = node(&p, "#veto");
    p.dispatch_key(veto, "keydown", "Delete", &fonts, W);
    p.dispatch_key(veto, "keyup", "Delete", &fonts, W);

    p.eval_for_test("window.__report();");
    let root = p.dom().root();
    let out = p
        .dom()
        .text_content(manuk_css::query_selector_all(p.dom(), root, "#log")[0]);
    println!("CONTENTEDITABLE-DELETE-FORWARD RESULT: {out}");

    for claim in [
        "edtext=llo", // two forward deletes removed "H" then "e"
        "evs=bi:deleteContentForward|in:deleteContentForward|bi:deleteContentForward|in:deleteContentForward",
        "dcount=2",   // the end-of-text Delete fired no input
        "vtext=Keep", // a vetoed Delete removed nothing
    ] {
        assert!(
            out.contains(claim),
            "G_CONTENTEDITABLE_DELETE_FORWARD: expected `{claim}` in {out:?}\n  \
             the Delete key in a contenteditable must remove the grapheme after the caret and fire \
             beforeinput→input (inputType:deleteContentForward); a Delete at the end is a no-op (no \
             input); a vetoed beforeinput removes nothing."
        );
    }
}
