//! **G_EXEC_FORMAT_BOLD — `execCommand('bold'|'italic')` wraps the selection in `<b>`/`<i>`, firing
//! `beforeinput`→`input` (`inputType:'formatBold'`/`'formatItalic'`), vetoable.**
//!
//! The write half of a rich-text toolbar's Bold/Italic button (Gmail compose, Slack, comment editors).
//! Built on the already-won Selection/Range substrate: it extracts the selected range, wraps it in an
//! inline formatting element, re-inserts it, and re-selects the formatted run — the same veto contract as
//! the insert/delete helpers. Before this, every command past insertText/insertLineBreak/cut/copy returned
//! `false` and `queryCommandSupported('bold')` was false, so a page feature-detected the truth.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `html=hello <b>world</b>` — selecting "world" and running `bold` wraps just that run in `<b>`; the
//!     text content is unchanged ("hello world"). RED: drop the `bold` branch → `execCommand` returns false,
//!     no `<b>`, `html=hello world`.
//!   * `evs=bi:formatBold|in:formatBold` — the wrap fires beforeinput then input with the format inputType.
//!   * `supported=true` — `queryCommandSupported('bold')` and `('italic')` now report true.
//!   * `veto=vetoed` — an editable whose `beforeinput` handler `preventDefault()`s gets NO wrap (the editor
//!     owns the formatting); the italic editable confirms `<i>` on the same path.

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
fn exec_command_bold_and_italic_wrap_the_selection() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<div id="ed" contenteditable="true">hello world</div>
<div id="it" contenteditable="true">make me italic</div>
<div id="veto" contenteditable="true">leave me</div>
<div id="log"></div>
<script>
  var evs = [];
  var ed = document.getElementById('ed');
  ed.addEventListener('beforeinput', function (e) { evs.push('bi:' + e.inputType); });
  ed.addEventListener('input',       function (e) { evs.push('in:' + e.inputType); });
  // The veto editable claims the format — preventDefault, so nothing is wrapped.
  document.getElementById('veto').addEventListener('beforeinput', function (e) { e.preventDefault(); });
  // Select "world" (offset 6..11 of the single text node) inside #ed.
  window.__selWorld = function () {
    var t = ed.firstChild;
    window.getSelection().setBaseAndExtent(t, 6, t, 11);
  };
  window.__selAll = function (id) {
    window.getSelection().selectAllChildren(document.getElementById(id));
  };
  window.__report = function () {
    document.getElementById('log').textContent =
      'html=' + ed.innerHTML +
      ' it=' + document.getElementById('it').innerHTML +
      ' evs=' + evs.join('|') +
      ' supported=' + (document.queryCommandSupported('bold') && document.queryCommandSupported('italic')) +
      ' veto=' + (document.getElementById('veto').innerHTML === 'leave me' ? 'vetoed' : document.getElementById('veto').innerHTML);
  };
</script></body>"#,
        "https://format.test/",
        &fonts,
        W,
    );

    // Bold: select "world" → wrap it in <b>.
    p.eval_for_test("window.__selWorld(); document.execCommand('bold');");
    // Italic: select all of #it → wrap it in <i>.
    p.eval_for_test("window.__selAll('it'); document.execCommand('italic');");
    // Veto: select all of #veto → its beforeinput preventDefaults, so no wrap.
    p.eval_for_test("window.__selAll('veto'); document.execCommand('bold');");
    p.eval_for_test("window.__report();");

    let out = p.dom().text_content(node(&p, "#log"));
    println!("EXEC-FORMAT RESULT: {out}");

    for claim in [
        "html=hello <b>world</b>",
        "it=<i>make me italic</i>",
        "evs=bi:formatBold|in:formatBold",
        "supported=true",
        "veto=vetoed",
    ] {
        assert!(
            out.contains(claim),
            "G_EXEC_FORMAT_BOLD: expected `{claim}` in {out:?}\n  \
             execCommand('bold'|'italic') must wrap the selection in <b>/<i>, fire beforeinput/input \
             (inputType:formatBold/formatItalic), report queryCommandSupported true, and honour a \
             beforeinput veto."
        );
    }
}
