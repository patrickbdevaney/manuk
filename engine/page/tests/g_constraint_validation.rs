//! **G_CONSTRAINT_VALIDATION — the form-validity API (`checkValidity` / `validity` / the `invalid`
//! event) must exist and be correct.**
//!
//! Every validation path on the web reads this surface: the browser's own native form validation, and
//! every library that reimplements it (React Hook Form, Formik, VeeValidate) reads `el.validity.
//! valueMissing`, calls `form.checkValidity()`, and listens for the `invalid` event. It was ALL absent —
//! `input.checkValidity` was `undefined`, so `if (!input.checkValidity())` is a `TypeError` that takes
//! the submit handler with it and the form silently cannot submit (the exact "a missing method is a
//! thrown exception, and its blast radius is whatever was running" failure `G_GLOBALS` exists for,
//! specialised to forms).
//!
//! This gate asserts the observable contract: the eight `ValidityState` flags compute from the reflected
//! content attributes + the current value, `checkValidity()` returns `validity.valid` and fires a
//! cancelable `invalid` event when it fails, a form aggregates its controls, custom validity overrides,
//! and a barred control (`type=hidden`) does not validate. Every claim is RED against the absent API
//! (a `TypeError` before the first assert), which is what makes it a ratchet tooth.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div>
  <input id="a" required>
  <input id="b" required value="x">
  <input id="c" type="email" value="nope">
  <input id="d" type="email" value="a@b.co">
  <input id="e" pattern="[0-9]{3}" value="ab">
  <input id="f" pattern="[0-9]{3}" value="123">
  <input id="g" type="hidden" required>
  <input id="h" type="number" value="5" min="10">
  <form id="frm"><input required></form>
  <script>
    var $ = function(id){ return document.getElementById(id); };
    var r = [];

    // 1. required + empty → valueMissing, and checkValidity() is false.
    r.push('reqEmpty:' + ($('a').validity.valueMissing === true && $('a').checkValidity() === false));
    // 2. required + a value → not missing, valid.
    r.push('reqFilled:' + ($('b').validity.valueMissing === false && $('b').validity.valid === true));
    // 3. type=email with a bad value → typeMismatch; a good address → valid.
    r.push('emailBad:' + ($('c').validity.typeMismatch === true));
    r.push('emailOk:' + ($('d').validity.valid === true));
    // 4. pattern mismatch vs match.
    r.push('patBad:' + ($('e').validity.patternMismatch === true));
    r.push('patOk:' + ($('f').validity.valid === true));
    // 5. a barred control (type=hidden) does not validate: willValidate false, checkValidity true.
    r.push('barred:' + ($('g').willValidate === false && $('g').checkValidity() === true));
    // 6. numeric range underflow.
    r.push('range:' + ($('h').validity.rangeUnderflow === true));

    // 7. setCustomValidity forces invalid, and clearing it restores valid.
    $('b').setCustomValidity('nope');
    var afterSet = ($('b').validity.customError === true && $('b').checkValidity() === false);
    $('b').setCustomValidity('');
    var afterClear = ($('b').validity.valid === true);
    r.push('custom:' + (afterSet && afterClear));

    // 8. checkValidity() fires a cancelable `invalid` event on a failing control.
    var fired = false;
    $('a').addEventListener('invalid', function(){ fired = true; });
    $('a').checkValidity();
    r.push('invalidEvent:' + fired);

    // 9. form.checkValidity() aggregates: false while a required child is empty.
    r.push('formAgg:' + ($('frm').checkValidity() === false));

    $('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn constraint_validation_computes_and_reports_validity() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://forms.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "reqEmpty:true",     // required + empty → valueMissing
        "reqFilled:true",    // required + value → valid
        "emailBad:true",     // type=email typeMismatch
        "emailOk:true",      // valid email
        "patBad:true",       // pattern mismatch
        "patOk:true",        // pattern match
        "barred:true",       // type=hidden does not validate
        "range:true",        // numeric rangeUnderflow
        "custom:true",       // setCustomValidity forces + clears
        "invalidEvent:true", // checkValidity fires the cancelable `invalid` event
        "formAgg:true",      // form aggregates its controls
    ] {
        assert!(
            got.contains(claim),
            "G_CONSTRAINT_VALIDATION: expected {claim} in {got:?}\n  \
             The form-validity API must exist and compute correctly — a missing `checkValidity` is a \
             TypeError that takes the submit handler with it, and the form silently cannot submit."
        );
    }
}
