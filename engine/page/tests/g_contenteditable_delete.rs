//! **G_CONTENTEDITABLE_DELETE — Backspace deletes the grapheme before the caret in a contenteditable.**
//!
//! Typing landed (t471 `execCommand('insertText')`, t472 the typed-key default action); an editable is
//! only usable once you can also take a character BACK. Before this, a `Backspace` keydown in a
//! `<div contenteditable>` fired the `KeyboardEvent` and did nothing — the text never shrank. This wires
//! the default `deleteContentBackward` action: on an uncancelled `Backspace`, remove the grapheme before
//! the caret (or the current non-collapsed selection), firing `beforeinput`→`input`
//! (`inputType:'deleteContentBackward'`) — the DELETE counterpart of the shared insert primitive.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `edtext=Hel` — with a caret after "Hello", two `Backspace` presses delete "o" then "l" → "Hel".
//!     RED: drop the `Backspace` arm of the default action and the key fires but nothing deletes — the
//!     text stays "Hello".
//!   * `evs=bi:deleteContentBackward|in:deleteContentBackward|bi:deleteContentBackward|in:deleteContentBackward`
//!     — each delete fires `beforeinput` then `input`, `inputType` `deleteContentBackward`; TWO pairs for
//!     the two presses.
//!   * `boundary=` (empty) then a Backspace at offset 0 fires NO input (nothing to delete) — the log's
//!     `bcount` stays at the two real deletes, proving a no-op Backspace does not spuriously fire `input`.
//!   * `vtext=Keep` — a `contenteditable` whose `beforeinput` handler `preventDefault()`s VETOES the
//!     delete: Backspace leaves its text "Keep".

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

/// One page per test — the `PageContext` is per-process (a second `Page::load` SIGSEGVs), so the normal,
/// boundary, and veto editables all live in ONE document.
#[test]
fn backspace_deletes_the_grapheme_before_the_caret_in_a_contenteditable() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<div id="ed" contenteditable="true">Hello</div>
<div id="veto" contenteditable="true">Keep</div>
<div id="log"></div>
<script>
  var evs = [], bcount = 0;
  var ed = document.getElementById('ed');
  ['beforeinput', 'input'].forEach(function (ty) {
    ed.addEventListener(ty, function (e) {
      evs.push((ty === 'beforeinput' ? 'bi:' : 'in:') + e.inputType);
      if (ty === 'input') { bcount++; }
    });
  });
  // Caret at the end of "Hello".
  var t = ed.firstChild;
  window.getSelection().collapse(t, t.data.length);

  var veto = document.getElementById('veto');
  veto.addEventListener('beforeinput', function (e) { e.preventDefault(); });

  window.__caretEdStart = function () { window.getSelection().collapse(ed.firstChild, 0); };
  window.__caretVetoEnd = function () {
    var vt = veto.firstChild; window.getSelection().collapse(vt, vt.data.length);
  };
  window.__report = function () {
    document.getElementById('log').textContent =
      'edtext=' + ed.textContent + ' evs=' + evs.join('|') +
      ' bcount=' + bcount + ' vtext=' + veto.textContent;
  };
</script></body>"#,
        "https://del.test/",
        &fonts,
        W,
    );

    let ed = node(&p, "#ed");
    // Two backspaces: "Hello" → "Hell" → "Hel".
    p.dispatch_key(ed, "keydown", "Backspace", &fonts, W);
    p.dispatch_key(ed, "keyup", "Backspace", &fonts, W);
    p.dispatch_key(ed, "keydown", "Backspace", &fonts, W);
    p.dispatch_key(ed, "keyup", "Backspace", &fonts, W);

    // A Backspace with the caret at offset 0 must be a no-op — no `input`.
    p.eval_for_test("window.__caretEdStart();");
    p.dispatch_key(ed, "keydown", "Backspace", &fonts, W);
    p.dispatch_key(ed, "keyup", "Backspace", &fonts, W);

    // The veto editable ignores the delete.
    p.eval_for_test("window.__caretVetoEnd();");
    let veto = node(&p, "#veto");
    p.dispatch_key(veto, "keydown", "Backspace", &fonts, W);
    p.dispatch_key(veto, "keyup", "Backspace", &fonts, W);

    p.eval_for_test("window.__report();");
    let root = p.dom().root();
    let out = p
        .dom()
        .text_content(manuk_css::query_selector_all(p.dom(), root, "#log")[0]);
    println!("CONTENTEDITABLE-DELETE RESULT: {out}");

    for claim in [
        "edtext=Hel", // two backspaces removed "o" then "l"
        "evs=bi:deleteContentBackward|in:deleteContentBackward|bi:deleteContentBackward|in:deleteContentBackward",
        "bcount=2",   // exactly two `input`s — the offset-0 Backspace fired none
        "vtext=Keep", // a vetoed Backspace deleted nothing
    ] {
        assert!(
            out.contains(claim),
            "G_CONTENTEDITABLE_DELETE: expected `{claim}` in {out:?}\n  \
             Backspace in a contenteditable must delete the grapheme before the caret and fire \
             beforeinput→input (inputType:deleteContentBackward); a Backspace at offset 0 is a no-op \
             (no input); a vetoed beforeinput deletes nothing."
        );
    }
}
