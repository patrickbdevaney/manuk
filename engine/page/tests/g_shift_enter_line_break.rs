//! **G_SHIFT_ENTER_LINE_BREAK — Shift+Enter inserts a hard line break (`<br>`) at the caret in a
//! contenteditable; plain Enter stays a no-op.**
//!
//! Now that the dispatched KeyboardEvent carries modifier state (t477), the UNAMBIGUOUS half of Enter
//! handling can run: `Shift+Enter` inserts a `<br>` at the caret (both Chrome and Firefox agree on this),
//! reusing the t475 `execCommand('insertLineBreak')` machinery and firing `beforeinput`→`input`
//! (`inputType:insertLineBreak`). PLAIN Enter stays a no-op here — `insertParagraph` (a block split) is
//! browser-divergent (Chrome `<div>`, Firefox `<br>`) and remains honestly unimplemented, so a page's own
//! Enter handler keeps ownership.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `brs=1` — a Shift+Enter with the caret between "A" and "B" inserts exactly one `<br>`. RED: drop the
//!     Shift+Enter branch → no `<br>`, `brs=0`.
//!   * `evs=bi:insertLineBreak|in:insertLineBreak` — the break fires beforeinput then input.
//!   * `text=AB` — a `<br>` is not a character, so textContent is unchanged.
//!   * `plainbrs=0` — a PLAIN Enter (no Shift) inserts nothing (insertParagraph stays honestly a no-op).

use manuk_page::{KeyModifiers, Page};
use manuk_text::FontContext;

const W: f32 = 800.0;
const SHIFT: KeyModifiers = KeyModifiers {
    ctrl: false,
    shift: true,
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
fn shift_enter_inserts_a_line_break_and_plain_enter_does_not() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<div id="ed" contenteditable="true">AB</div>
<div id="log"></div>
<script>
  var evs = [];
  var ed = document.getElementById('ed');
  ed.addEventListener('beforeinput', function (e) { evs.push('bi:' + e.inputType); });
  ed.addEventListener('input',       function (e) { evs.push('in:' + e.inputType); });
  // Place the caret between "A" and "B" (offset 1 of the text node).
  window.__caret1 = function () {
    var t = ed.firstChild;
    window.getSelection().collapse(t, 1);
  };
  window.__report = function () {
    document.getElementById('log').textContent =
      'brs=' + ed.getElementsByTagName('br').length + ' evs=' + evs.join('|') + ' text=' + ed.textContent;
  };
</script></body>"#,
        "https://shiftenter.test/",
        &fonts,
        W,
    );
    let ed = node(&p, "#ed");

    // Shift+Enter with the caret between A and B → one <br>.
    p.eval_for_test("window.__caret1();");
    p.dispatch_key_mods(ed, "keydown", "Enter", SHIFT, &fonts, W);
    p.dispatch_key_mods(ed, "keyup", "Enter", SHIFT, &fonts, W);
    p.eval_for_test("window.__report();");
    let out = p.dom().text_content(node(&p, "#log"));
    println!("SHIFT-ENTER RESULT: {out}");

    for claim in [
        "brs=1",
        "evs=bi:insertLineBreak|in:insertLineBreak",
        "text=AB",
    ] {
        assert!(
            out.contains(claim),
            "G_SHIFT_ENTER_LINE_BREAK: expected `{claim}` in {out:?}\n  \
             Shift+Enter must insert exactly one <br> at the caret and fire beforeinput→input \
             (inputType:insertLineBreak), leaving textContent unchanged."
        );
    }

    // A PLAIN Enter (no modifiers) must insert nothing — re-place the caret, press Enter, re-report:
    // the <br> count must still be 1 (only the Shift+Enter one) and NO new insertLineBreak event fired.
    p.eval_for_test("window.__caret1();");
    p.dispatch_key(ed, "keydown", "Enter", &fonts, W);
    p.dispatch_key(ed, "keyup", "Enter", &fonts, W);
    p.eval_for_test("window.__report();");
    let out2 = p.dom().text_content(node(&p, "#log"));
    println!("AFTER-PLAIN-ENTER RESULT: {out2}");
    assert!(
        out2.contains("brs=1"),
        "G_SHIFT_ENTER_LINE_BREAK: a PLAIN Enter must add NO <br> (insertParagraph stays a no-op) — the \
         editable should still hold exactly the one <br> from the Shift+Enter, got {out2:?}"
    );
    assert!(
        out2.matches("insertLineBreak").count() == 2,
        "G_SHIFT_ENTER_LINE_BREAK: a plain Enter must fire NO insertLineBreak — only the Shift+Enter pair \
         (bi+in) should be present, got {out2:?}"
    );
}
