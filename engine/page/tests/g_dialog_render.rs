//! G_DIALOG_RENDER — the half of `<dialog>` that script cannot see.
//!
//! Two claims, both invisible to a JS-only conformance check and both wrong before this tick:
//!
//!   1. A **closed** dialog renders NOTHING. With no UA `display:none` rule a `<dialog>` is just a
//!      block, so the modal's contents — "DELETE EVERYTHING?", the cookie-consent copy, the command
//!      palette's whole item list — were painted into the middle of the page before anyone opened it.
//!   2. An **open modal** joins the top layer: it paints above an author `z-index: 50` overlay even
//!      though it sits earlier in the tree and is not positioned. That promotion is what makes a
//!      modal modal; without it `showModal()` renders behind the sticky header it must cover.
//!
//! `open data-manuk-modal` in the markup is exactly the attribute pair `showModal()` sets — see
//! `g_dialog.rs`, which proves that half.

use manuk_text::FontContext;

const CLOSED: &str = r#"<!doctype html>
<html><body>
  <dialog id="dlg"><p id="secret">DELETE EVERYTHING?</p></dialog>
  <div id="after">after</div>
</body></html>"#;

const OPEN_MODAL: &str = r#"<!doctype html>
<html><body>
  <dialog id="dlg" open data-manuk-modal><p id="secret">MODALTEXT</p></dialog>
  <div id="overlay" style="position:relative; z-index:50; background:#ff0000;">OVERLAYTEXT</div>
</body></html>"#;

/// The index of the first `Text` item whose string contains `needle`.
fn text_index(list: &manuk_paint::DisplayList, needle: &str) -> Option<usize> {
    list.items.iter().position(
        |it| matches!(it, manuk_paint::DisplayItem::Text { text, .. } if text.contains(needle)),
    )
}

#[test]
fn closed_dialog_paints_nothing_and_an_open_modal_tops_the_stack() {
    let fonts = FontContext::new();

    // ── 1. Closed: in the DOM, out of the render tree.
    let page = manuk_page::Page::load(CLOSED, "https://dialog.test/", &fonts, 800.0);
    let root = page.dom().root();
    let secret = manuk_css::query_selector_all(page.dom(), root, "#secret");
    assert!(
        !secret.is_empty(),
        "#secret must still be IN THE DOM when closed"
    );
    let rects = page.node_rects();
    let boxed = rects.get(&secret[0]).copied();
    assert!(
        boxed.is_none_or(|r| r.height == 0.0),
        "G_DIALOG_RENDER: a CLOSED dialog's contents got a real box ({boxed:?}) — the modal is \
         painting into the page before anyone opened it"
    );
    assert!(
        text_index(&page.display_list(), "DELETE EVERYTHING").is_none(),
        "G_DIALOG_RENDER: a CLOSED dialog's text reached the display list"
    );

    // ── 2. Open + modal: painted, and painted LAST — above the z-index:50 overlay.
    let page = manuk_page::Page::load(OPEN_MODAL, "https://dialog.test/", &fonts, 800.0);
    let list = page.display_list();
    let modal = text_index(&list, "MODALTEXT")
        .expect("G_DIALOG_RENDER: an OPEN dialog must render — its text is missing from the paint");
    let overlay =
        text_index(&list, "OVERLAYTEXT").expect("G_DIALOG_RENDER: the overlay must still render");
    assert!(
        modal > overlay,
        "G_DIALOG_RENDER: the modal painted at {modal}, BEHIND the z-index:50 overlay at {overlay} \
         — a modal that renders under the page's own overlay is not modal"
    );
}
