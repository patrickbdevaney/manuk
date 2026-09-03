//! **G_SELECTION_APPLICABILITY — whether the text-selection API applies, and where the caret rests
//! before anyone has moved it.**
//!
//! Two questions the engine answered as one. `selectionStart` returned `0` for every element on the
//! page — a `type=number`, a `type=date`, a `<div>` — and returned **the length of the value** for the
//! text fields it did apply to. Both answers are wrong in the same way: they describe a cursor that is
//! not there.
//!
//!   * **`el.selectionStart !== null` is how a mask/caret library asks "is this a text field".** We
//!     said `0`, never `null`, so the answer was YES for a spinner, a date picker and an email field,
//!     and the library then computed an offset into a control that has no cursor.
//!   * **The caret's RESTING place is 0; its POST-WRITE place is the end.** We published the second as
//!     the first, so a freshly-rendered form reported a cursor at the end of text nobody had typed.
//!
//! Every row below is headless-Chrome-measured. The teeth are read-back values and thrown exception
//! NAMES, so a stub returning a constant — or one that throws the wrong error — fails.
//!
//! ⚠ ONE `#[test]` in this binary on purpose: two SpiderMonkey contexts in one manuk-page binary tear
//! each other down and the second test reads the first one's empty output.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<input id="i" value="abcdef">
<textarea id="t">abcdef</textarea>
<div id="out">-</div>
<script>
var r = [];
function k(n, v) { r.push(n + ':' + JSON.stringify(v)); }
function mk(type, val) {
  var e = document.createElement('input');
  if (type !== null) { e.setAttribute('type', type); }
  if (val !== undefined) { e.setAttribute('value', val); }
  document.body.appendChild(e);
  return e;
}
function thrown(f) { try { f(); return 'no-throw'; } catch (e) { return e.name; } }

// ── 1. THE RESTING CARET IS 0, not the value length ────────────────────────────────────
var i = document.getElementById('i'), t = document.getElementById('t');
k('a_inputDefault', i.selectionStart + '-' + i.selectionEnd);      // Chrome 0-0
k('b_textareaDefault', t.selectionStart + '-' + t.selectionEnd);   // Chrome 0-0
k('c_dirDefault', i.selectionDirection);   // ours "none"; Chrome "forward" — WPT accepts either

// ── 2. THE FIVE TYPES IT APPLIES TO, plus the unknown-keyword fallback ─────────────────
var applies = '';
['text','search','tel','url','password'].forEach(function(ty) {
  applies += (mk(ty, 'abcdef').selectionStart === 0) ? '1' : '0';
});
k('d_fiveApply', applies);                                          // "11111"
k('e_unknownTypeApplies', mk('aninvalidtype', 'abcdef').selectionStart);  // 0 — falls back to Text
k('f_noTypeApplies', mk(null, 'abcdef').selectionStart);                 // 0 — missing type is Text

// ── 3. AND THE ONES IT DOES NOT — null, never 0 ────────────────────────────────────────
var nulls = '';
['email','number','date','month','week','time','datetime-local','range','color',
 'checkbox','radio','file','hidden','submit','image','reset','button'].forEach(function(ty) {
  var e = mk(ty);
  nulls += (e.selectionStart === null && e.selectionEnd === null &&
            e.selectionDirection === null) ? '1' : '0';
});
k('g_seventeenNull', nulls);                                        // seventeen 1s

// ── 4. THE SETTERS THROW InvalidStateError — and select() DOES NOT ─────────────────────
var n = mk('number', '123');
k('h_setStart', thrown(function() { n.selectionStart = 0; }));
k('i_setEnd', thrown(function() { n.selectionEnd = 0; }));
k('j_setDir', thrown(function() { n.selectionDirection = 'none'; }));
k('k_ssr', thrown(function() { n.setSelectionRange(0, 1); }));
k('l_srt', thrown(function() { n.setRangeText('foobar'); }));
k('m_selectNoThrow', thrown(function() { n.select(); }));           // the ONE that must not throw

// ── 5. AN APPLICABLE TYPE THROWS NOTHING ──────────────────────────────────────────────
var ok = mk('password', 'abcdef');
k('n_applicableNoThrow', thrown(function() {
  ok.selectionStart = 1; ok.selectionEnd = 1; ok.selectionDirection = 'forward';
  ok.setSelectionRange(0, 1); ok.setRangeText('z');
}));

// ── 6. `.value =` COLLAPSES THE CARET TO THE END — ONLY IF THE VALUE CHANGED ───────────
var c1 = mk('text', 'abcdef'); c1.setSelectionRange(1, 3); c1.value = 'zzzzzzzzzz';
k('o_valueChangedLonger', c1.selectionStart + '-' + c1.selectionEnd);   // 10-10
var c2 = mk('text', 'abcdef'); c2.setSelectionRange(2, 5); c2.value = 'ab';
k('p_valueChangedShorter', c2.selectionStart + '-' + c2.selectionEnd);  // 2-2
var c3 = mk('text', 'abcdef'); c3.setSelectionRange(2, 4); c3.value = 'abcdef';
k('q_valueSameKept', c3.selectionStart + '-' + c3.selectionEnd);        // 2-4 — the PAIR row

// ── 7. selectionDirection ROUND-TRIPS, and an unknown keyword RESETS to the default ───
var d = mk('text', 'abcdef');
d.selectionDirection = 'backward'; k('r_dirBackward', d.selectionDirection);
d.selectionDirection = 'sideways'; k('s_dirBogusResets', d.selectionDirection);
d.selectionDirection = 'forward';  k('t_dirForward', d.selectionDirection);

// ── 8. setRangeText `preserve` — the two edges ask DIFFERENT questions ─────────────────
function pres(s0, s1, rs, re, repl) {
  var e = mk('text', 'abcdefgh');
  e.setSelectionRange(s0, s1);
  e.setRangeText(repl, rs, re);
  return e.value + '|' + e.selectionStart + '-' + e.selectionEnd;
}
k('u_caretInside', pres(3, 3, 2, 5, 'Z'));        // Chrome abZfgh|2-3   (we gave 2-2)
k('v_bothInside', pres(3, 4, 2, 5, 'Z'));         // Chrome abZfgh|2-3   (we gave 2-2)
k('w_startBefore', pres(0, 3, 2, 5, 'Z'));        // Chrome abZfgh|0-3   (we gave 0-2)
k('x_caretInsideGrow', pres(4, 4, 2, 5, 'ZZZZ')); // Chrome abZZZZfgh|2-6 (we gave 2-2)
k('y_spanning', pres(3, 7, 2, 5, 'Z'));           // Chrome abZfgh|2-5   — boundary row, agreed before
k('z_exactRange', pres(2, 5, 2, 5, 'Z'));         // Chrome abZfgh|2-3   — boundary row, agreed before

// ── 9. AN INVERTED RANGE COLLAPSES ONTO ITS END — but a single-edge SETTER wins instead ───
function ssr(a, b) {
  var e = mk('text', 'abcdef'); e.setSelectionRange(a, b);
  return e.selectionStart + '-' + e.selectionEnd;
}
k('aa_ssrInverted', ssr(2, 1));        // Chrome 1-1 — the END wins, we dragged end UP to 2-2
k('ab_ssrEndZero', ssr(5, 0));         // Chrome 0-0
k('ac_ssrStartPastLen', ssr(7, 1));    // Chrome 1-1 — clamp to 6, then collapse onto end
var s1 = mk('text', 'abcdef'); s1.setSelectionRange(1, 3); s1.selectionStart = 5;
k('ad_setStartOverEnd', s1.selectionStart + '-' + s1.selectionEnd);   // Chrome 5-5 — START wins
var s2 = mk('text', 'abcdef'); s2.setSelectionRange(2, 4); s2.selectionEnd = 1;
k('ae_setEndUnderStart', s2.selectionStart + '-' + s2.selectionEnd);  // Chrome 1-1 — END wins

document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

/// One test in the binary — see the module note.
#[test]
fn selection_api_applies_to_five_types_and_the_caret_rests_at_zero() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://selection-applies.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SELECTION-APPLICABILITY RESULT: {got}");

    for claim in [
        // 1 — the resting caret
        "a_inputDefault:\"0-0\"",
        "b_textareaDefault:\"0-0\"",
        "c_dirDefault:\"none\"",
        // 2 — the applicable set, and the unknown-keyword fallback into it
        "d_fiveApply:\"11111\"",
        "e_unknownTypeApplies:0",
        "f_noTypeApplies:0",
        // 3 — and everything else is null, not 0
        "g_seventeenNull:\"11111111111111111\"",
        // 4 — the setters throw; select() does not
        "h_setStart:\"InvalidStateError\"",
        "i_setEnd:\"InvalidStateError\"",
        "j_setDir:\"InvalidStateError\"",
        "k_ssr:\"InvalidStateError\"",
        "l_srt:\"InvalidStateError\"",
        "m_selectNoThrow:\"no-throw\"",
        // 5 — an applicable type throws nothing
        "n_applicableNoThrow:\"no-throw\"",
        // 6 — the value-write collapse, and the same-value row that names it a CHANGE detector
        "o_valueChangedLonger:\"10-10\"",
        "p_valueChangedShorter:\"2-2\"",
        "q_valueSameKept:\"2-4\"",
        // 7 — direction round-trips
        "r_dirBackward:\"backward\"",
        "s_dirBogusResets:\"none\"",
        "t_dirForward:\"forward\"",
        // 8 — setRangeText preserve: an edge INSIDE the replaced span
        "u_caretInside:\"abZfgh|2-3\"",
        "v_bothInside:\"abZfgh|2-3\"",
        "w_startBefore:\"abZfgh|0-3\"",
        "x_caretInsideGrow:\"abZZZZfgh|2-6\"",
        "y_spanning:\"abZfgh|2-5\"",
        "z_exactRange:\"abZfgh|2-3\"",
        // 9 — the collapse direction: setSelectionRange onto END, a single-edge setter onto ITSELF
        "aa_ssrInverted:\"1-1\"",
        "ab_ssrEndZero:\"0-0\"",
        "ac_ssrStartPastLen:\"1-1\"",
        "ad_setStartOverEnd:\"5-5\"",
        "ae_setEndUnderStart:\"1-1\"",
    ] {
        assert!(
            got.contains(claim),
            "G_SELECTION_APPLICABILITY: expected `{claim}`\n  got: {got}\n\n  \
             The text-selection API applies to <textarea> and to <input> in Text/Search/Telephone/URL/\
             Password state (an unknown `type` keyword falls back to Text) and to NOTHING else: \
             elsewhere the getters are null and the setters throw InvalidStateError — except select(), \
             which never throws. The caret RESTS at 0; only a `.value` write that CHANGES the value \
             moves it to the end. Every row is headless-Chrome-measured."
        );
    }
}
