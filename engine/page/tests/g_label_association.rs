//! **G_LABEL_ASSOCIATION — `control.labels` and `label.control` link a form control to its `<label>`s.**
//!
//! Both were `undefined`. `input.labels` is how every accessibility helper reads the text that NAMES a
//! control (`input.labels[0].textContent`), and `label.control` is the inverse a "click the label to
//! focus the field" handler walks. A control with no `.labels` and a `<label>` with no `.control` leaves
//! both blind. Each claim is a way the missing surface goes RED:
//!
//!   * `label.control` resolves the `for=` target when it is labelable, and the FIRST labelable
//!     DESCENDANT when there is no `for=` (`<label><input></label>`).
//!   * `control.labels` is a NodeList (live-recomputed, static per read) of every `<label>` associated
//!     with it, in tree order — a control can carry more than one label.
//!   * a hidden input is NOT labelable: its `.labels` is `null`, and a `<label for=hidden>` does NOT
//!     claim it (`.control` is null).

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
    <label id="l1" for="email">Email</label>
    <input id="email" type="text">
    <label id="l2" for="email">(again)</label>
    <label id="l3">Wrapped <input id="wrapped" type="checkbox"></label>
    <label id="l4" for="hid">Hidden</label>
    <input id="hid" type="hidden">
    <div id="out">-</div><script>
    var r = [];
    var l1 = document.getElementById('l1'), l3 = document.getElementById('l3');
    var email = document.getElementById('email'), wrapped = document.getElementById('wrapped');
    var hid = document.getElementById('hid');
    r.push('ctrlFor:' + (l1.control === email ? 'email' : String(l1.control)));  // for= target
    r.push('ctrlDesc:' + (l3.control === wrapped ? 'wrapped' : String(l3.control))); // first descendant
    r.push('labelsType:' + (email.labels instanceof NodeList));
    r.push('labelsLen:' + email.labels.length);                 // l1 + l2 both point at email
    r.push('labelsText:' + email.labels[0].textContent.trim()); // in tree order → "Email"
    r.push('labelsItem:' + (email.labels.item(1) ? 'ok' : 'null'));
    r.push('hiddenLabels:' + (hid.labels === null ? 'null' : String(hid.labels))); // hidden input → null
    r.push('hiddenCtrl:' + (document.getElementById('l4').control === null ? 'null' : 'CLAIMED')); // for=hidden claims nothing
    document.getElementById('out').textContent = r.join(' ');
    </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn labels_and_control_link_a_form_field_to_its_label() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://label-assoc.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "ctrlFor:email",     // label.control follows for= to a labelable target
        "ctrlDesc:wrapped",  // no for= → the first labelable descendant
        "labelsType:true",   // control.labels is a NodeList
        "labelsLen:2",       // two labels point at #email
        "labelsText:Email",  // in tree order, the first label's text
        "labelsItem:ok",     // the NodeList has .item and both entries
        "hiddenLabels:null", // a hidden input is not labelable → .labels is null
        "hiddenCtrl:null",   // a <label for=hidden> claims no control
    ] {
        assert!(
            got.contains(claim),
            "G_LABEL_ASSOCIATION: expected {claim} in {got:?}\n  \
             control.labels (a NodeList of associated <label>s) and label.control (the labeled control) \
             must link a form field to its label — their absence blinds every accessibility helper."
        );
    }
}
