//! **G_INERT_FOCUS — an `inert` element cannot receive focus.**
//!
//! Tick 450 made `inert` transparent to the agent's hit-test and reflectable. This closes the sibling
//! interaction the same modal-backdrop story needs: an `inert` element (and everything in its subtree)
//! is removed from the tab order and `el.focus()` on it is a no-op — the page behind an open
//! `<dialog>.showModal()` must not be tabbable, or keyboard focus escapes the modal into neutralised UI.
//!
//! `Page::set_focus` is the single sink every focus path funnels through (the shell's Tab handling, the
//! agent's focus grounding, and the JS `el.focus()` request queue). It now refuses a focus request whose
//! target is inside an inert subtree, before touching the DOM state. Each claim is a way this goes RED:
//!
//!   * focusing a control inside an `inert` container returns `false` and sets no `:focus` styling.
//!   * a control OUTSIDE the inert container still focuses (returns `true`, `:focus` applies) — the
//!     refusal is scoped to the inert subtree, not a blanket veto.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  input        { box-sizing: border-box; width: 100px; }
  #live:focus  { width: 300px; }
  #trap:focus  { width: 300px; }
</style></head><body style="margin:0">
<input id="live">
<div id="bg" inert>
  <input id="trap">
</div>
</body></html>"##;

fn node_of(page: &manuk_page::Page, sel: &str) -> manuk_dom::NodeId {
    let root = page.dom().root();
    manuk_css::query_selector_all(page.dom(), root, sel)[0]
}

fn width_of(page: &manuk_page::Page, sel: &str) -> f32 {
    let n = node_of(page, sel);
    page.node_rects().get(&n).map(|r| r.width).unwrap_or(0.0)
}

#[test]
fn inert_element_cannot_receive_focus() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://inert-focus.test/", &fonts, 800.0);

    // Baseline: neither input is focused, both at their base 100px, or every claim below is vacuous.
    assert!(
        (width_of(&page, "#live") - 100.0).abs() < 0.5 && (width_of(&page, "#trap") - 100.0).abs() < 0.5,
        "G_INERT_FOCUS: unfocused baseline must be 100px / 100px — got {} / {}. The fixture is not \
         measuring what it claims.",
        width_of(&page, "#live"),
        width_of(&page, "#trap")
    );

    // A control OUTSIDE the inert container focuses normally.
    let live = node_of(&page, "#live");
    assert!(
        page.set_focus(Some(live), true, &fonts, 800.0),
        "G_INERT_FOCUS: focusing a normal (non-inert) control must succeed — the fix must not veto all \
         focus."
    );
    assert!(
        (width_of(&page, "#live") - 300.0).abs() < 0.5,
        "G_INERT_FOCUS: `#live:focus` must apply to the non-inert control — got {}.",
        width_of(&page, "#live")
    );

    // The control INSIDE the inert container must refuse focus.
    let trap = node_of(&page, "#trap");
    let took = page.set_focus(Some(trap), true, &fonts, 800.0);
    assert!(
        !took,
        "G_INERT_FOCUS: focusing a control inside an `inert` subtree must be refused (return false) — \
         it returned true, so keyboard focus escapes an open modal into the neutralised page behind it."
    );
    assert!(
        (width_of(&page, "#trap") - 100.0).abs() < 0.5,
        "G_INERT_FOCUS: `#trap:focus` must NOT apply — got width {}. Focus reached an inert control and \
         drove the cascade, which is the exact escape this gate exists to prevent.",
        width_of(&page, "#trap")
    );
}
