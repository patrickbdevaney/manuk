//! **G_SELECT_EVENT — the `select` event: queued, not coalesced, and owned by the API rather than by
//! the selection value.**
//!
//! t1392 built the selection STATE — applicability, the resting caret, the collapse rules. This is
//! the other half: the notification. Without it, `select-event.html` was **0 of 270** and four more
//! files sat in `TH_TIMEOUT`, because a promise waiting on an event that never arrives does not fail,
//! it hangs.
//!
//! Every row headless-Chrome-measured, and **each case in its OWN page load** — a single page running
//! all the cases in sequence returned wrong answers for everything after the first, which is the
//! t780 lesson (*the instrument was the bug*) arriving before the engine work rather than after it.
//!
//! ```text
//!   setSelectionRange / select() / selectionStart= / selectionEnd= /
//!   selectionDirection= / setRangeText           -> fires · bubbles · NOT cancelable · isTrusted
//!   the count read SYNCHRONOUSLY after the call             -> 0    it is a QUEUED task
//!   the same call twice (the second changes nothing)        -> 1    a change DETECTOR
//!   two DIFFERENT changes in one task                       -> 2    NOT coalesced
//!   on a DISCONNECTED element                               -> 1
//!   `el.value = "..."` (moves the caret, silently)          -> 0
//!   setRangeText that rewrites the VALUE but not the range  -> 0
//! ```
//!
//! ⭐⭐⭐ **The two silent rows are the ones that place the trigger.** `el.value =` demonstrably moves
//! the caret and fires nothing; `setRangeText(..., 'preserve')` demonstrably rewrites the text and
//! fires nothing. So the event is not *"the selection differs"* — it is *"the page used the selection
//! API"*. The obvious implementation, firing from the shared `store_selection` clamp, gets the first
//! of those wrong, because the value setter calls it too.
//!
//! ⚠ **It is NOT coalesced, and WPT reads as though it were.** Its *"must fire select only once"*
//! case calls the SAME action twice — so the second is a no-change, and the test is checking the
//! change detector. Building a queue-suppression flag from that reading would swallow the second of
//! two genuinely different changes, which is the `twice` row above.
//!
//! ⚠ ONE `#[test]` in this binary: two SpiderMonkey contexts in one manuk-page binary tear each other
//! down and the second test reads the first one's empty output.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<input id="i" value="abcdef">
<input id="pristine" value="abcdef">
<textarea id="t">abcdef</textarea>
<div id="out">-</div>
<script>
var r = [];
function k(n, v) { r.push(n + ':' + JSON.stringify(v)); }
function mk(v) {
  var e = document.createElement('input');
  e.value = (v === undefined) ? 'abcdef' : v;
  document.body.appendChild(e);
  return e;
}
function counter(el) { var c = { n: 0, ev: null };
  el.addEventListener('select', function (e) { c.n++; c.ev = e; }); return c; }

var i = document.getElementById('i'), t = document.getElementById('t');

// ── 1. IT IS A QUEUED TASK, NOT A SYNCHRONOUS CALL ────────────────────────────────────
var c1 = counter(i);
i.setSelectionRange(1, 3);
k('a_syncCount', c1.n);                       // 0 — nothing has run yet

// ── 2. every entry point fires ────────────────────────────────────────────────────────
var cSel = counter(mk());       cSel.n = 0;   var eSel = mk();
var c2 = counter(eSel);         eSel.select();
var e3 = mk(); var c3 = counter(e3);          e3.selectionStart = 1;
var e4 = mk(); var c4 = counter(e4);          e4.selectionEnd = 2;
var e5 = mk(); var c5 = counter(e5);          e5.setSelectionRange(1, 3); e5.selectionDirection = 'backward';
var e6 = mk(); var c6 = counter(e6);          e6.setRangeText('QQ', 1, 3, 'select');
var c7 = counter(t);                          t.setSelectionRange(1, 3);

// ── 3. the change DETECTOR, and the absence of coalescing ─────────────────────────────
var e8 = mk(); var c8 = counter(e8);   e8.setSelectionRange(1, 3); e8.setSelectionRange(1, 3);
var e9 = mk(); var c9 = counter(e9);   e9.setSelectionRange(1, 2); e9.setSelectionRange(3, 4);
// ⚠ This row needs a PRISTINE control, and `mk()` cannot supply one: `mk()` assigns `.value`, and
//    that moves the caret to the END (t1392), so `setSelectionRange(0,0)` on it IS a change. Only an
//    element whose value arrived from the MARKUP still has its caret at the resting 0.
var e10 = document.getElementById('pristine'); var c10 = counter(e10);
e10.setSelectionRange(0, 0);

// ── 4. the two SILENT writes — this is what places the trigger ────────────────────────
var e11 = mk(); var c11 = counter(e11); e11.value = 'zzzzzzzz';
// A CONTROL and its arm, because "fires nothing" is only readable as a DIFFERENCE: both do the same
// setSelectionRange, and only the second also rewrites the value.
var e12c = mk(); var c12c = counter(e12c); e12c.setSelectionRange(1, 3);
var e12 = mk(); var c12 = counter(e12); e12.setSelectionRange(1, 3);
e12.setRangeText('QQ', 1, 3, 'preserve');


// ── 5. a DISCONNECTED element still fires ─────────────────────────────────────────────
var e13 = document.createElement('input'); e13.value = 'abcdef';
var c13 = counter(e13); e13.setSelectionRange(0, 3);

// ── 6. it BUBBLES to a document listener ──────────────────────────────────────────────
var docHits = 0;
document.addEventListener('select', function () { docHits++; });
var e14 = mk(); e14.setSelectionRange(2, 4);

setTimeout(function () {
  k('b_afterTask', c1.n);
  k('c_bubbles', c1.ev && c1.ev.bubbles);
  k('d_cancelable', c1.ev && c1.ev.cancelable);
  k('e_isTrusted', c1.ev && c1.ev.isTrusted);
  k('f_type', c1.ev && c1.ev.type);
  k('g_targetIsInput', c1.ev && c1.ev.target === i);
  k('h_select', c2.n);
  k('i_selectionStart', c3.n);
  k('j_selectionEnd', c4.n);
  k('k_selectionDirection', c5.n);          // 2: the range, then the direction
  k('l_setRangeText', c6.n);
  k('m_textarea', c7.n);
  k('n_sameTwice', c8.n);                   // 1 — the second is a no-change
  k('o_twoDifferent', c9.n);                // 2 — NOT coalesced
  k('p_noChangeAtAll', c10.n);              // 0
  k('q_valueWriteSilent', c11.n);           // 0 — moves the caret, fires nothing
  k('r_ctrlSsrOnly', c12c.n);               // 1 — the control arm
  k('r_rangeTextAddsNothing', c12.n);       // 1 — SAME as the control: the rewrite fired nothing
  k('r_valueDidChange', e12.value);         // ...and it really did rewrite the value
  k('s_disconnected', c13.n);
  k('t_bubblesToDocument', docHits > 0);
  document.getElementById('out').textContent = r.join(' ');
}, 0);
</script></body></html>"##;

/// One test in the binary — see the module note.
#[test]
fn the_select_event_is_queued_uncoalesced_and_owned_by_the_selection_api() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://select-event.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SELECT-EVENT RESULT: {got}");

    for claim in [
        // 1 — queued, never synchronous
        "a_syncCount:0",
        "b_afterTask:1",
        // event properties
        "c_bubbles:true",
        "d_cancelable:false",
        "e_isTrusted:true",
        "f_type:\"select\"",
        "g_targetIsInput:true",
        // 2 — every entry point fires
        "h_select:1",
        "i_selectionStart:1",
        "j_selectionEnd:1",
        "k_selectionDirection:2",
        "l_setRangeText:1",
        "m_textarea:1",
        // 3 — a change detector, and no coalescing
        "n_sameTwice:1",
        "o_twoDifferent:2",
        "p_noChangeAtAll:0",
        // 4 — the two silent writes that place the trigger on the API, not on the value
        "q_valueWriteSilent:0",
        "r_ctrlSsrOnly:1",
        "r_rangeTextAddsNothing:1",
        "r_valueDidChange:\"aQQdef\"",
        // 5/6 — disconnected, and bubbling
        "s_disconnected:1",
        "t_bubblesToDocument:true",
    ] {
        assert!(
            got.contains(claim),
            "G_SELECT_EVENT: expected `{claim}`\n  got: {got}\n\n  \
             `select` is QUEUED (never synchronous), bubbles, is NOT cancelable, is trusted, and fires \
             once per ACTUAL change of (start, end, direction) — not coalesced. It is fired by the \
             SELECTION API, not by the selection value: `el.value =` moves the caret silently, and a \
             `setRangeText` that leaves the range alone fires nothing. Every row is Chrome-measured."
        );
    }
}
