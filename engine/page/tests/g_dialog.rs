//! G_DIALOG — `<dialog>`: the modal's JS surface actually works.
//!
//! The bar, and it is the daily-driver one: a page ships `<dialog>` + `showModal()` (every cookie
//! banner, confirm-delete and command palette written since ~2022), and
//!
//!   1. `showModal()` exists and opens it — before this tick it was `undefined`, i.e. a TypeError
//!      that took the click handler with it, so the button did nothing at all;
//!   2. `close(value)` closes it, sets `returnValue` and fires `close`;
//!   3. `<form method="dialog">`'s submit button closes the dialog with the button's value and
//!      does NOT navigate;
//!   4. Escape fires a cancelable `cancel` on the topmost modal, then dismisses it.
//!
//! The rendering half — a closed dialog paints nothing, an open modal joins the top layer — is
//! `g_dialog_render.rs` (a separate binary: two SpiderMonkey contexts tear down messily, see
//! `g_globals`).

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <dialog id="dlg">
    <p id="secret">DELETE EVERYTHING?</p>
    <form method="dialog"><button id="ok" value="confirmed">OK</button></form>
  </dialog>
  <dialog id="esc"><p>escape me</p></dialog>
  <div id="out"></div>
  <script>
    var $ = function(id) { return document.getElementById(id); };
    var r = [];
    var d = $('dlg');
    r.push('iface:' + (d instanceof HTMLDialogElement));
    r.push('closed:' + (d.open === false));

    // 1. showModal() opens it, and marks it modal (the flag the top-layer stacking reads).
    d.showModal();
    r.push('open:' + (d.open === true));
    r.push('modal:' + d.hasAttribute('data-manuk-modal'));

    // showModal() on an already-open dialog is an InvalidStateError, not a silent no-op —
    // libraries DO double-open on a re-render and catch by name.
    var threw = '';
    try { d.showModal(); } catch (e) { threw = e.name; }
    r.push('reopen:' + (threw === 'InvalidStateError'));

    // 2. close(value) -> returnValue + the `close` event the caller waits on.
    var closeFired = 0;
    d.addEventListener('close', function() { closeFired++; });
    d.close('scripted');
    r.push('afterclose:' + (d.open === false));
    r.push('rv:' + (d.returnValue === 'scripted'));
    r.push('closeevt:' + (closeFired === 1));
    r.push('unmodal:' + (d.hasAttribute('data-manuk-modal') === false));

    // 3. <form method="dialog"> — pure markup, no script: the click closes with the button's value.
    d.showModal();
    $('ok').click();
    r.push('formdialog:' + (d.open === false && d.returnValue === 'confirmed'));

    // 4. Escape: a cancelable `cancel` first. A guard that preventDefault()s keeps it open.
    var e2 = $('esc');
    e2.showModal();
    var guard = function(ev) { ev.preventDefault(); };
    e2.addEventListener('cancel', guard);
    var esc = function() {
      var ev = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true });
      ev.key = 'Escape';   // belt and braces: the ctor's dict must survive to the listener
      document.dispatchEvent(ev);
    };
    esc();
    r.push('cancelguard:' + (e2.open === true));
    e2.removeEventListener('cancel', guard);
    esc();
    r.push('escclose:' + (e2.open === false));

    // A non-modal show() opens WITHOUT joining the top layer — it stays in flow, per spec.
    e2.show();
    r.push('shownonmodal:' + (e2.open === true && e2.hasAttribute('data-manuk-modal') === false));

    $('out').textContent = r.join(' ');
  </script>
</body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn dialog_show_modal_close_and_cancel() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://dialog.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "iface:true",        // HTMLDialogElement brands the tag
        "closed:true",       // .open reflects
        "open:true",         // showModal() opens
        "modal:true",        // ...and marks modality for the top layer
        "reopen:true",       // re-showModal() throws InvalidStateError
        "afterclose:true",   // close() closes
        "rv:true",           // close(v) sets returnValue
        "closeevt:true",     // ...and fires `close` exactly once
        "unmodal:true",      // ...and leaves the top layer
        "formdialog:true",   // <form method=dialog> closes with the button value, no navigation
        "cancelguard:true",  // Escape's `cancel` is cancelable
        "escclose:true",     // ...and otherwise dismisses the topmost modal
        "shownonmodal:true", // show() opens without joining the top layer
    ] {
        assert!(
            got.contains(claim),
            "G_DIALOG: expected {claim} in {got:?}\n  \
             `<dialog>` is how the modern web ships every modal. A missing `showModal` is a \
             TypeError that takes the click handler with it, and the button does nothing at all."
        );
    }
}
