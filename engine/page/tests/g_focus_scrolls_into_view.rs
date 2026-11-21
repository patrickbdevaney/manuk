//! G_FOCUS_SCROLLS_INTO_VIEW: focusing an element scrolls it into view inside the scroll container that had
//! clipped it away (HTML focusing steps), centred like Chrome. A 400px listbox of 48 x 36px options,
//! focus option 26: Chrome reads scrollTop 754 (26*36 + 18 - 200). Red with `MANUK_REVEAL_MUTATE=1`
//! (the reveal path disabled) or with the `set_focus` hook removed: scrollTop stays 0.

use manuk_text::FontContext;

#[test]
fn focus_scrolls_option_into_view() {
    let items: String = (0..48)
        .map(|k| format!(r#"<li id=i{k} tabindex=-1 style="height:36px">opt {k}</li>"#))
        .collect();
    let html = format!(
        r#"<!doctype html><body style="margin:0"><ul id=l style="height:400px;overflow:auto;margin:0;padding:0;list-style:none">{items}</ul></body>"#
    );
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(&html, "https://f.test/", &fonts, 800.0);
    let root = page.dom().root();
    let l = manuk_css::query_selector_all(page.dom(), root, "#l")[0];
    let o = manuk_css::query_selector_all(page.dom(), root, "#i26")[0];
    assert_eq!(
        page.scroll_geometry(l).unwrap()[0],
        0.0,
        "VACUOUS: starts scrolled"
    );
    page.set_focus(Some(o), false, &fonts, 800.0);
    let top = page.scroll_geometry(l).unwrap()[0];
    assert!(
        (top - 754.0).abs() <= 36.0,
        "focus() must scroll option 26 into view: scrollTop {top}, want ~754"
    );
}
