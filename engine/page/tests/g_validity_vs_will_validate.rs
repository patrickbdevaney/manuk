//! **G_VALIDITY_VS_WILL_VALIDATE — `validity` describes the VALUE; `willValidate` says whether anyone
//! will act on it. They are two questions and we were answering one.**
//!
//! The second row of t1390's `html/semantics` survey: `forms/constraints`, 338 failing. Every
//! validation library — the browser's own native UI, React Hook Form, Formik, VeeValidate — reads
//! `el.validity.*`, calls `form.checkValidity()`, and listens for `invalid`.
//!
//! ⚠ **A SIBLING OF `g_constraint_validation.rs`, NOT A REPLACEMENT FOR IT.** That gate (tick 161)
//! asserts the API EXISTS and computes the common cases; this one asserts the two questions it
//! answers are SEPARATE. Both run. The distinction matters because this file was very nearly written
//! *over* that one — see the journal for t1391.
//!
//! ## ⭐⭐⭐ THE CENTRAL DEFECT — ONE EARLY RETURN CONFLATING TWO QUESTIONS
//!
//! `__computeValidity` returned an **all-false** `ValidityState` for any element barred from
//! constraint validation. Chrome-measured, a DISABLED `pattern="[a-z]+"` input holding `"123"`:
//!
//! ```text
//!   willValidate      false      it is barred — nothing will act on it
//!   patternMismatch   TRUE       the VALUE is still wrong, and the object still says so
//!   valid             false
//!   checkValidity()   true       the method asks "will this block submission", not "is this good"
//! ```
//!
//! **All four at once**, and a library reads exactly that combination to decide whether to draw its
//! own message. Collapsing them loses the only signal it has.
//!
//! ## THE SIX RULES, EVERY ROW CHROME-MEASURED
//!
//! ```text
//!  1  the flags are computed even when BARRED                    disabled+pattern → pm TRUE
//!  2  …EXCEPT valueMissing, which needs the element to be MUTABLE
//!                       <fieldset disabled> required empty  → vm FALSE, valid TRUE
//!                       readonly required empty             → vm FALSE
//!                       inside <datalist>, required empty   → vm TRUE  (barred but MUTABLE)
//!  3  checkValidity() is TRUE for a barred element, whatever `valid` says
//!  4  tooLong / tooShort need the DIRTY VALUE flag — only the USER sets it
//!                       <input maxlength=2 value="abcdef">  → tooLong FALSE
//!                       …and still FALSE after el.value = 'xyzxyz'
//!  5  `pattern` applies to six types only (text search url tel email password)
//!                       <input type=number pattern="[a-z]+" value="123"> → pm FALSE
//!  6  min/max apply to the TEMPORAL types too, compared LEXICOGRAPHICALLY
//!                       type=date  min=2020-01-01 value=2019-06-01 → rangeUnderflow TRUE
//!                       type=month / type=time likewise
//!  ⚠ and `disabled` INHERITS from an ancestor <fieldset disabled>, which `el.disabled` does not
//!    reflect — the IDL attribute is the element's OWN attribute.
//! ```
//!
//! ⭐⭐ **Rule 2 is the row that proves rule 1 is about the OBJECT and not about the element.** Two
//! disabled controls disagree: the one with a pattern mismatch reports `valid: false`, the one that
//! is merely required-and-empty reports `valid: true`. That is only possible if the mutability
//! clause lives on `valueMissing` — which is exactly where the spec puts it — rather than on the
//! whole computation.
//!
//! ⭐ **Rule 4 is a rule about who typed, not about how long the string is.** `maxlength` does not
//! make a value invalid; it stops the USER typing past it. A value from the markup or from a script
//! is over-length and VALID. There is no user typing in this engine yet, so both flags are currently
//! unreachable — **and that is the correct answer, not a missing feature.**
//!
//! ⭐ **Rule 6's comparison is lexicographic on purpose**: `YYYY-MM-DD`, `YYYY-MM`, `YYYY-Www`,
//! `HH:MM` and `YYYY-MM-DDTHH:MM` are fixed-width and zero-padded, so string order IS chronological
//! order. `parseFloat` on a date gives `2019`, which compares against `2020-01-01` as `NaN`.
//!
//! ⚠ Lives in `engine/page/tests/` (it needs SpiderMonkey); the wall does not run it — audit #78.
//! Its wall-independent guard is the WPT `html/semantics` row, in the metric since t1388.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body style="margin:0">
  <div id="out">-</div>
  <form id="f">
    <input id="a" pattern="[a-z]+" value="123" disabled>
    <input id="b" maxlength="2" value="abcdef">
    <input id="d" minlength="5" value="ab">
    <input id="e" type="number" pattern="[a-z]+" value="123">
    <input id="g" type="date" min="2020-01-01" max="2020-12-31" value="2019-06-01">
    <input id="h" type="date" min="2020-01-01" max="2020-12-31" value="2021-06-01">
    <input id="i" type="month" min="2020-01" max="2020-12" value="2019-06">
    <input id="j" type="time" min="10:00" max="12:00" value="09:00">
    <datalist><input id="k" required value=""></datalist>
    <fieldset disabled><input id="l" required value=""></fieldset>
    <input id="m" required value="" readonly>
    <input id="n" required value="">
    <input id="o" type="checkbox" pattern="[a-z]+">
  </form>
  <script>
    var R = [];
    globalThis.__report = function () {
      function row(id) {
        var e = document.getElementById(id), v = e.validity;
        R.push(id + ':will=' + e.willValidate + ',valid=' + v.valid + ',vm=' + v.valueMissing +
               ',pm=' + v.patternMismatch + ',tl=' + v.tooLong + ',ts=' + v.tooShort +
               ',ru=' + v.rangeUnderflow + ',ro=' + v.rangeOverflow + ',chk=' + e.checkValidity());
      }
      ['a','b','d','e','g','h','i','j','k','l','m','n','o'].forEach(row);
      var b = document.getElementById('b');
      b.value = 'xyzxyz';
      R.push('b_scriptset:tl=' + b.validity.tooLong);
      R.push('form:chk=' + document.getElementById('f').checkValidity());
      document.getElementById('out').textContent = R.join(' ');
    };
  </script></body></html>"#;

#[test]
fn validity_describes_the_value_and_will_validate_says_who_acts_on_it() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://cv.test/", &fonts, 800.0);
    page.eval_for_test("globalThis.__report && __report()");

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    // ── VACUITY. An ordinary required-and-empty control must still be INVALID, or an
    //    implementation that reports `valid: true` for everything satisfies most rows below.
    assert!(
        got.contains("n:will=true,valid=false,vm=true"),
        "VACUOUS: a plain required-and-empty input is not reported invalid, so nothing here is a \
         test of the algorithm. Got: {got}"
    );

    let rows: &[(&str, &str)] = &[
        ("a:will=false,valid=false,vm=false,pm=true,tl=false,ts=false,ru=false,ro=false,chk=true",
         "THE CENTRAL ROW — a DISABLED pattern-mismatching input: barred (will=false), the VALUE is still wrong (pm=true, valid=false), and checkValidity() is TRUE because the method asks whether submission is blocked. All four at once"),
        ("l:will=false,valid=true,vm=false",
         "…and the OTHER disabled control disagrees: required-and-empty inside <fieldset disabled> is VALID, because `valueMissing` is the one flag with a mutability clause. Two disabled rows, two answers — that is what puts the clause on the FLAG and not on the object"),
        ("m:will=false,valid=true,vm=false",
         "readonly is the other non-mutable state"),
        ("k:will=false,valid=false,vm=true",
         "inside a <datalist>: BARRED but MUTABLE, so `valueMissing` is still computed — the row that separates the two conditions"),
        ("b:will=true,valid=true,vm=false,pm=false,tl=false",
         "`maxlength=2` with a 6-character AUTHORED value is VALID: maxlength stops the USER typing, it does not invalidate a value"),
        ("b_scriptset:tl=false",
         "…and a SCRIPT set does not dirty it either — the flag is about who typed, not how long the string is"),
        ("d:will=true,valid=true,vm=false,pm=false,tl=false,ts=false",
         "the minlength twin"),
        ("e:will=true,valid=true,vm=false,pm=false",
         "`pattern` on a `type=number` is IGNORED — applying it everywhere makes a numeric field permanently invalid because of an attribute the spec says it must ignore"),
        ("o:will=true,valid=true,vm=false,pm=false",
         "…and on a checkbox"),
        ("g:will=true,valid=false,vm=false,pm=false,tl=false,ts=false,ru=true,ro=false,chk=false",
         "min/max apply to `type=date`, compared LEXICOGRAPHICALLY — parseFloat on a date gives 2019 and compares as NaN"),
        ("h:will=true,valid=false,vm=false,pm=false,tl=false,ts=false,ru=false,ro=true,chk=false",
         "…and the overflow direction"),
        ("i:will=true,valid=false,vm=false,pm=false,tl=false,ts=false,ru=true,ro=false,chk=false",
         "…and `type=month`"),
        ("j:will=true,valid=false,vm=false,pm=false,tl=false,ts=false,ru=true,ro=false,chk=false",
         "…and `type=time`, whose HH:MM is fixed-width for the same reason"),
        ("form:chk=false",
         "the FORM's check is the conjunction over its controls, and the barred ones do not contribute"),
    ];
    for (claim, why) in rows {
        assert!(
            got.contains(claim),
            "G_VALIDITY_VS_WILL_VALIDATE: expected {claim:?}.\n  {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  restore the `if (!__willValidate(el)) return all-false` early return
//       -> `a` reads pm=false/valid=true and `k` reads vm=false. The two barred-but-different rows
//          collapse into one answer, which is the defect.
// N2  drop the `__isMutable` clause from `valueMissing`
//       -> `l` and `m` read vm=true/valid=false. Only the mutability rows move; `a` stays green,
//          which is what says the clause is on the FLAG.
// N3  compute `tooLong` without the dirty-value flag
//       -> `b` and `b_scriptset` read tl=true. A maxlength attribute would invalidate every
//          server-rendered value longer than it.
// N4  disable the temporal min/max arm (which is what a numeric-only implementation is)
//       -> the four temporal rows lose their range flags. `parseFloat` is the same mutation by
//          another route: `2019-06-01` parses as 2019 and `2020-01` as 2020, so a MONTH-early
//          value compares by year and a `type=time` value does not compare at all.
