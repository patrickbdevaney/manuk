//! **G_EXEC_INSERT_TEXT — `document.execCommand('insertText', …)`, the first contenteditable EDITING brick.**
//!
//! The editability QUERY surface landed earlier (`el.isContentEditable`, `:read-write`), so a rich
//! editor now *mounts* on a `<div contenteditable>` — but the moment it tried to put a character into
//! the host it hit a wall: `insertText` (and every other command that mutates editable content) honestly
//! returned `false`, so the single most fundamental editing primitive — "insert this text at the caret"
//! — did nothing. An "insert emoji/snippet" toolbar button, a paste-as-plaintext handler, and the
//! default typed-character action ALL funnel through `insertText`; without it a contenteditable is a
//! read-only box that merely *looks* editable.
//!
//! This wires the real primitive: at the caret inside the editing host, fire `beforeinput`
//! (`inputType:'insertText'`, cancelable) → mutate the DOM → fire `input` (`inputType:'insertText'`,
//! not cancelable). Every rich editor (ProseMirror, Slate, Lexical, Draft) keys its model and its undo
//! stack on exactly that `beforeinput`/`input` pair and its `inputType`.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `supported` — `queryCommandSupported('insertText')` is now `true` (a page feature-detects it).
//!   * `ret`/`text` — with a caret after "Hello", `execCommand('insertText', false, ' World')` returns
//!     `true` AND the host's text is now "Hello World" (the DOM actually mutated, merged into the one
//!     text run). RED: drop the `inserttext` branch and the call returns `false`, the text stays "Hello".
//!   * `log` — the events fire in order with the right payload: `beforeinput` (insertText, " World")
//!     STRICTLY before `input` (insertText, " World").
//!   * `vret`/`vtext`/`vlog` — a `beforeinput` handler that calls `preventDefault()` VETOES the insert:
//!     the command still ran (`vret:true`), the DOM is untouched ("Keep"), and `input` never fired
//!     (`vlog:bi` only) — the spec's cancelable-beforeinput contract an editor relies on to run its own
//!     model instead of the browser's default insertion.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="ed" contenteditable="true">Hello</div>
<div id="veto" contenteditable="true">Keep</div>
<div id="out">-</div>
<script>
var r = [];
function k(n, v) { r.push(n + ':' + v); }
try {
  var ed = document.getElementById('ed');
  var log = [];
  ed.addEventListener('beforeinput', function (e) { log.push('bi:' + e.inputType + ':' + e.data); });
  ed.addEventListener('input',       function (e) { log.push('in:' + e.inputType + ':' + e.data); });

  // Place a collapsed caret at the end of "Hello" (offset 5 in the text node).
  var t = ed.firstChild;
  var sel = window.getSelection();
  sel.collapse(t, t.data.length);

  k('supported', document.queryCommandSupported('insertText'));   // true
  k('ret', document.execCommand('insertText', false, ' World'));  // true
  k('text', ed.textContent);                                      // "Hello World"
  k('log', log.join('|'));   // bi:insertText: World|in:insertText: World — beforeinput strictly before input

  // Veto: a beforeinput handler cancels → no mutation, no input event.
  var veto = document.getElementById('veto');
  var vlog = [];
  veto.addEventListener('beforeinput', function (e) { vlog.push('bi'); e.preventDefault(); });
  veto.addEventListener('input',       function (e) { vlog.push('in'); });
  var vt = veto.firstChild;
  sel.collapse(vt, vt.data.length);
  k('vret', document.execCommand('insertText', false, 'X'));      // true — the command ran
  k('vtext', veto.textContent);                                   // "Keep" — unchanged, insert vetoed
  k('vlog', vlog.join('|'));                                      // "bi" only — input never fired
} catch (e) { k('THREW', e); }
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn exec_command_insert_text_is_the_first_contenteditable_editing_brick() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://edit.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("EXEC-INSERT-TEXT RESULT: {got}");

    for claim in [
        "supported:true",   // queryCommandSupported('insertText')
        "ret:true",         // execCommand returned true
        "text:Hello World", // the DOM actually mutated at the caret
        "log:bi:insertText: World|in:insertText: World", // beforeinput STRICTLY before input, right payload
        "vret:true",                                     // a vetoed command still "ran"
        "vtext:Keep",                                    // …but the DOM is untouched
        "vlog:bi",                                       // …and input never fired
    ] {
        assert!(
            got.contains(claim),
            "G_EXEC_INSERT_TEXT: expected `{claim}` in {got:?}\n  \
             execCommand('insertText') must insert text at the caret inside a contenteditable, firing \
             beforeinput→input (inputType:insertText), and a cancelled beforeinput must veto the insert \
             (no mutation, no input) — the primitive every rich editor's model is built on."
        );
    }
}
