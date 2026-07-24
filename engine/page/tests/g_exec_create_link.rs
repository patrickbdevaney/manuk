//! **G_EXEC_CREATE_LINK — `execCommand('createLink', false, url)` wraps the selection in `<a href="url">`,
//! firing `beforeinput`→`input` (`inputType:'insertLink'`, `data:url`), vetoable.**
//!
//! Brick 14 of the contenteditable EDITING subsystem — an editor's "add link" button. Reuses the t481
//! `__wrapSelectionFormat` helper (generalised to set attributes + carry event `data`), so the DOM result is
//! the same unambiguous selection-wrap, now with an `href`. Zero new dep.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `html=see <a href="https://ex.com/">this</a> now` — selecting "this" and running createLink wraps it
//!     in an anchor with the URL. RED: drop the `createlink` branch → returns false, no `<a>`,
//!     `html=see this now`.
//!   * `evs=bi:insertLink|in:insertLink` — fires beforeinput then input with the link inputType.
//!   * `data=https://ex.com/` — the beforeinput carries the URL as `.data`.
//!   * `supported=true` — `queryCommandSupported('createLink')` reports true.
//!   * `veto=vetoed` — an editable whose `beforeinput` handler `preventDefault()`s gets NO link.

use manuk_page::Page;
use manuk_text::FontContext;

const W: f32 = 800.0;

fn node(p: &Page, sel: &str) -> manuk_dom::NodeId {
    let root = p.dom().root();
    manuk_css::query_selector_all(p.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"))
}

#[test]
fn exec_command_create_link_wraps_the_selection_in_an_anchor() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<div id="ed" contenteditable="true">see this now</div>
<div id="veto" contenteditable="true">keep me</div>
<div id="log"></div>
<script>
  var evs = [], seenData = '';
  var ed = document.getElementById('ed');
  ed.addEventListener('beforeinput', function (e) { evs.push('bi:' + e.inputType); seenData = e.data; });
  ed.addEventListener('input',       function (e) { evs.push('in:' + e.inputType); });
  document.getElementById('veto').addEventListener('beforeinput', function (e) { e.preventDefault(); });
  window.__selThis = function () {
    // Select "this" (offset 4..8 of "see this now").
    var t = ed.firstChild;
    window.getSelection().setBaseAndExtent(t, 4, t, 8);
  };
  window.__selAll = function (id) {
    window.getSelection().selectAllChildren(document.getElementById(id));
  };
  window.__report = function () {
    document.getElementById('log').textContent =
      'html=' + ed.innerHTML +
      ' evs=' + evs.join('|') +
      ' data=' + seenData +
      ' supported=' + document.queryCommandSupported('createLink') +
      ' veto=' + (document.getElementById('veto').innerHTML === 'keep me' ? 'vetoed' : document.getElementById('veto').innerHTML);
  };
</script></body>"#,
        "https://createlink.test/",
        &fonts,
        W,
    );

    p.eval_for_test(
        "window.__selThis(); document.execCommand('createLink', false, 'https://ex.com/');",
    );
    p.eval_for_test(
        "window.__selAll('veto'); document.execCommand('createLink', false, 'https://ex.com/');",
    );
    p.eval_for_test("window.__report();");

    let out = p.dom().text_content(node(&p, "#log"));
    println!("EXEC-CREATE-LINK RESULT: {out}");

    for claim in [
        r#"html=see <a href="https://ex.com/">this</a> now"#,
        "evs=bi:insertLink|in:insertLink",
        "data=https://ex.com/",
        "supported=true",
        "veto=vetoed",
    ] {
        assert!(
            out.contains(claim),
            "G_EXEC_CREATE_LINK: expected `{claim}` in {out:?}\n  \
             execCommand('createLink', false, url) must wrap the selection in <a href=url>, fire \
             beforeinput/input (inputType:insertLink, data:url), report queryCommandSupported true, and \
             honour a beforeinput veto."
        );
    }
}
