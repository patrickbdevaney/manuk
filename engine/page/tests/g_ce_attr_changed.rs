//! **G_CE_ATTR_CHANGED — `attributeChangedCallback` fires on a LIVE `setAttribute`, not just at upgrade.**
//!
//! Custom elements upgraded and fired `attributeChangedCallback` for the observed attributes present at
//! boot, but a LATER script mutation — the reactive-attribute idiom every design-system web component is
//! built on —
//!
//! ```js
//! toggle.setAttribute('checked', '');   // <my-toggle> flips to the checked look
//! toggle.removeAttribute('checked');    // …and back
//! ```
//!
//! never reached the element: `setAttribute`/`removeAttribute`/`toggleAttribute` wrote the attribute into
//! the DOM and did NOT invoke `attributeChangedCallback`, so the component's rendered state froze at
//! whatever it was at boot. The MutationObserver feed could not stand in — it is delivered async (a
//! microtask), and a custom-element reaction is synchronous (the component has re-rendered by the next
//! line of script).
//!
//! ## Each claim, and how it goes RED
//!
//!   * `boot` — the callback still fires for the attribute present at upgrade (regression guard).
//!   * `set` — `setAttribute('label','B')` fires `attributeChangedCallback('label','A','B')` SYNCHRONOUSLY.
//!     RED: remove the `wrapAttrMutator('setAttribute', …)` wrap and the live set is silent (`label` stays A).
//!   * `remove` — `removeAttribute('label')` fires with newValue `null`.
//!   * `toggle` — `toggleAttribute('label')` (re-adding) fires with oldValue `null`.
//!   * `unobserved` — mutating a NON-observed attribute does NOT fire the callback (no over-firing).

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<my-toggle id="t" label="A"></my-toggle>
<div id="out">-</div><script>
var log = [];
class MyToggle extends HTMLElement {
  static get observedAttributes() { return ['label']; }
  attributeChangedCallback(name, oldV, newV) {
    log.push(name + ':' + (oldV === null ? 'null' : oldV) + '->' + (newV === null ? 'null' : newV));
  }
}
customElements.define('my-toggle', MyToggle);

var t = document.getElementById('t');
var bootFired = log.length;                 // the upgrade already fired label:null->A

t.setAttribute('label', 'B');               // LIVE set — must fire synchronously
var afterSet = log.length;

t.removeAttribute('label');                 // fires label:B->null
t.toggleAttribute('label');                 // re-adds -> fires label:null->''

t.setAttribute('data-x', 'y');              // NOT observed — must NOT fire

var r = [];
r.push('boot:' + (bootFired >= 1 ? 'yes' : 'no'));
r.push('set:' + (afterSet > bootFired ? 'yes' : 'no'));
r.push('log:' + log.join('|'));
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn attribute_changed_callback_fires_on_a_live_set_attribute() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ce-attr.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CE ATTR RESULT: {got}");

    for claim in [
        "boot:yes", // the upgrade-time callback still fires (regression guard)
        "set:yes",  // setAttribute on a live upgraded element fires synchronously — the whole bug
        // the exact reaction sequence: boot, live set, remove (->null), toggle re-add (null->''),
        // and NO entry for the unobserved data-x
        "log:label:null->A|label:A->B|label:B->null|label:null->",
    ] {
        assert!(
            got.contains(claim),
            "G_CE_ATTR_CHANGED: expected {claim} in {got:?}\n  \
             attributeChangedCallback must fire SYNCHRONOUSLY on setAttribute/removeAttribute/\
             toggleAttribute of an OBSERVED attribute on an upgraded custom element — not only at \
             upgrade, and never for an unobserved attribute."
        );
    }
}
