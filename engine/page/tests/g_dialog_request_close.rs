//! **G_DIALOG_REQUEST_CLOSE — `dialog.requestClose()` fires a cancelable `cancel`, then closes if unvetoed.**
//!
//! `requestClose([returnValue])` (Baseline 2025) is the "ask to close" sibling of `close()`. It is what a
//! dialog's Close button and ✕ should call: unlike `close()` (which shuts the dialog unconditionally), it
//! fires a **cancelable `cancel` event first**, so a "you have unsaved changes — discard?" guard can
//! `preventDefault()` and keep the dialog open. It is the same veto hook Escape already runs through, now
//! reachable from script.
//!
//! It was absent: `close()` and `showModal()` existed but `requestClose` was `undefined`, so a page (or a
//! component library's close button) calling `dlg.requestClose()` hit a synchronous TypeError that took
//! the click handler down with it — the button did nothing.
//!
//! Four things to prove:
//! 1. on an open dialog with no veto, `requestClose()` closes it (the `open` attribute is gone and the
//!    `close` event fired) and passes its return value through;
//! 2. it fires a `cancel` event that is **cancelable**;
//! 3. a `cancel` handler that calls `preventDefault()` **vetoes** the close — the dialog stays open;
//! 4. on an already-closed dialog it is a no-op (no `cancel`, no throw).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<dialog id="d1">one</dialog>
<dialog id="d2">two</dialog>
<dialog id="d3">three</dialog>
<div id="out">-</div>
<script>
  var R = [];

  // (1) open + requestClose with a returnValue, no veto -> closes, fires close, returnValue set.
  var d1 = document.getElementById('d1');
  d1.setAttribute('open','');
  var d1closed = false, d1cancelable = null;
  d1.addEventListener('close', function(){ d1closed = true; });
  d1.addEventListener('cancel', function(e){ d1cancelable = e.cancelable; });
  d1.requestClose('saved');
  R.push('d1open:' + d1.hasAttribute('open'));      // false — it closed
  R.push('d1close:' + d1closed);                    // true — close event fired
  R.push('d1cancelable:' + d1cancelable);           // true — cancel was cancelable
  R.push('d1rv:' + d1.returnValue);                 // saved

  // (3) a cancel handler that preventDefaults VETOES the close.
  var d2 = document.getElementById('d2');
  d2.setAttribute('open','');
  d2.addEventListener('cancel', function(e){ e.preventDefault(); });
  d2.requestClose();
  R.push('d2open:' + d2.hasAttribute('open'));      // true — vetoed, still open

  // (4) requestClose on a CLOSED dialog is a no-op — no cancel, no throw.
  var d3 = document.getElementById('d3');
  var d3cancel = false;
  d3.addEventListener('cancel', function(){ d3cancel = true; });
  var threw = false;
  try { d3.requestClose(); } catch (e) { threw = true; }
  R.push('d3cancel:' + d3cancel);                   // false — closed dialog, nothing fired
  R.push('d3threw:' + threw);                        // false

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn a_request_close_fires_cancelable_cancel_then_closes_unless_vetoed() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://dlg.test/", &fonts, 400.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("d1open:false", "requestClose() with no veto must close the dialog — the `open` attribute is removed"),
        ("d1close:true", "…and fire the `close` event, the signal libraries wait on to read returnValue"),
        (
            "d1cancelable:true",
            "the `cancel` it fires must be CANCELABLE — that is the whole point, it is the veto hook",
        ),
        ("d1rv:saved", "the returnValue argument is passed through to the closed dialog"),
        (
            "d2open:true",
            "a `cancel` handler calling preventDefault() must VETO the close — the dialog stays open. \
             This is `requestClose`'s reason to exist over `close`",
        ),
        (
            "d3cancel:false",
            "requestClose() on an already-closed dialog is a no-op — it must not fire `cancel`",
        ),
        ("d3threw:false", "…and must not throw"),
    ] {
        assert!(
            got.contains(claim),
            "G_DIALOG_REQUEST_CLOSE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
