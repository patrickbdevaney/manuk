//! **G_CONTENTEDITABLE_TYPING — a printable key typed into a `contenteditable` inserts the character.**
//!
//! Tick 471 landed the programmatic insert (`execCommand('insertText')`); this lands the path a REAL
//! user or the agent uses — pressing a key. Before it, `dispatch_key` fired the `KeyboardEvent` and
//! stopped: a `<div contenteditable>` received the keystroke, ran the page's handlers, and then did
//! NOTHING — the character never appeared. A contenteditable is only actually editable once the default
//! typed-character action inserts the pressed key at the caret and fires the `beforeinput`/`input`
//! (`inputType:'insertText'`) pair — the SAME `__insertTextAtCaret` primitive `execCommand('insertText')`
//! uses, so the two typing paths cannot drift.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `edtext=HiX` — with a caret after "Hi", a `keydown`/`keyup` of `X` inserts `X` into the DOM.
//!     RED: drop the default-action block in `dispatch_key` and the key event fires but nothing inserts —
//!     the text stays "Hi".
//!   * `evs=bi:insertText:X|in:insertText:X` — the insert fires `beforeinput` then `input`, `inputType`
//!     `insertText`, `data` `X` — and a subsequent NON-printable key (`Enter`, whose `key` is not a
//!     single character) inserts nothing and fires no input event, so the log holds exactly ONE pair.
//!   * `blocktext=Keep` — a `contenteditable` whose `keydown` handler calls `preventDefault()` VETOES
//!     the default insertion: typing `Z` into it leaves its text "Keep" untouched (the editor-manages-
//!     its-own-model contract, via the cancelable keydown rather than a page-visible re-insert).

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

/// One page per test — a `PageContext` is per-process here (a second `Page::load` SIGSEGVs), so both the
/// normal editable and the veto editable live in ONE document.
#[test]
fn a_printable_key_typed_into_a_contenteditable_inserts_the_character() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<div id="ed" contenteditable="true">Hi</div>
<div id="block" contenteditable="true">Keep</div>
<div id="log"></div>
<script>
  var evs = [];
  var ed = document.getElementById('ed');
  ed.addEventListener('beforeinput', function (e) { evs.push('bi:' + e.inputType + ':' + e.data); });
  ed.addEventListener('input',       function (e) { evs.push('in:' + e.inputType + ':' + e.data); });
  // Caret at the end of "Hi".
  var t = ed.firstChild;
  window.getSelection().collapse(t, t.data.length);

  // An editor that manages its own model vetoes the default insertion at keydown.
  var block = document.getElementById('block');
  block.addEventListener('keydown', function (e) { e.preventDefault(); });

  window.__caretBlockEnd = function () {
    var bt = block.firstChild;
    window.getSelection().collapse(bt, bt.data.length);
  };
  window.__report = function () {
    document.getElementById('log').textContent =
      'edtext=' + ed.textContent + ' evs=' + evs.join('|') + ' blocktext=' + block.textContent;
  };
</script></body>"#,
        "https://type.test/",
        &fonts,
        W,
    );

    let ed = node(&p, "#ed");
    // Type 'X' — a single printable key: keydown inserts, keyup does not.
    p.dispatch_key(ed, "keydown", "X", &fonts, W);
    p.dispatch_key(ed, "keyup", "X", &fonts, W);
    // A non-printable key must NOT insert (its `key` is not a single character).
    p.dispatch_key(ed, "keydown", "Enter", &fonts, W);
    p.dispatch_key(ed, "keyup", "Enter", &fonts, W);

    // Into the veto editable: a prevented keydown inserts nothing.
    p.eval_for_test("window.__caretBlockEnd();");
    let block = node(&p, "#block");
    p.dispatch_key(block, "keydown", "Z", &fonts, W);
    p.dispatch_key(block, "keyup", "Z", &fonts, W);

    p.eval_for_test("window.__report();");
    let root = p.dom().root();
    let out = p
        .dom()
        .text_content(manuk_css::query_selector_all(p.dom(), root, "#log")[0]);
    println!("CONTENTEDITABLE-TYPING RESULT: {out}");

    for claim in [
        "edtext=HiX", // the typed char landed in the DOM; Enter added nothing
        "evs=bi:insertText:X|in:insertText:X", // beforeinput→input, one pair only (Enter fired none)
        "blocktext=Keep",                      // a prevented keydown vetoed the insert
    ] {
        assert!(
            out.contains(claim),
            "G_CONTENTEDITABLE_TYPING: expected `{claim}` in {out:?}\n  \
             a printable key typed into a contenteditable must insert the character at the caret and fire \
             beforeinput→input (inputType:insertText); a non-printable key inserts nothing; a keydown a \
             handler preventDefault()-ed inserts nothing."
        );
    }
}
