//! **G_EXEC_INSERT_LINE_BREAK — `execCommand('insertLineBreak')` inserts a `<br>` at the caret.**
//!
//! The line-break member of the contenteditable EDITING family (alongside insertText t471, and the
//! Backspace/Delete pair t473/t474). A `<pre>`-style editor or an "insert line break" toolbar button
//! calls `execCommand('insertLineBreak')` to drop a hard newline; before this it returned `false` and
//! nothing happened. This inserts a `<br>` element at the caret (splitting the current text run if the
//! caret sits inside one) and fires `beforeinput`→`input` (`inputType:'insertLineBreak'`).
//!
//! ## Each claim, and how it goes RED
//!
//!   * `supported=true` — `queryCommandSupported('insertLineBreak')` is now `true`.
//!   * `ret=true`/`brs=1` — with a caret between "A" and "B", the command returns `true` and a single
//!     `<br>` now lives in the editable. RED: drop the `insertlinebreak` branch → returns `false`, `brs=0`.
//!   * `text=AB` — the `<br>` carries no text, so the editable's textContent is unchanged (the break is
//!     structural, between the two letters).
//!   * `evs=bi:insertLineBreak|in:insertLineBreak` — beforeinput then input, the right inputType.
//!   * `vbrs=0` — a `contenteditable` whose `beforeinput` handler `preventDefault()`s gets NO `<br>`.
//!   * `para=false` — `insertParagraph` (block splitting) is honestly still unbuilt and returns `false`.

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
fn exec_command_insert_line_break_inserts_a_br_at_the_caret() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<div id="ed" contenteditable="true">AB</div>
<div id="veto" contenteditable="true">Keep</div>
<div id="log"></div>
<script>
  var evs = [];
  var ed = document.getElementById('ed');
  ed.addEventListener('beforeinput', function (e) { evs.push('bi:' + e.inputType); });
  ed.addEventListener('input',       function (e) { evs.push('in:' + e.inputType); });
  // Caret between "A" and "B".
  window.getSelection().collapse(ed.firstChild, 1);

  var r = [];
  function k(n, v) { r.push(n + '=' + v); }
  try {
    k('supported', document.queryCommandSupported('insertLineBreak'));   // true
    k('ret', document.execCommand('insertLineBreak'));                   // true
    k('brs', ed.getElementsByTagName('br').length);                     // 1
    k('text', ed.textContent);                                          // "AB" (br has no text)
    k('evs', evs.join('|'));                                            // bi:insertLineBreak|in:insertLineBreak

    // Veto editable: a cancelled beforeinput inserts no <br>.
    var veto = document.getElementById('veto');
    veto.addEventListener('beforeinput', function (e) { e.preventDefault(); });
    window.getSelection().collapse(veto.firstChild, veto.firstChild.data.length);
    document.execCommand('insertLineBreak');
    k('vbrs', veto.getElementsByTagName('br').length);                  // 0

    k('para', document.execCommand('insertParagraph'));                 // false — honestly unbuilt
  } catch (e) { k('THREW', e); }
  document.getElementById('log').textContent = r.join(' ');
</script></body>"#,
        "https://br.test/",
        &fonts,
        W,
    );

    let out = p.dom().text_content(node(&p, "#log"));
    println!("EXEC-INSERT-LINE-BREAK RESULT: {out}");

    for claim in [
        "supported=true",
        "ret=true",
        "brs=1",   // exactly one <br> inserted at the caret
        "text=AB", // structural break — textContent unchanged
        "evs=bi:insertLineBreak|in:insertLineBreak",
        "vbrs=0",     // vetoed → no <br>
        "para=false", // insertParagraph still honestly false
    ] {
        assert!(
            out.contains(claim),
            "G_EXEC_INSERT_LINE_BREAK: expected `{claim}` in {out:?}\n  \
             execCommand('insertLineBreak') must insert a <br> at the caret in a contenteditable and fire \
             beforeinput→input (inputType:insertLineBreak); a vetoed beforeinput inserts none; \
             insertParagraph stays honestly false."
        );
    }
}
