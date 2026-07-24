//! **G_KEY_PASTE — Ctrl/Cmd+V pastes the clipboard text at the caret in a contenteditable, firing a
//! cancelable `paste` event first.**
//!
//! The read half of keyboard clipboard support (cut/copy landed t478). With modifier state on the
//! KeyboardEvent (t477) and a synchronous clipboard read (t461), `Ctrl+V` now: reads the clipboard text,
//! fires a cancelable `paste` ClipboardEvent whose `clipboardData.getData('text/plain')` returns that text
//! (so an editor can intercept and veto), and — if not prevented — inserts it at the caret with
//! `inputType:'insertFromPaste'` (`beforeinput`→`input`). Before this, Ctrl+V in an editor did nothing.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `paste=PASTED` — the `paste` handler sees `clipboardData.getData('text/plain')` === the clipboard text.
//!   * `text=XPASTEDY` — with the caret between "X" and "Y", the clipboard text is inserted there. RED: drop
//!     the Ctrl+V branch → the editable stays "XY", no paste event, `text=XY`.
//!   * `evs=bi:insertFromPaste|in:insertFromPaste` — the insert fires beforeinput then input.
//!   * `veto=Keep` — an editable whose `paste` handler calls `preventDefault()` gets NO insertion (the editor
//!     owns the paste).

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
fn ctrl_v_pastes_clipboard_text_at_the_caret() {
    // Seed the host clipboard deterministically (what Ctrl+V reads).
    manuk_js::set_host_clipboard("PASTED".to_string());

    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<div id="ed" contenteditable="true">XY</div>
<div id="veto" contenteditable="true">Keep</div>
<div id="log"></div>
<script>
  var evs = [], seen = '';
  var ed = document.getElementById('ed');
  ed.addEventListener('paste', function (e) {
    seen = (e.clipboardData && e.clipboardData.getData) ? e.clipboardData.getData('text/plain') : '(no clipboardData)';
  });
  ed.addEventListener('beforeinput', function (e) { evs.push('bi:' + e.inputType); });
  ed.addEventListener('input',       function (e) { evs.push('in:' + e.inputType); });
  // The veto editable claims paste — preventDefault, so nothing is inserted.
  document.getElementById('veto').addEventListener('paste', function (e) { e.preventDefault(); });
  window.__caret1 = function () { var t = ed.firstChild; window.getSelection().collapse(t, 1); };
  window.__report = function () {
    document.getElementById('log').textContent =
      'paste=' + seen + ' text=' + ed.textContent + ' evs=' + evs.join('|');
  };
</script></body>"#,
        "https://paste.test/",
        &fonts,
        W,
    );
    let ed = node(&p, "#ed");

    // Ctrl+V with the caret between X and Y → paste event + "PASTED" inserted there.
    p.eval_for_test("window.__caret1();");
    p.dispatch_key_mods(ed, "keydown", "v", CTRL, &fonts, W);
    p.eval_for_test("window.__report();");
    let out = p.dom().text_content(node(&p, "#log"));
    println!("KEY-PASTE RESULT: {out}");

    // The veto editable: select all "Keep", Ctrl+V — its paste handler preventDefaults, so nothing changes.
    let veto = node(&p, "#veto");
    p.eval_for_test(
        "(function(){window.getSelection().selectAllChildren(document.getElementById('veto'));})()",
    );
    p.dispatch_key_mods(veto, "keydown", "v", CTRL, &fonts, W);
    let veto_text = p.dom().text_content(veto);
    println!("VETO: text={veto_text:?}");

    for claim in [
        "paste=PASTED",
        "text=XPASTEDY",
        "evs=bi:insertFromPaste|in:insertFromPaste",
    ] {
        assert!(
            out.contains(claim),
            "G_KEY_PASTE: expected `{claim}` in {out:?}\n  \
             Ctrl+V must fire a paste event exposing clipboardData.getData('text/plain') and insert the \
             clipboard text at the caret (inputType:insertFromPaste)."
        );
    }
    assert_eq!(
        veto_text, "Keep",
        "G_KEY_PASTE: an editable whose paste handler preventDefault()s must get NO insertion — \"Keep\" stays"
    );
}
