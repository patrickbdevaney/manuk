//! **G_ELEMENT_INTERNALS — `attachInternals()` returns a real ElementInternals, once per element.**
//!
//! Form-associated custom elements — the Lit/Shoelace-style controls, GitHub's own web components,
//! Salesforce Lightning, any `static formAssociated = true` custom input — call `this.attachInternals()`
//! in their CONSTRUCTOR to get the object that submits their value (`setFormValue`), reports validity
//! (`setValidity`/`checkValidity`), exposes `:state()` custom states, and reflects ARIA. It is not
//! feature-detected, so its absence throws `attachInternals is not a function` out of the constructor
//! and the entire component fails to upgrade — it renders as an empty, dead tag.
//!
//! We do not yet wire internals into the real form-submission pipeline (the follow-on), but the object
//! is REAL, not inert: it retains the form value, the validity flags + message, and the custom state
//! set, so the constructor completes and the retained state is queryable. The gate drives that:
//!
//!   1. `attachInternals()` (on the element prototype) returns an ElementInternals with
//!      setFormValue/setValidity/checkValidity/states, and does not throw.
//!   2. `setFormValue` does not throw; `setValidity({}, '')` → `checkValidity()` true, and
//!      `setValidity({valueMissing:true}, 'Required')` → `checkValidity()` false with that message.
//!   3. `states` is a working set — `add`/`has` drive `:state()` styling.
//!   4. A SECOND `attachInternals()` on the same element throws (spec: once per element).
//!
//! RED: removing the shim drops `defined`, `validity`, `once` — `attachInternals` is not a function and
//! the constructor-equivalent call throws, the exact dead-component failure.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <div id="host"></div>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } }
    };
    try {
      var host = document.getElementById('host');
      var it = host.attachInternals();
      R.push('defined:' + (it && typeof it.setFormValue === 'function' &&
                           typeof it.setValidity === 'function' &&
                           typeof it.checkValidity === 'function' && it.states &&
                           typeof it.states.add === 'function'));

      it.setFormValue('hello', null);
      it.setValidity({}, '');
      var validWhenClean = (it.checkValidity() === true && it.validity.valid === true);
      it.setValidity({ valueMissing: true }, 'Required');
      var invalidWhenFlagged = (it.checkValidity() === false &&
                                it.validity.valueMissing === true &&
                                it.validationMessage === 'Required');
      R.push('validity:' + (validWhenClean && invalidWhenFlagged));

      it.states.add('checked');
      R.push('states:' + (it.states.has('checked') === true));

      // Second attachInternals on the same element must throw.
      var threw = false;
      try { host.attachInternals(); } catch (e) { threw = true; }
      R.push('once:' + threw);

      R.push('ready:true');
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

#[test]
fn attach_internals_returns_real_internals_once() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ei.test/", &fonts, 800.0);
    let root = page.dom().root();

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("defined:true", "`attachInternals()` must return an ElementInternals with setFormValue/setValidity/checkValidity/states — components call it unguarded in the constructor, so its absence throws and the component fails to upgrade"),
        ("validity:true", "`setValidity` must drive `checkValidity()`/`validity`/`validationMessage` — clean flags are valid, a raised flag with a message is invalid with that message"),
        ("states:true", "`states` must be a working set (add/has) — it drives `:state()` styling the component reads back"),
        ("once:true", "a second `attachInternals()` on the same element must throw — the spec allows it at most once"),
        ("ready:true", "the constructor-equivalent sequence must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_ELEMENT_INTERNALS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
