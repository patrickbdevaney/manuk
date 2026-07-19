//! **G_SELECT_ACTUATION — every `<select>` on the web read as an empty string.**
//!
//! Measured before building (tick 253): `select.value` was `""`, `selectedIndex` `undefined`,
//! `options` `undefined`, `length` `0`, `option.selected` `undefined`. `HTMLSelectElement` existed
//! as an interface marker and nothing behind it.
//!
//! **The divergence that hid it.** Form *submission* reads the DOM directly and was always correct,
//! so a select submitted the right value while any script that branched on `select.value` saw an
//! empty string. Two paths to the same question, one right and one silent — the class this project
//! keeps finding (cf. the two image-decode passes in tick 249's wiki entry).
//!
//! ## How each assertion here can go RED
//!
//! - **`value` is the SELECTED OPTION's value.** RED, run: restore `el_get_value` to read the
//!   element's own `value` attribute and every select reports `""` again.
//! - **An implicit first selection.** A single-select with no `selected` attribute shows and submits
//!   its first option. RED, run: return `-1` from `selected_index` when nothing is marked — the
//!   honest-looking answer, and wrong.
//! - **Value fallback to text.** `<option>Blue</option>` with no `value` reports `"Blue"`. RED, run:
//!   drop the text fallback in `option_value` and every unvalued option reports `""`.
//! - **`optgroup` options still count.** RED, run: walk children instead of descendants and a
//!   grouped select — very common — reports zero options.
//! - **Setting deselects the others.** RED, run: `set_attr("selected")` without clearing siblings
//!   and two options are marked at once, leaving the control with no defined value.
//! - **`input` AND `change`, in that order.** React's `onChange` IS the `input` event. RED, run:
//!   fire only `change` and every React select goes unchanged while vanilla pages still work — a
//!   split that presents as "it works on some sites".

use manuk_page::Page;
use manuk_text::FontContext;

const W: f32 = 800.0;

/// One test — a `PageContext` is per-process, see `g_mouse_actuation.rs`.
#[test]
fn a_select_reports_its_selection_and_can_be_driven() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<select id="s">
  <option value="a">A</option>
  <option value="b" selected>B</option>
  <option value="c">C</option>
</select>

<!-- no `selected` anywhere: the browser shows, and submits, the FIRST option -->
<select id="implicit"><option value="x">X</option><option value="y">Y</option></select>

<!-- values fall back to text, and <optgroup> options still belong to the select -->
<select id="grouped">
  <optgroup label="Warm"><option>Red</option><option>Orange</option></optgroup>
  <optgroup label="Cool"><option>Blue</option></optgroup>
</select>

<div id="log"></div>
<script>
  var events = [];
  document.getElementById('s').addEventListener('input',  function () { events.push('input'); });
  document.getElementById('s').addEventListener('change', function () { events.push('change'); });

  window.__report = function () {
    var s = document.getElementById('s');
    var im = document.getElementById('implicit');
    var g = document.getElementById('grouped');
    document.getElementById('log').textContent =
      'value=' + s.value + ' idx=' + s.selectedIndex +
      ' implicitValue=' + im.value + ' implicitIdx=' + im.selectedIndex +
      ' groupedValue=' + g.value + ' groupedIdx=' + g.selectedIndex +
      ' events=' + events.join(',');
  };
  // `options` is not implemented yet (named residue), so reach the option directly.
  window.__optSelected = function (n) {
    return document.querySelectorAll('#s option')[n].selected;
  };
</script></body>"#,
        "https://select.test/",
        &fonts,
        W,
    );

    let root = p.dom().root();
    let s = manuk_css::query_selector_all(p.dom(), root, "#s")[0];
    let lg = manuk_css::query_selector_all(p.dom(), root, "#log")[0];

    let report = |p: &mut Page| {
        p.eval_for_test("window.__report();");
    };

    // ── 1. READING an explicitly-selected select ────────────────────────────────────────────
    report(&mut p);
    let out = p.dom().text_content(lg);
    assert!(
        out.contains("value=b"),
        "`select.value` is the SELECTED OPTION's value — reading the select's own `value` \
         attribute (what this did) returns \"\" for every select on the web. got: {out}"
    );
    assert!(
        out.contains("idx=1"),
        "selectedIndex follows the marked option; got: {out}"
    );

    // ── 2. The IMPLICIT first selection ─────────────────────────────────────────────────────
    assert!(
        out.contains("implicitValue=x") && out.contains("implicitIdx=0"),
        "a single-select with nothing marked selects its FIRST option — that is what the browser \
         shows and what the form submits. Reporting -1/\"\" is the honest-looking wrong answer. \
         got: {out}"
    );

    // ── 3. optgroup + value-falls-back-to-text ──────────────────────────────────────────────
    assert!(
        out.contains("groupedValue=Red") && out.contains("groupedIdx=0"),
        "`<option>Red</option>` has no value attribute and reports its TEXT, and options inside \
         <optgroup> still belong to the select — a children-only walk reports a grouped select as \
         empty. got: {out}"
    );

    // ── 4. ACTUATION: choose option 2, and both events fire in order ─────────────────────────
    assert!(p.select_option(s, 2, &fonts, W), "option 2 exists");
    report(&mut p);
    let out = p.dom().text_content(lg);
    assert!(
        out.contains("value=c") && out.contains("idx=2"),
        "driving the select moves the selection; got: {out}"
    );
    assert!(
        out.contains("events=input,change"),
        "`input` THEN `change`. React's onChange IS the input event, so firing only `change` \
         leaves every React select unchanged while vanilla pages work — a split that presents as \
         'it works on some sites'. got: {out}"
    );

    // ── 5. Exactly one option stays selected ────────────────────────────────────────────────
    p.eval_for_test(
        "document.getElementById('log').textContent = 'sel=' + \
         [0,1,2].map(function(n){return window.__optSelected(n);}).join(',');",
    );
    let out = p.dom().text_content(lg);
    assert!(
        out.contains("sel=false,false,true"),
        "exactly ONE option is selected after the change — a control with two marked options has \
         no defined value. got: {out}"
    );

    // ── 6. `select.value = x` selects by value; an unmatched value selects NOTHING ───────────
    p.eval_for_test("document.getElementById('s').value = 'a';");
    report(&mut p);
    let out = p.dom().text_content(lg);
    assert!(
        out.contains("value=a") && out.contains("idx=0"),
        "`select.value = 'a'` selects the option whose value is 'a'; got: {out}"
    );

    p.eval_for_test("document.getElementById('s').value = 'nope';");
    report(&mut p);
    let out = p.dom().text_content(lg);
    assert!(
        out.contains("idx=-1"),
        "a value matching no option selects NOTHING (index -1) — the spec's behaviour, and what \
         lets a page notice its own option list changed under it. got: {out}"
    );

    // ── 7. Out-of-range actuation is refused, not clamped ────────────────────────────────────
    assert!(
        !p.select_option(s, 99, &fonts, W),
        "an out-of-range index is refused rather than silently clamped to the last option"
    );
}
