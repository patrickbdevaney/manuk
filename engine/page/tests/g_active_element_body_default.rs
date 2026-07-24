//! **G_ACTIVE_ELEMENT_BODY_DEFAULT — `document.activeElement` defaults to `<body>`, never `null`.**
//!
//! When no element is focused, `document.activeElement` is the **body element** — that is what Chrome
//! returns and what the spec requires for a fully-loaded HTML document. It was returning **`null`**, and a
//! null `activeElement` is a crash waiting to happen: `document.activeElement.blur()` (the idiomatic
//! "dismiss whatever is focused"), `document.activeElement.tagName`, and `document.activeElement === el`
//! guards all assume a real element. Widgets, focus-trap libraries and keyboard handlers read it on every
//! interaction.
//!
//! Two things to prove, and the second guards against "fixed it to a constant":
//! 1. with nothing focused, `activeElement` is the `<body>` (not null);
//! 2. focusing an element still updates `activeElement` to THAT element — the body default is a fallback,
//!    not a replacement for real focus tracking.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<input id="f" type="text">
<div id="out">-</div>
<script>
  var R = [];
  // (1) nothing focused yet → activeElement is <body>, and crucially NOT null.
  R.push('isNull:' + (document.activeElement === null));
  R.push('tag:' + (document.activeElement ? document.activeElement.tagName : 'NULL'));
  R.push('isBody:' + (document.activeElement === document.body));
  // (2) focusing an element moves activeElement to it — real focus tracking still wins.
  var f = document.getElementById('f');
  f.focus();
  R.push('focused:' + (document.activeElement === f ? 'INPUT' : (document.activeElement ? document.activeElement.tagName : 'NULL')));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn a_active_element_defaults_to_body_and_still_tracks_focus() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ae.test/", &fonts, 400.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "isNull:false",
            "activeElement must NOT be null with nothing focused — a null crashes the ubiquitous \
             `document.activeElement.blur()` / `.tagName` idioms",
        ),
        ("tag:BODY", "…it is the BODY element, the spec default for a loaded document (as Chrome returns)"),
        ("isBody:true", "and it is literally `document.body`, not merely some element named body"),
        (
            "focused:INPUT",
            "focusing an element still moves activeElement to THAT element — the body default is a \
             fallback, it does not replace real focus tracking",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_ACTIVE_ELEMENT_BODY_DEFAULT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
