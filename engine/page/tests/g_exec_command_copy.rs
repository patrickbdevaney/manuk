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
//! as `navigator.clipboard.writeText`) and `selectAll`.
//!
//! ## ⚠ THIS GATE WAS THE LIE, FOR NINETY-FOUR TICKS (corrected at tick 576)
//!
//! Written at tick 463, it asserted `queryCommandSupported('bold') === false` — the honest "no" of an
//! engine whose editing subsystem did not exist yet. **`execCommand('bold')` landed at tick 481**
//! (`__wrapSelectionFormat`, EDITING brick 11), and from that moment the engine answered `true` and this
//! assertion was simply wrong. Nothing said so, because `g_exec_command_copy` is one of ~260 test
//! binaries the verify wall does not launch.
//!
//! That is the standing rule read in its less obvious direction. *"A 'no' stub becomes a lie when the
//! cap lands"* is usually about the **engine**; here the engine told the truth and the **assertion**
//! rotted. Both are the same failure — a claim about capability that nobody re-measured after the
//! capability moved — and the correction is the same: **the gate follows the capability, never the
//! reverse.**
//!
//! So the rot is not merely patched to green: the claim is replaced by the stronger one it should
//! always have been. `queryCommandSupported('bold')` is `true` **and** `execCommand('bold')` over a real
//! selection in a contenteditable actually produces a `<b>`. Asserting only the former would leave the
//! same hole one layer up — a support claim nothing checks against behaviour.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `copy` — with a selection, `execCommand('copy')` returns `true` AND the selected text reaches the
//!     host clipboard queue (checked in Rust via `take_pending_clipboard_writes`). RED: remove the shim
//!     and the call throws — the whole handler dies and `#out` never updates.
//!   * `qs-copy`/`qs-sel` — `queryCommandSupported('copy'|'selectAll')` is `true`.
//!   * `qs-bold`/`boldWraps` — bold is supported AND it works: over a non-collapsed selection inside a
//!     contenteditable, `execCommand('bold')` wraps the run in a `<b>`. RED: revert `__EXEC_SUPPORTED`
//!     to drop `bold`, or make `__wrapSelectionFormat` a no-op.
//!   * `boldNoSel` — with NOTHING selected it returns `false`, which is the honest edge this brick
//!     really does have: a collapsed caret would arm a "typing style" for the next keystroke, and that
//!     stateful toggle is not built. Supported ≠ succeeds unconditionally.
//!   * `cutNoEdit`/`cutEdit`/`cutGone` — the SECOND stale claim in this file, and it is the t573
//!     fixture lesson rather than the rot above: `cut:false` was *passing*, because the only selection
//!     this fixture ever made was inside a `<pre>` and `cut` correctly declines outside an editing
//!     host. `execCommand('cut')` has been fully implemented for a long time. **An assertion whose
//!     fixture cannot reach the mechanism is green for a reason unrelated to the claim** — so both
//!     halves are now asserted: it declines outside an editable, it succeeds inside one, and the text
//!     is really gone.
//!   * `selAll` — `execCommand('selectAll')` selects the document, so the Selection is non-empty after.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<pre id="code">const x = 42;</pre>
<div id="edit" contenteditable="true">formatme</div>
<div id="cutme" contenteditable="true">cutme</div>
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
  k('qs-bold', document.queryCommandSupported('bold'));          // true — bold LANDED at tick 481
  // With nothing selected bold declines — the honest edge this brick really has (a collapsed caret
  // would arm a typing style for the next keystroke, and that stateful toggle is not built).
  sel.removeAllRanges();
  k('boldNoSel', document.execCommand('bold'));                  // false — no selection to wrap
  // …and over a real selection in a contenteditable it WORKS. This is the claim that makes the
  // support answer above mean something.
  var host = document.getElementById('edit');
  sel.selectAllChildren(host);
  document.execCommand('bold');
  k('boldWraps', host.querySelectorAll('b').length === 1);       // true — the run is wrapped
  // `cut` was asserted false here for 113 ticks and passed for the WRONG REASON: the only selection
  // this fixture ever made was inside a <pre>, and cut declines outside an editing host. That edge is
  // real, so it is asserted on purpose — and so is the capability the old fixture could not reach.
  sel.selectAllChildren(document.getElementById('code'));
  k('cutNoEdit', document.execCommand('cut'));                   // false — <pre> is not editable
  var cuthost = document.getElementById('cutme');
  sel.selectAllChildren(cuthost);
  k('cutEdit', document.execCommand('cut'));                     // true — inside a contenteditable
  k('cutGone', cuthost.textContent.length === 0);                // …and the text is actually removed
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
        "copy:true",       // execCommand('copy') succeeded — the whole point
        "qs-copy:true",    // queryCommandSupported('copy')
        "qs-sel:true",     // queryCommandSupported('selectAll')
        "qs-bold:true",    // bold LANDED at tick 481 — this line said `false` for 94 ticks
        "boldNoSel:false", // …and still declines with nothing selected: supported != unconditional
        "boldWraps:true",  // …and over a real selection it produces the <b>. The claim with teeth.
        "cutNoEdit:false", // cut outside an editing host declines — the edge the old fixture hit
        "cutEdit:true",    // …but inside one it works. `cut:false` passed for 113 ticks on a <pre>.
        "cutGone:true",    // …and the cut text is really removed, not merely reported removed
        "selAll:true",     // execCommand('selectAll') left a non-empty selection
    ] {
        assert!(
            got.contains(claim),
            "G_EXEC_COMMAND_COPY: expected `{claim}` in {got:?}\n  \
             document.execCommand('copy'|'selectAll') must work (the legacy copy-button path); bold \
             must report supported AND actually wrap a real selection; cut must still honestly \
             return false — a page feature-detects the truth, in both directions."
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
