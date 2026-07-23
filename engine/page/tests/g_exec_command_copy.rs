//! **G_EXEC_COMMAND_COPY — `document.execCommand('copy')`, the legacy copy-button path.**
//!
//! `document.execCommand` was ABSENT, so the single most common copy-to-clipboard implementation on the
//! web — select a node's text, then `document.execCommand('copy')` (clipboard.js and every hand-rolled
//! "copy" button, usually as the fallback when the async Clipboard API is missing) —
//!
//! ```js
//! getSelection().selectAllChildren(codeBlock);
//! const ok = document.execCommand('copy');   // was: TypeError, handler dies
//! ```
//!
//! threw `TypeError: document.execCommand is not a function` and took the handler down. This wires the
//! commands that need NO editable DOM mutation — `copy` (copy the selection through the same host bridge
//! as `navigator.clipboard.writeText`) and `selectAll` — and honestly returns `false` for the FORMATTING
//! commands (bold/italic/…) and `cut`, which are the contenteditable editing subsystem.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `copy` — with a selection, `execCommand('copy')` returns `true` AND the selected text reaches the
//!     host clipboard queue (checked in Rust via `take_pending_clipboard_writes`). RED: remove the shim
//!     and the call throws — the whole handler dies and `#out` never updates.
//!   * `qs-copy`/`qs-sel` — `queryCommandSupported('copy'|'selectAll')` is `true`.
//!   * `qs-bold`/`bold`/`cut` — a FORMATTING command and `cut` are NOT supported and return `false`
//!     (honest scope: the editing subsystem is a separate brick — a page must feature-detect the truth).
//!   * `selAll` — `execCommand('selectAll')` selects the document, so the Selection is non-empty after.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<pre id="code">const x = 42;</pre>
<div id="out">-</div>
<script>
var r = [];
function k(n, v) { r.push(n + ':' + v); }
try {
  var sel = window.getSelection();
  sel.selectAllChildren(document.getElementById('code'));
  k('copy', document.execCommand('copy'));                       // true — copies "const x = 42;"
  k('qs-copy', document.queryCommandSupported('copy'));          // true
  k('qs-sel', document.queryCommandSupported('selectAll'));      // true
  k('qs-bold', document.queryCommandSupported('bold'));          // false — editing subsystem
  k('bold', document.execCommand('bold'));                       // false — not built, honestly
  k('cut', document.execCommand('cut'));                         // false — needs editable removal
  document.execCommand('selectAll');
  k('selAll', window.getSelection().toString().length > 0);      // selectAll left a non-empty selection
} catch (e) { k('THREW', e); }
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn document_exec_command_copy_is_the_legacy_copy_button_path() {
    let _ = manuk_js::take_clipboard_writes(); // drain prior state

    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://exec.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("EXEC-COMMAND RESULT: {got}");

    for claim in [
        "copy:true",     // execCommand('copy') succeeded — the whole point
        "qs-copy:true",  // queryCommandSupported('copy')
        "qs-sel:true",   // queryCommandSupported('selectAll')
        "qs-bold:false", // a formatting command is honestly unsupported
        "bold:false",    // …and returns false, not a lie
        "cut:false",     // cut needs editable removal (the editing subsystem) — false, honestly
        "selAll:true",   // execCommand('selectAll') left a non-empty selection
    ] {
        assert!(
            got.contains(claim),
            "G_EXEC_COMMAND_COPY: expected `{claim}` in {got:?}\n  \
             document.execCommand('copy'|'selectAll') must work (the legacy copy-button path), and \
             formatting/cut commands must honestly return false — a page feature-detects the truth."
        );
    }

    // The strongest tooth: the copied text actually reached the host clipboard queue.
    let writes = manuk_js::take_clipboard_writes();
    println!("EXEC-COMMAND CLIPBOARD WRITES: {writes:?}");
    assert!(
        writes.iter().any(|w| w == "const x = 42;"),
        "G_EXEC_COMMAND_COPY: execCommand('copy') must put the SELECTED TEXT on the host clipboard — \
         queued writes were {writes:?}, expected to contain \"const x = 42;\""
    );
}
