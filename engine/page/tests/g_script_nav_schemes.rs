//! **G_SCRIPT_NAV_SCHEMES — SEC.DID.12 (SECURITY.md P14): a page script may send the tab, or a new window,
//! only to http(s) (or `file:` from a local file).**
//!
//! Before this gate `location.href = "file:///etc/passwd"` on an https page reported a navigation to the
//! host, `window.open("data:text/html,...")` / `window.open("file:///...")` / `window.open("javascript:...")`
//! each queued a new tab, and `location.href = "javascript:..."` was mangled into a bogus relative path.
//! Chrome blocks top-level `data:`/`file:` navigation from a web page; so do we now.
//!
//! The control arms (https absolute, relative, and a `file:` page opening `file:`) are what stop a constant
//! `false` from satisfying the refusals.
//!
//! Mutation record (chrome_ver=145.0.7632.116): `g.__navAllowed` in `js/src/dom_bindings.rs` with
//! `return true;` prepended turns this gate RED (every refusal row); `return false;` prepended turns the
//! control rows RED.
#![cfg(feature = "spidermonkey")]

use manuk_text::FontContext;

fn run(base: &str, script: &str, fonts: &FontContext, rt: &tokio::runtime::Runtime) -> String {
    let _ = manuk_js::take_window_opens();
    let html = format!("<!doctype html><body><script>{script}</script></body>");
    let page = rt.block_on(async { manuk_page::Page::load_async(&html, base, fonts, 800.0).await });
    let nav = page.take_script_navigation().unwrap_or_else(|| "none".into());
    let opens: Vec<String> = manuk_js::take_window_opens().into_iter().map(|(_, u)| u).collect();
    format!("nav={nav} opens={opens:?}")
}

#[test]
fn a_page_script_can_only_navigate_or_open_to_web_schemes() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let web = "https://stub.test/a/b.html";
    let refused = "nav=none opens=[]";

    // ── CONTROLS: these must still work, or the refusals below are vacuous.
    assert_eq!(run(web, r#"location.href = "https://other.test/x";"#, &fonts, &rt), r#"nav=https://other.test/x opens=[]"#);
    assert_eq!(run(web, r#"location.assign("/rel");"#, &fonts, &rt), r#"nav=https://stub.test/rel opens=[]"#);
    assert_eq!(run(web, r#"window.open("https://ok.test/");"#, &fonts, &rt), r#"nav=none opens=["https://ok.test/"]"#);
    assert_eq!(run(web, r#"window.open("rel.html");"#, &fonts, &rt), r#"nav=none opens=["rel.html"]"#);
    assert_eq!(
        run("file:///tmp/a.html", r#"window.open("file:///tmp/b.html");"#, &fonts, &rt),
        r#"nav=none opens=["file:///tmp/b.html"]"#
    );

    // ── REFUSALS.
    for s in [
        r#"location.href = "javascript:void(0)";"#,
        r#"location.href = "data:text/html,<b>x</b>";"#,
        r#"location.assign("data:text/html;base64,PGI+eDwvYj4=");"#,
        r#"location.replace("blob:https://stub.test/1");"#,
        r#"location.href = "file:///etc/passwd";"#,
        r#"location.href = " \tJaVaScRiPt:alert(1)";"#,
        r#"window.open("javascript:alert(1)");"#,
        r#"window.open("data:text/html,<b>x</b>");"#,
        r#"window.open("file:///etc/passwd");"#,
        r#"window.open("DATA:text/html,x");"#,
    ] {
        assert_eq!(run(web, s, &fonts, &rt), refused, "G_SCRIPT_NAV_SCHEMES: `{s}` must go nowhere");
    }
}
