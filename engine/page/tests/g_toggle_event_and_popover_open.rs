//! **G_TOGGLE_EVENT_AND_POPOVER_OPEN — the popover's OBSERVABLE state.**
//!
//! `[popover]` was **half-installed**: `showPopover()` worked, the element painted, and
//! `beforetoggle`/`toggle` fired with the right `oldState`/`newState` — while three of the four ways a
//! page can *observe* any of that were missing. `html/semantics/popovers` sat at **9 of 153 (5.9%)**.
//!
//!   * ⭐⭐⭐ **`:popover-open` never matched.** The state was real — `showPopover()` writes
//!     `data-manuk-popover-open` and the UA sheet keys `display` off it, so the popover opened and
//!     painted correctly — but the selector for it was never wired, in EITHER matcher. **A state that
//!     exists and cannot be asked about.**
//!   * ⭐⭐ **`ToggleEvent` was not a global** (`ToggleEvent is not defined`, 38 subtests in one file),
//!     and the events we fired were plain `Event`s wearing two extra properties.
//!   * ⭐ Every event interface's attributes were WRITABLE, so a listener could rewrite an event's
//!     payload and every later listener saw the forgery.
//!   * ⭐ Every event constructor accepted a missing `type` argument.
//!
//! ⚠ **`:popover-open` had to be taught to BOTH selector engines** — the minimal one behind
//! `element.matches()` / `querySelector`, and the Stylo one behind the live cascade. This gate asks
//! through both doors on the same element in the same state, because a rule that reaches a stylesheet
//! and not a script (or the reverse) is the twin-drift this codebase keeps finding.
//!
//! ⚠ ONE `#[test]` in this binary: two SpiderMonkey contexts in one manuk-page binary tear each other
//! down and the second test reads the first one's empty output.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  #p:popover-open { outline-color: rgb(1, 2, 3); }
</style></head><body>
<div id="p" popover>hello</div>
<div id="out">-</div>
<script>
var r = [];
function k(n, v) { r.push(n + ':' + JSON.stringify(v)); }
function thr(f) { try { f(); return 'no-throw'; } catch (e) { return e.name; } }
var p = document.getElementById('p');

// ── 1. THE INTERFACE EXISTS AND INHERITS ──────────────────────────────────────────────
k('a_isGlobal', typeof ToggleEvent);
k('b_instanceOfSelf', new ToggleEvent('') instanceof ToggleEvent);
k('c_instanceOfEvent', new ToggleEvent('') instanceof Event);
k('d_ctorName', ToggleEvent.name);
k('e_brand', Object.prototype.toString.call(new ToggleEvent('x')));

// ── 2. THE TYPE ARGUMENT IS REQUIRED — on EVERY event constructor ─────────────────────
k('f_toggleNoArgs', thr(function () { new ToggleEvent(); }));
k('g_eventNoArgs', thr(function () { new Event(); }));
k('h_customEventNoArgs', thr(function () { new CustomEvent(); }));
// ⚠ …but `undefined` PASSED EXPLICITLY is a legal call whose type is the string "undefined".
k('i_explicitUndefined', new ToggleEvent(undefined).type);
k('j_explicitNull', new ToggleEvent(null).type);

// ── 3. THE ATTRIBUTES ARE READONLY ────────────────────────────────────────────────────
var t = new ToggleEvent('t');
k('k_oldDefault', t.oldState);
k('l_newDefault', t.newState);
k('m_sourceDefault', t.source === null);
var t2 = new ToggleEvent('t', { oldState: 'closed', newState: 'open' });
t2.oldState = 'ZZZ'; t2.newState = 'ZZZ';
k('n_oldReadonly', t2.oldState);
k('o_newReadonly', t2.newState);
var c = new CustomEvent('c', { detail: 5 }); c.detail = 9;
k('p_detailReadonly', c.detail);

// ── 4. `:popover-open` — BOTH DOORS, same element, same state ─────────────────────────
function door1() { return p.matches(':popover-open'); }                 // the selector engine
function door2() { return getComputedStyle(p).outlineColor; }           // the live cascade
function qsa() { return document.querySelectorAll(':popover-open').length; }
k('q_closedMatches', door1());
k('r_closedQsa', qsa());
p.showPopover();
k('s_openMatches', door1());
k('t_openQsa', qsa());
k('u_openCascade', door2());              // the stylesheet rule applies only when open
k('v_openDisplay', getComputedStyle(p).display);
p.hidePopover();
k('w_closedAgainMatches', door1());
k('x_closedAgainDisplay', getComputedStyle(p).display);

// ── 5. THE POPOVER FIRES A REAL ToggleEvent ───────────────────────────────────────────
var seen = null;
p.addEventListener('beforetoggle', function (e) { seen = e; });
p.showPopover();
k('y_eventIsToggleEvent', seen instanceof ToggleEvent);
k('z_eventStates', seen && (seen.oldState + '>' + seen.newState));
k('za_eventCancelable', seen && seen.cancelable);

document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

/// One test in the binary — see the module note.
#[test]
fn popover_open_is_askable_through_both_doors_and_toggle_is_a_real_event_interface() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://popover-state.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("POPOVER-STATE RESULT: {got}");

    for claim in [
        // 1 — the interface
        "a_isGlobal:\"function\"",
        "b_instanceOfSelf:true",
        "c_instanceOfEvent:true",
        "d_ctorName:\"ToggleEvent\"",
        "e_brand:\"[object ToggleEvent]\"",
        // 2 — a required argument is required, and an explicit `undefined` is NOT missing
        "f_toggleNoArgs:\"TypeError\"",
        "g_eventNoArgs:\"TypeError\"",
        "h_customEventNoArgs:\"TypeError\"",
        "i_explicitUndefined:\"undefined\"",
        "j_explicitNull:\"null\"",
        // 3 — readonly attributes
        "k_oldDefault:\"\"",
        "l_newDefault:\"\"",
        "m_sourceDefault:true",
        "n_oldReadonly:\"closed\"",
        "o_newReadonly:\"open\"",
        "p_detailReadonly:5",
        // 4 — :popover-open through BOTH doors
        "q_closedMatches:false",
        "r_closedQsa:0",
        "s_openMatches:true",
        "t_openQsa:1",
        "u_openCascade:\"rgb(1, 2, 3)\"",
        "v_openDisplay:\"block\"",
        "w_closedAgainMatches:false",
        "x_closedAgainDisplay:\"none\"",
        // 5 — a real ToggleEvent, not an Event wearing two properties
        "y_eventIsToggleEvent:true",
        "z_eventStates:\"closed>open\"",
        "za_eventCancelable:true",
    ] {
        assert!(
            got.contains(claim),
            "G_TOGGLE_EVENT_AND_POPOVER_OPEN: expected `{claim}`\n  got: {got}\n\n  \
             An open popover must be askable through BOTH selector doors (`matches`/`querySelectorAll` \
             AND the live cascade), and its notification must be a real `ToggleEvent` — a global that \
             inherits Event, carries readonly `oldState`/`newState`/`source`, and refuses a missing \
             `type` argument. Every row is headless-Chrome-measured."
        );
    }
}
