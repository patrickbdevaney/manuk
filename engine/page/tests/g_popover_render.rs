//! G_POPOVER_RENDER — the half of the popover API that script cannot see.
//!
//!   1. A **closed** popover renders NOTHING. Without the `[popover]` UA rule the dropdown's items,
//!      the tooltip's copy and the whole menu were laid out and painted into the page in tree order,
//!      before anyone opened them — the same failure as a closed `<dialog>`.
//!   2. An **open** popover joins the **top layer**: it paints above an author `z-index: 50` element
//!      even though it sits earlier in the tree. A menu that renders under the sticky header it
//!      hangs off is not a menu.
//!
//! `data-manuk-popover-open` in the markup is exactly what `showPopover()` sets — `g_popover.rs`
//! proves that half.

use manuk_text::FontContext;

const CLOSED: &str = r#"<!doctype html>
<html><body>
  <div id="menu" popover><p id="item">MENUITEM</p></div>
  <div id="after">after</div>
</body></html>"#;

const OPEN: &str = r#"<!doctype html>
<html><body>
  <div id="menu" popover data-manuk-popover-open><p id="item">MENUITEM</p></div>
  <div id="header" style="position:relative; z-index:50; background:#ff0000;">HEADERTEXT</div>
</body></html>"#;

/// The index of the first `Text` item whose string contains `needle`.
fn text_index(list: &manuk_paint::DisplayList, needle: &str) -> Option<usize> {
    list.items.iter().position(
        |it| matches!(it, manuk_paint::DisplayItem::Text { text, .. } if text.contains(needle)),
    )
}

#[test]
fn closed_popover_paints_nothing_and_an_open_one_tops_the_stack() {
    let fonts = FontContext::new();

    // ── 1. Closed: in the DOM, out of the render tree.
    let page = manuk_page::Page::load(CLOSED, "https://popover.test/", &fonts, 800.0);
    let root = page.dom().root();
    let item = manuk_css::query_selector_all(page.dom(), root, "#item");
    assert!(
        !item.is_empty(),
        "#item must still be IN THE DOM when closed"
    );
    let boxed = page.node_rects().get(&item[0]).copied();
    assert!(
        boxed.is_none_or(|r| r.height == 0.0),
        "G_POPOVER_RENDER: a CLOSED popover's contents got a real box ({boxed:?}) — the menu is \
         painting into the page before anyone opened it"
    );
    assert!(
        text_index(&page.display_list(), "MENUITEM").is_none(),
        "G_POPOVER_RENDER: a CLOSED popover's text reached the display list"
    );

    // ── 2. Open: painted, and painted LAST — above the z-index:50 header.
    let page = manuk_page::Page::load(OPEN, "https://popover.test/", &fonts, 800.0);
    let list = page.display_list();
    let menu = text_index(&list, "MENUITEM").expect(
        "G_POPOVER_RENDER: an OPEN popover must render — its text is missing from the paint",
    );
    let header =
        text_index(&list, "HEADERTEXT").expect("G_POPOVER_RENDER: the header must still render");
    assert!(
        menu > header,
        "G_POPOVER_RENDER: the popover painted at {menu}, BEHIND the z-index:50 header at {header} \
         — a menu that renders under the header it hangs off is not a menu"
    );
}
