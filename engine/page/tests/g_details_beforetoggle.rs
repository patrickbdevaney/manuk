//! **G_DETAILS_BEFORETOGGLE — `<details>` fires `beforetoggle` immediately before `toggle`, both paths.**
//!
//! Ticks 467/468 gave `<details>` its `toggle` event on the summary-CLICK path and the script `el.open`
//! path (with `<details name>` accordion exclusivity). The missing companion is `beforetoggle` — the event
//! the platform fires JUST BEFORE the panel's contents render, which a page listens for to lazy-load the
//! section's DOM before it becomes visible (the same role `beforetoggle` plays for popover, already built).
//! Without it, a handler wired to `beforetoggle` never runs and the section reveals its skeleton.
//!
//! This gate drives BOTH actuation paths — a real summary click and a scripted `el.open = true` (including
//! the accordion auto-close of a same-name sibling) — and asserts that on every state change `beforetoggle`
//! fires exactly once and STRICTLY before that element's `toggle`. `beforetoggle` on `<details>` is
//! non-cancelable (unlike popover's), and — matching this engine's existing `toggle` — stateless; its value
//! is the fire-before-render ordering. RED (before the fix): the `bt:` entries are absent, so every element
//! shows a `t:` with no preceding `bt:`.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<details id="a" name="faq" open><summary id="sa">A</summary><p>BODY-A</p></details>
<details id="b" name="faq"><summary id="sb">B</summary><p>BODY-B</p></details>
<div id="out">-</div>
<script>window.__log = [];
  ['a','b'].forEach(function (id) {
    var el = document.getElementById(id);
    el.addEventListener('beforetoggle', function () { window.__log.push('bt:' + id); });
    el.addEventListener('toggle',       function () { window.__log.push('t:'  + id); });
  });
</script>
</body></html>"##;

fn click(page: &mut manuk_page::Page, fonts: &FontContext, sel: &str) {
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, sel)[0];
    page.dispatch_click(n, fonts, 800.0);
}

#[test]
fn details_fires_beforetoggle_before_toggle_on_both_paths() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://beforetoggle.test/", &fonts, 800.0);

    // ── CLICK PATH: click #b's summary. #b opens (closed→open) and, by `name="faq"` exclusivity, #a
    // auto-closes (open→closed). Each state change must fire `beforetoggle` then `toggle`.
    click(&mut page, &fonts, "#sb");
    let read_out = |page: &mut manuk_page::Page| -> String {
        page.eval_for_test("document.getElementById('out').textContent = window.__log.join(' ');");
        let root = page.dom().root();
        let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
        page.dom().text_content(out)
    };
    let click_log = read_out(&mut page);
    println!("BEFORETOGGLE click log: {click_log}");

    // #b opened: `bt:b` strictly precedes `t:b`.
    let bt_b = click_log.find("bt:b");
    let t_b = click_log.find("t:b");
    assert!(
        bt_b.is_some() && t_b.is_some() && bt_b < t_b,
        "G_DETAILS_BEFORETOGGLE: clicking #b must fire `beforetoggle` on #b BEFORE its `toggle` \
         (got {click_log:?}). A lazy-load hook wired to beforetoggle prepares the panel before it renders."
    );
    // #a auto-closed by the accordion: it too gets `beforetoggle` before `toggle`.
    let bt_a = click_log.find("bt:a");
    let t_a = click_log.find("t:a");
    assert!(
        bt_a.is_some() && t_a.is_some() && bt_a < t_a,
        "G_DETAILS_BEFORETOGGLE: the accordion auto-close of #a must fire `beforetoggle` before `toggle` \
         (got {click_log:?})."
    );

    // ── SCRIPT PATH: reset the log, then re-open #a via `el.open = true`. #a opens and #b auto-closes;
    // both go through the IDL reflection setter, which must fire the same pair.
    page.eval_for_test("window.__log.length = 0; document.getElementById('a').open = true;");
    let script_log = read_out(&mut page);
    println!("BEFORETOGGLE script log: {script_log}");

    let s_bt_a = script_log.find("bt:a");
    let s_t_a = script_log.find("t:a");
    assert!(
        s_bt_a.is_some() && s_t_a.is_some() && s_bt_a < s_t_a,
        "G_DETAILS_BEFORETOGGLE: `el.open = true` must fire `beforetoggle` before `toggle` on the script \
         path too (got {script_log:?}) — the click path and the IDL setter must behave identically."
    );
    let s_bt_b = script_log.find("bt:b");
    let s_t_b = script_log.find("t:b");
    assert!(
        s_bt_b.is_some() && s_t_b.is_some() && s_bt_b < s_t_b,
        "G_DETAILS_BEFORETOGGLE: the scripted accordion auto-close of #b must fire `beforetoggle` before \
         `toggle` (got {script_log:?})."
    );
}
