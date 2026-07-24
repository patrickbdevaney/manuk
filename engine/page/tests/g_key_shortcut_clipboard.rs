//! **G_KEY_SHORTCUT_CLIPBOARD — Ctrl/Cmd+X cuts and Ctrl/Cmd+C copies the selection, as the default
//! keyboard action.**
//!
//! With modifier state now on the dispatched KeyboardEvent (t477), the browser DEFAULT action for the two
//! clipboard chords can run: `Ctrl+X` cuts the selection, `Ctrl+C` copies it — routed straight through the
//! `execCommand('cut')`/`execCommand('copy')` paths (copy t463, cut t476), which already enforce
//! editable-only cut and the host clipboard write. Before this, a keyboard cut/copy did nothing (the chord
//! was suppressed from typing but never routed anywhere), so pressing Ctrl+X in any editor left the text in
//! place and the clipboard empty. A page that handles the chord itself (`preventDefault` on the keydown)
//! keeps ownership — the default does not run.
//!
//! ## Each claim, and how it goes RED
//!
//!   * cut — select "World" in a `<div contenteditable>Hello World</div>` and press `Ctrl+X`: the editable
//!     is left "Hello " and "World" reaches the host clipboard queue. RED: drop the chord→execCommand
//!     routing → the editable stays "Hello World" and the clipboard stays empty.
//!   * copy — with the editable now "Hello ", select "Hello" and press `Ctrl+C`: "Hello" reaches the
//!     clipboard and the DOM is UNCHANGED (copy removes nothing).
//!   * veto — a second editable whose keydown handler `preventDefault()`s `Ctrl+X` keeps its text (the page
//!     owns the chord; the default cut must not run).

use manuk_page::{KeyModifiers, Page};
use manuk_text::FontContext;

const W: f32 = 800.0;
const CTRL: KeyModifiers = KeyModifiers {
    ctrl: true,
    shift: false,
    alt: false,
    meta: false,
};

fn node(p: &Page, sel: &str) -> manuk_dom::NodeId {
    let root = p.dom().root();
    manuk_css::query_selector_all(p.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"))
}

#[test]
fn ctrl_x_cuts_and_ctrl_c_copies_the_selection() {
    let _ = manuk_js::take_clipboard_writes(); // drain prior state

    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<div id="ed" contenteditable="true">Hello World</div>
<div id="veto" contenteditable="true">Keep</div>
<script>
  // The veto editable claims Ctrl+X itself, so the browser default cut must not fire on it.
  document.getElementById('veto').addEventListener('keydown', function (e) {
    if ((e.ctrlKey || e.metaKey) && (e.key === 'x' || e.key === 'X')) { e.preventDefault(); }
  });
</script></body>"#,
        "https://kbclip.test/",
        &fonts,
        W,
    );
    let ed = node(&p, "#ed");

    // 1. Select "World" (offsets 6..11) and press Ctrl+X — cut.
    p.eval_for_test("(function(){var t=document.getElementById('ed').firstChild;window.getSelection().setBaseAndExtent(t,6,t,11);})()");
    p.dispatch_key_mods(ed, "keydown", "x", CTRL, &fonts, W);
    let after_cut = p.dom().text_content(ed);
    let cut_writes = manuk_js::take_clipboard_writes();
    println!("CUT: text={after_cut:?} clip={cut_writes:?}");

    // 2. Select "Hello" (offsets 0..5 of the now-"Hello " text) and press Ctrl+C — copy (no DOM change).
    p.eval_for_test("(function(){var t=document.getElementById('ed').firstChild;window.getSelection().setBaseAndExtent(t,0,t,5);})()");
    p.dispatch_key_mods(ed, "keydown", "c", CTRL, &fonts, W);
    let after_copy = p.dom().text_content(ed);
    let copy_writes = manuk_js::take_clipboard_writes();
    println!("COPY: text={after_copy:?} clip={copy_writes:?}");

    // 3. The veto editable claims Ctrl+X: select all of "Keep", press Ctrl+X — text unchanged.
    let veto = node(&p, "#veto");
    p.eval_for_test(
        "(function(){window.getSelection().selectAllChildren(document.getElementById('veto'));})()",
    );
    p.dispatch_key_mods(veto, "keydown", "x", CTRL, &fonts, W);
    let after_veto = p.dom().text_content(veto);
    println!("VETO: text={after_veto:?}");

    assert_eq!(
        after_cut, "Hello ",
        "G_KEY_SHORTCUT_CLIPBOARD: Ctrl+X must CUT the selection — the editable should be left \"Hello \""
    );
    assert!(
        cut_writes.iter().any(|w| w == "World"),
        "G_KEY_SHORTCUT_CLIPBOARD: Ctrl+X must put the cut text on the host clipboard — writes were \
         {cut_writes:?}, expected \"World\""
    );
    assert_eq!(
        after_copy, "Hello ",
        "G_KEY_SHORTCUT_CLIPBOARD: Ctrl+C must COPY (remove nothing) — the editable must stay \"Hello \""
    );
    assert!(
        copy_writes.iter().any(|w| w == "Hello"),
        "G_KEY_SHORTCUT_CLIPBOARD: Ctrl+C must put the copied text on the host clipboard — writes were \
         {copy_writes:?}, expected \"Hello\""
    );
    assert_eq!(
        after_veto, "Keep",
        "G_KEY_SHORTCUT_CLIPBOARD: a keydown handler that preventDefault()s Ctrl+X owns the chord — the \
         default cut must not run, so \"Keep\" stays"
    );
}
