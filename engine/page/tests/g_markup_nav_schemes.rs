//! **G_MARKUP_NAV_SCHEMES — SEC.DID.12.b (SECURITY.md P16): a page's own markup (`<meta refresh>`, and via
//! the shared predicate anchors and forms) may send the tab only to http(s), or `file:` from a local file.**
//!
//! The page-side twin of `g_script_nav_schemes`. Controls (https absolute, relative, `file:` from a file
//! page) stop a constant `false` passing; refusals stop a constant `true` passing.
//!
//! Mutation record (chrome_ver=145.0.7632.116): `top_nav_allowed` in `engine/page/src/lib.rs` returning
//! `true` turns the refusal rows RED; returning `false` turns the control rows RED. The anchor
//! (`shell/src/gui.rs` unit test) and form (`agent/src/forms.rs` unit test) rows ride the same predicate.
use manuk_text::FontContext;

fn refresh(base: &str, content: &str, fonts: &FontContext, rt: &tokio::runtime::Runtime) -> String {
    let html = format!(
        r#"<!doctype html><html><head><meta http-equiv="refresh" content="{content}"></head><body>x</body></html>"#
    );
    let page = rt.block_on(async { manuk_page::Page::load_async(&html, base, fonts, 800.0).await });
    page.meta_refresh()
        .map(|(_, u)| u)
        .unwrap_or_else(|| "none".into())
}

#[test]
fn markup_navigation_is_web_schemes_only() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let web = "https://stub.test/a/b.html";
    // controls
    assert_eq!(
        refresh(web, "0;url=https://o.test/x", &fonts, &rt),
        "https://o.test/x"
    );
    assert_eq!(
        refresh(web, "0;url=/rel", &fonts, &rt),
        "https://stub.test/rel"
    );
    assert_eq!(
        refresh("file:///tmp/a.html", "0;url=b.html", &fonts, &rt),
        "file:///tmp/b.html"
    );
    // refusals
    for c in [
        "0;url=data:text/html,<b>x</b>",
        "0;url=javascript:alert(1)",
        "0;url=file:///etc/passwd",
        "0;url=blob:https://stub.test/1",
        "0;url=DATA:text/html,x",
    ] {
        assert_eq!(
            refresh(web, c, &fonts, &rt),
            "none",
            "G_MARKUP_NAV_SCHEMES: refresh `{c}` must go nowhere"
        );
    }
    // the predicate itself
    assert!(manuk_page::top_nav_allowed(web, "https://x.test/"));
    assert!(manuk_page::top_nav_allowed(web, "http://x.test/"));
    assert!(!manuk_page::top_nav_allowed(web, "data:text/html,x"));
    assert!(!manuk_page::top_nav_allowed(web, "file:///etc/passwd"));
    assert!(manuk_page::top_nav_allowed(
        "file:///a.html",
        "file:///b.html"
    ));
}
