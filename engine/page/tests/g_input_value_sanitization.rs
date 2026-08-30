//! **G_INPUT_VALUE_SANITIZATION — what `input.value` MEANS, as opposed to what is stored.**
//!
//! Found by surveying `html/semantics` — the largest failing area on the board, and one that was not
//! on the board at all until t1388 opened the metric's aperture. The survey ranked its subtrees,
//! then histogrammed the biggest one's failure MESSAGES:
//!
//! ```text
//!   html/semantics/forms/the-input-element        828 failing  (43.0%)   ← the largest
//!     …of which "input.value should be '…' after change of state"   280   ← ONE mechanism, 34%
//!   html/semantics/scripting-1/the-script-element 484
//!   html/semantics/forms/textfieldselection       385
//!   html/semantics/forms/constraints              338
//! ```
//!
//! *An area percentage is not a work item; a message histogram is.*
//!
//! ## THE MECHANISM
//!
//! An `<input>` has two values: the CONTENT ATTRIBUTE, which keeps whatever was written, and the API
//! value, which is the content run through **the value sanitization algorithm** for the element's
//! current `type`. We had only the first.
//!
//! ⭐ **A paste from a spreadsheet carries a `\r`.** It came back out of `.value` with the carriage
//! return still in it, was submitted with it, and compared unequal to the same text typed by hand —
//! a bug that looks like a server problem from every angle except this one.
//!
//! ## THE TABLE, EVERY ROW CHROME-MEASURED (`--headless=new`), from `"  foo\rbar  "`
//!
//! ```text
//!   text · search · tel · password · hidden · checkbox ·
//!   radio · submit · image · reset · button      "  foobar  "   strip CR/LF only
//!   url · email                                  "foobar"       strip CR/LF, then TRIM
//!   date · month · week · time ·
//!   datetime-local · number                      ""             valid-or-empty
//!   range                                        "50"           valid-or-default, then CLAMP
//!   color                                        "#000000"      valid-or-black, LOWERCASED
//!
//!   number  "50" → "50"      " 50 " → ""     ⚠ number does NOT trim
//!   range   "200" → "100"    "-5" → "0"      ⚠ range CLAMPS to [min, max]
//!   color   "#ABCDEF" → "#abcdef"            ⚠ and lowercases
//!   date    "2020-01-02" → "2020-01-02"      "nope" → ""
//! ```
//!
//! ⭐⭐ **`number` does not trim and `url`/`email` do — that pair is the whole shape of the
//! algorithm.** It is not *"clean up the string"*; it is a per-type definition of what the string is
//! allowed to BE, and the two trimming types are the two whose values are conventionally pasted with
//! surrounding space. A single "trim and strip" implementation passes the text rows, the url rows and
//! the email rows, and is wrong on `number`.
//!
//! ⭐ **`range` is valid-or-DEFAULT, and the default is the MIDPOINT.** An unset range reads `50`,
//! not `0` and not `""` — which is why a slider rendered from `.value` before the user touches it has
//! its thumb in the middle. An empty string would put it nowhere.
//!
//! ## ⚠ APPLIED ON READ, WITH ONE NAMED CONSEQUENCE
//!
//! Sanitising on READ keeps the content attribute intact — so `getAttribute("value")` returns what
//! the author wrote while `.value` returns what the type means, which is exactly the spec's split —
//! and it makes a `type` change re-sanitise for free, because the next read asks the NEW type.
//!
//! ⚠ The divergence is a MULTI-STEP type change: the spec's sanitiser is destructive, so
//! `text → number → text` leaves `""` in Chrome where reading through the raw content gives the text
//! back. Measured, named, not built — fixing it needs a separate dirty-value store, which is a
//! different mechanism from the algorithm itself.
//!
//! ⚠ This gate lives in `engine/page/tests/` because it needs SpiderMonkey (`manuk-agent` does not
//! enable it), which the wall does not run — surface audit #78. Its wall-independent guard is the
//! WPT `html/semantics` row, which entered the primary metric at t1388.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body style="margin:0">
  <div id="out">-</div>
  <input id="attr" type="number" value="abc">
  <script>
    var R = [];
    function mk(type, value, extra) {
      var i = document.createElement('input');
      if (extra) { for (var k in extra) { i.setAttribute(k, extra[k]); } }
      i.type = type; i.value = value;
      return i;
    }
    function t(label, type, value, extra) {
      R.push(label + '=' + JSON.stringify(mk(type, value, extra).value));
    }
    globalThis.__report = function () {
      var V = "  foo\rbar  ";
      ['text','search','tel','password','hidden','checkbox','radio','submit','reset','button']
        .forEach(function (ty) { t('strip_' + ty, ty, V); });
      t('trim_url', 'url', V);
      t('trim_email', 'email', V);
      ['date','month','week','time','datetime-local','number'].forEach(function (ty) {
        t('empty_' + ty, ty, V);
      });
      t('range_junk', 'range', V);
      t('color_junk', 'color', V);
      t('num_ok', 'number', '50');
      t('num_spaced', 'number', ' 50 ');
      t('num_junk', 'number', 'abc');
      t('range_hi', 'range', '200');
      t('range_lo', 'range', '-5');
      t('range_minmax', 'range', '5', { min: '10', max: '20' });
      t('color_case', 'color', '#ABCDEF');
      t('color_named', 'color', 'red');
      t('date_ok', 'date', '2020-01-02');
      t('date_junk', 'date', 'nope');
      t('email_pad', 'email', '  a@b.c  ');
      t('text_lf', 'text', 'a\nb');
      // The two values of one element: the CONTENT attribute keeps what was authored.
      var a = document.getElementById('attr');
      R.push('attr_raw=' + JSON.stringify(a.getAttribute('value')));
      R.push('attr_api=' + JSON.stringify(a.value));
      // A type change re-sanitises: the same element, read as two different types.
      var c = mk('text', V);
      R.push('chg_text=' + JSON.stringify(c.value));
      c.type = 'email';
      R.push('chg_email=' + JSON.stringify(c.value));
      document.getElementById('out').textContent = R.join(' ');
    };
  </script></body></html>"#;

#[test]
fn the_api_value_is_the_sanitised_one() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://input.test/", &fonts, 800.0);
    page.eval_for_test("globalThis.__report && __report()");

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    // ── VACUITY. The report ran, and a value that needs NO sanitisation survives intact — an
    //    implementation that returned "" for everything satisfies half the rows below.
    assert!(
        got.contains("num_ok=\"50\""),
        "VACUOUS: a valid number did not survive, so the emptied rows below prove nothing. Got: {got}"
    );
    assert!(
        got.contains("date_ok=\"2020-01-02\""),
        "VACUOUS: a valid date did not survive either. Got: {got}"
    );

    let rows: &[(&str, &str)] = &[
        ("strip_text=\"  foobar  \"", "CR/LF is stripped and the surrounding space is KEPT — a paste from a spreadsheet stops carrying its carriage return"),
        ("strip_password=\"  foobar  \"", "…and the same for every non-trimming text-ish type"),
        ("strip_hidden=\"  foobar  \"", "…including `hidden`, which is a value-mode type even though nothing renders"),
        ("strip_checkbox=\"  foobar  \"", "…and the non-text types whose value is a submission token"),
        ("trim_url=\"foobar\"", "url TRIMS as well as strips — half the algorithm, and only two types have it"),
        ("trim_email=\"foobar\"", "…and email is the other one"),
        ("empty_number=\"\"", "a value that is not a valid floating-point number is EMPTY, not the raw string"),
        ("empty_date=\"\"", "…and the five temporal types are valid-or-empty too"),
        ("empty_datetime-local=\"\"", "…including the compound one"),
        ("range_junk=\"50\"", "range is valid-or-DEFAULT and the default is the MIDPOINT — a slider drawn from .value sits in the middle, not at zero and not nowhere"),
        ("color_junk=\"#000000\"", "color is valid-or-BLACK"),
        ("num_spaced=\"\"", "⭐ NUMBER DOES NOT TRIM. ' 50 ' is invalid, and this is the row that stops one 'trim and strip' implementation from covering the whole algorithm"),
        ("num_junk=\"\"", "…nor does it parse loosely"),
        ("range_hi=\"100\"", "range CLAMPS to the maximum"),
        ("range_lo=\"0\"", "…and to the minimum"),
        ("range_minmax=\"10\"", "…and the bounds are min/max, not 0..100 — 5 under min=10 clamps UP"),
        ("color_case=\"#abcdef\"", "color LOWERCASES, so a page comparing against its own palette string matches"),
        ("color_named=\"#000000\"", "a CSS colour NAME is not a valid simple colour"),
        ("date_junk=\"\"", "and a date that is not one is empty"),
        ("email_pad=\"a@b.c\"", "the real-world email case: a pasted address with surrounding space"),
        ("text_lf=\"ab\"", "a newline in a single-line control is REMOVED, not replaced by a space"),
        ("attr_raw=\"abc\"", "⭐ THE TWO VALUES OF ONE ELEMENT: the CONTENT attribute keeps what the author wrote"),
        ("attr_api=\"\"", "…and `.value` is what the type means. Both, from the same element, in the same report"),
        ("chg_text=\"  foobar  \"", "the same element read as text…"),
        ("chg_email=\"foobar\"", "…and after `type = 'email'`, re-sanitised for the new type"),
    ];
    for (claim, why) in rows {
        assert!(
            got.contains(claim),
            "G_INPUT_VALUE_SANITIZATION: expected {claim:?} in the report.\n  {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  return the raw content attribute (the pre-tick behaviour)
//       -> every row except `num_ok`, `date_ok` and `attr_raw` fails. That is the defect.
// N2  implement the algorithm as one "strip CR/LF then trim" for all types
//       -> `strip_*` (the surrounding space is eaten) and `num_spaced` (which must stay invalid)
//          fail, while url/email pass. The trimming pair is a per-type rule, not a cleanup.
// N3  make `range` valid-or-EMPTY instead of valid-or-default
//       -> `range_junk` reads "" instead of "50": an untouched slider would have no position.
