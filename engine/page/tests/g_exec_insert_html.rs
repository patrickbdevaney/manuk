//! **G_EXEC_INSERT_HTML — `execCommand('insertHTML', false, html)` parses an HTML fragment and inserts it
//! at the caret, firing `beforeinput`→`input` (`inputType:'insertHTML'`), vetoable.**
//!
//! Brick 13 of the contenteditable EDITING subsystem. Unlike `insertText` (t471) this parses MARKUP — the
//! path an editor's "insert snippet / merge-tag / rich paste" button funnels through. The DOM result is
//! UNAMBIGUOUS (exactly the parsed fragment at the caret), so it needs no browser-divergent block heuristic.
//! Built on the already-won `Range.createContextualFragment` + `insertNode` substrate; zero new dep.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `html=a<b>X</b><i>Y</i>b` — with the caret between "a" and "b", the fragment `<b>X</b><i>Y</i>` is
//!     parsed and inserted THERE. RED: drop the `inserthtml` branch → `execCommand` returns false, no
//!     insertion, `html=ab`.
//!   * `evs=bi:insertHTML|in:insertHTML` — the insert fires beforeinput then input with the HTML inputType.
//!   * `supported=true` — `queryCommandSupported('insertHTML')` reports true.
//!   * `veto=vetoed` — an editable whose `beforeinput` handler `preventDefault()`s gets NO insertion.

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
fn exec_command_insert_html_parses_and_inserts_at_the_caret() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<div id="ed" contenteditable="true">ab</div>
<div id="veto" contenteditable="true">keep</div>
<div id="log"></div>
<script>
  var evs = [];
  var ed = document.getElementById('ed');
  ed.addEventListener('beforeinput', function (e) { evs.push('bi:' + e.inputType); });
  ed.addEventListener('input',       function (e) { evs.push('in:' + e.inputType); });
  document.getElementById('veto').addEventListener('beforeinput', function (e) { e.preventDefault(); });
  window.__caret = function () {
    // Caret between "a" and "b" (offset 1 of the text node).
    var t = ed.firstChild;
    window.getSelection().collapse(t, 1);
  };
  window.__selAll = function (id) {
    window.getSelection().selectAllChildren(document.getElementById(id));
  };
  window.__report = function () {
    document.getElementById('log').textContent =
      'html=' + ed.innerHTML +
      ' evs=' + evs.join('|') +
      ' supported=' + document.queryCommandSupported('insertHTML') +
      ' veto=' + (document.getElementById('veto').innerHTML === 'keep' ? 'vetoed' : document.getElementById('veto').innerHTML);
  };
</script></body>"#,
        "https://inserthtml.test/",
        &fonts,
        W,
    );

    p.eval_for_test(
        "window.__caret(); document.execCommand('insertHTML', false, '<b>X</b><i>Y</i>');",
    );
    p.eval_for_test(
        "window.__selAll('veto'); document.execCommand('insertHTML', false, '<u>Z</u>');",
    );
    p.eval_for_test("window.__report();");

    let out = p.dom().text_content(node(&p, "#log"));
    println!("EXEC-INSERT-HTML RESULT: {out}");

    for claim in [
        "html=a<b>X</b><i>Y</i>b",
        "evs=bi:insertHTML|in:insertHTML",
        "supported=true",
        "veto=vetoed",
    ] {
        assert!(
            out.contains(claim),
            "G_EXEC_INSERT_HTML: expected `{claim}` in {out:?}\n  \
             execCommand('insertHTML', false, html) must parse the fragment, insert it at the caret, fire \
             beforeinput/input (inputType:insertHTML), report queryCommandSupported true, and honour a veto."
        );
    }
}
