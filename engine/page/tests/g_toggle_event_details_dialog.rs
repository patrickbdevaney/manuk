//! **G_TOGGLE_EVENT_DETAILS_DIALOG — the same event, three elements, and only one of them fired it.**
//!
//! t1395 built the `ToggleEvent` interface for `[popover]` and recorded, in its own journal, that
//! Chrome's `toggle` is queued and COALESCED — *"measured, not built"*. This is the other two
//! elements, and they were wrong in four different ways at once:
//!
//! ```text
//!                            chrome                           before
//!   <details>.open = true    toggle, QUEUED, ToggleEvent,     beforetoggle AND toggle, SYNCHRONOUS,
//!                            oldState/newState set, trusted   plain Event, both states UNDEFINED
//!   <dialog>.showModal()     beforetoggle + toggle            NOTHING AT ALL
//!   open/close in one task   ONE toggle, `closed > closed`    two
//! ```
//!
//! ⭐⭐ **`<details>` fires NO `beforetoggle`, and we fired one.** `[popover]` and `<dialog>` emit
//! both; a `<details>` emits `toggle` only. A spurious cancel-shaped event on an element whose spec
//! has no cancel point is not a harmless extra — a component listening for `beforetoggle` to veto
//! (the popover idiom) would believe it had a veto here.
//!
//! ⭐ **COALESCED — the opposite of the `select` event one tick earlier.** t1394 measured that two
//! different selection changes in one task fire TWO events; two `open` changes here fire ONE, whose
//! `oldState` is the FIRST transition's and whose `newState` is the LAST. Two async notifications in
//! adjacent subsystems with opposite batching rules, neither inferable from the other.
//!
//! ## ⭐⭐⭐ TWO ENTRANCES, AND THE FIRST FIX ONLY FOUND ONE
//!
//! `details.open = true` and `details.setAttribute('open','')` are the same state change, and WPT's
//! `toggleEvent.html` writes it the SECOND way in every one of its eleven cases. A first
//! implementation hooked the IDL reflection setter, passed a hand-written probe, and moved that file
//! by **one**.
//!
//! The choke point is the ATTRIBUTE: `reflect_js`'s boolean setter is literally
//! `if (v) el.setAttribute(a,''); else el.removeAttribute(a)`. Hooking there covers both spellings
//! with one implementation — and the exclusive-accordion sibling, which removes the attribute
//! directly, gets its `toggle` for free instead of a second hand-written dispatch that could drift.
//! Both duplicate dispatches were then DELETED. It is t1397's `record_mutation` lesson in a second
//! place: **when N surfaces cause one state change, hook what they funnel through, not the N.**
//!
//! ## ⚠⚠ AND `isTrusted` HAD TO BE OVERRIDDEN FOR THE FOURTH TIME
//!
//! `assert_true: event is trusted expected true got false` was the only thing left between this and
//! the file. `__dispatchEvent` infers `isTrusted` from *"was an event OBJECT supplied"*, and an object
//! must be supplied to carry `oldState`/`newState`. That inference has now been overridden for the
//! `select` event (t1394), the popover `ToggleEvent` (t1395), the `<img>` `load` (t1399) and this.
//! **A default that is wrong for every engine-synthesised event is a default pointing the wrong way.**
//!
//! ⚠ ONE `#[test]` in this binary: two SpiderMonkey contexts in one manuk-page binary tear each other
//! down and the second test reads the first one's empty output.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<details id="d"><summary>s</summary><p>body</p></details>
<dialog id="dlg">hi</dialog>
<div id="out">-</div>
<script>
var r = [];
function k(n, v) { r.push(n + ':' + JSON.stringify(v)); }
var d = document.getElementById('d'), dlg = document.getElementById('dlg');

// ── 1. <details>: `toggle` ONLY, QUEUED, a real ToggleEvent, trusted ──────────────────
var dEv = [];
d.addEventListener('beforetoggle', function (e) { dEv.push('BEFORETOGGLE'); });
d.addEventListener('toggle', function (e) {
  dEv.push(e.oldState + '>' + e.newState);
  window.dCtor = e.constructor && e.constructor.name;
  window.dTrusted = e.isTrusted;
});
d.open = true;
k('a_syncAfterIdlSet', dEv.slice());          // [] — it is a QUEUED task

// ── 2. THE OTHER ENTRANCE — setAttribute is the same state change ─────────────────────
var attrEl = document.createElement('details'); document.body.appendChild(attrEl);
var aEv = [];
attrEl.addEventListener('toggle', function (e) { aEv.push(e.oldState + '>' + e.newState); });
attrEl.setAttribute('open', '');

// ── 3. COALESCING — first oldState, last newState, ONE event ──────────────────────────
var co = document.createElement('details'); document.body.appendChild(co);
var cEv = [];
co.addEventListener('toggle', function (e) { cEv.push(e.oldState + '>' + e.newState); });
co.setAttribute('open', ''); co.removeAttribute('open');

// ── 4. <dialog>: BOTH events, and it fired neither ────────────────────────────────────
var gEv = [];
dlg.addEventListener('beforetoggle', function (e) { gEv.push('bt:' + e.oldState + '>' + e.newState); });
dlg.addEventListener('toggle', function (e) { gEv.push('t:' + e.oldState + '>' + e.newState); });
dlg.showModal();
k('b_dialogSyncHasBeforetoggle', gEv.slice());   // beforetoggle is SYNCHRONOUS

setTimeout(function () {
  k('c_detailsEvents', dEv);
  k('d_detailsCtor', window.dCtor);
  k('e_detailsTrusted', window.dTrusted);
  k('f_setAttributeEntrance', aEv);
  k('g_coalescedToOne', cEv);
  k('h_dialogEvents', gEv);
  document.getElementById('out').textContent = r.join(' ');
}, 300);
</script></body></html>"##;

/// One test in the binary — see the module note.
#[test]
fn details_and_dialog_queue_a_real_toggle_event_through_one_choke_point() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://toggle2.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("TOGGLE-DETAILS-DIALOG RESULT: {got}");

    for claim in [
        // 1 — queued, and `<details>` has NO beforetoggle
        "a_syncAfterIdlSet:[]",
        "c_detailsEvents:[\"closed>open\"]",
        "d_detailsCtor:\"ToggleEvent\"",
        "e_detailsTrusted:true",
        // 2 — the OTHER entrance reaches the same implementation
        "f_setAttributeEntrance:[\"closed>open\"]",
        // 3 — coalesced: first oldState, last newState, ONE event
        "g_coalescedToOne:[\"closed>closed\"]",
        // 4 — <dialog> fires BOTH, and beforetoggle is synchronous
        "b_dialogSyncHasBeforetoggle:[\"bt:closed>open\"]",
        "h_dialogEvents:[\"bt:closed>open\",\"t:closed>open\"]",
    ] {
        assert!(
            got.contains(claim),
            "G_TOGGLE_EVENT_DETAILS_DIALOG: expected `{claim}`\n  got: {got}\n\n  \
             `<details>` fires a QUEUED, trusted `ToggleEvent` named `toggle` and NO `beforetoggle`; \
             `<dialog>` fires a synchronous `beforetoggle` and a queued `toggle`. Both spellings of \
             the state change — the `.open` IDL setter and `setAttribute('open')` — must reach the \
             same implementation, and two changes in one task must COALESCE to one event carrying \
             the FIRST oldState and the LAST newState. Every row is headless-Chrome-measured."
        );
    }
}
