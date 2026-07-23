//! **G_FORM_OWNER — `element.form` is the FORM OWNER of a form-associated element.**
//!
//! `input.form` was `undefined`, so every form library that groups controls by their owning form
//! (`input.form === thisForm`) got nothing — including the `form=` REASSOCIATION case a control uses to
//! belong to a `<form>` it does not live inside. Each claim is a way this goes RED:
//!
//!   * a control inside a `<form>` reports that form; a control with `form="id"` reports the referenced
//!     form even from OUTSIDE it.
//!   * `form="id"` pointing at a NON-form element yields NO owner (null) — not the nearest ancestor.
//!   * an orphan control is null; an `<option>` reports its `<select>`'s owner; a `<label>` reports its
//!     labeled control's owner; a non-form-associated element (`<div>`) has no such property (undefined).

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<form id="f1"><input id="inside"><select id="sel"><option id="opt">o</option></select><label id="lab" for="inside">L</label></form>
<form id="f2"></form>
<input id="assoc" form="f2">
<input id="bad" form="notaform">
<span id="notaform"></span>
<input id="orphan">
<div id="plain"></div>
<div id="out">-</div><script>
var r=[];
function fid(el){ return el.form ? el.form.id : String(el.form); }
r.push('inside:'+fid(document.getElementById('inside')));
r.push('assoc:'+fid(document.getElementById('assoc')));       // form= from outside
r.push('badRef:'+fid(document.getElementById('bad')));         // form= → non-form → null
r.push('orphan:'+fid(document.getElementById('orphan')));      // null
r.push('select:'+fid(document.getElementById('sel')));
r.push('option:'+fid(document.getElementById('opt')));         // via its select
r.push('label:'+fid(document.getElementById('lab')));          // via its control
r.push('plain:'+(document.getElementById('plain').form===undefined?'undefined':'DEFINED')); // not form-assoc
document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn element_form_resolves_the_form_owner() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://form-owner.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "inside:f1",   // ancestor form
        "assoc:f2",    // form= reassociation from outside
        "badRef:null", // form= to a non-form → null, not the ancestor
        "orphan:null", // no ancestor, no form= → null
        "select:f1",
        "option:f1",       // option → its select's owner
        "label:f1",        // label → its control's owner
        "plain:undefined", // a <div> is not form-associated
    ] {
        assert!(
            got.contains(claim),
            "G_FORM_OWNER: expected {claim} in {got:?}\n  \
             element.form must resolve the form owner (ancestor <form>, or the form= referenced form, or \
             null) for form-associated elements — its absence blinds every form library."
        );
    }
}
