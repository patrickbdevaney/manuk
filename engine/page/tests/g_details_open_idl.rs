//! **G_DETAILS_OPEN_IDL — setting `details.open` from script fires `toggle` and honours the group.**
//!
//! Tick 467 made the SUMMARY-CLICK path fire `toggle` and enforce `<details name>` exclusivity. But a
//! large slice of the web never clicks the summary — it drives the disclosure from STATE: a framework
//! renders `<details open={isExpanded}>`, or a controller writes `el.open = true`. That goes through the
//! IDL setter, not a click, and a plain attribute-reflection flips `open` **silently**:
//!
//!   * a `toggle` listener wired to lazy-load the panel's contents never fires — the section opens empty;
//!   * a named accordion driven by script sits multiply-open, because the exclusivity a click enforces
//!     was on the click path only.
//!
//! This gate drives both effects through `el.open = …` and never touches a summary. It asserts on
//! script-observable state (the `open` attribute + a `toggle` log) — `g_details_accordion` already proves
//! layout follows the attribute, so this need not re-lay-out. Its RED (before the fix) is `bT:` empty —
//! the silent flip fires no toggle.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<details id="a" name="faq" open><summary>A</summary><p>BODY-A</p></details>
<details id="b" name="faq"><summary>B</summary><p>BODY-B</p></details>
<details id="solo"><summary>S</summary><p>BODY-S</p></details>
<div id="out">-</div>
<script>window.__log = [];
  document.getElementById('a').addEventListener('toggle', function () { window.__log.push('a'); });
  document.getElementById('b').addEventListener('toggle', function () { window.__log.push('b'); });
</script>
</body></html>"##;

#[test]
fn setting_details_open_from_script_fires_toggle_and_enforces_the_group() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://open-idl.test/", &fonts, 800.0);

    // Drive #b OPEN purely from script — no click anywhere — then record every observable effect into a
    // single string: which details carry `open`, and what the toggle log holds.
    page.eval_for_test(
        "var A = document.getElementById('a'), B = document.getElementById('b'), \
             S = document.getElementById('solo'); \
         B.open = true;                       /* open the second panel of the faq group from script */ \
         var afterB = 'aOpen:' + A.hasAttribute('open') + ' bOpen:' + B.hasAttribute('open') \
                    + ' log:' + window.__log.join(''); \
         window.__log.length = 0; \
         S.open = true;                       /* an UNNAMED details — not part of any group */ \
         var afterS = 'bStill:' + B.hasAttribute('open') + ' sOpen:' + S.hasAttribute('open'); \
         document.getElementById('out').textContent = afterB + ' | ' + afterS;",
    );
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    // Exclusivity on the script path: opening #b closed its same-name sibling #a.
    assert!(
        got.contains("aOpen:false"),
        "G_DETAILS_OPEN_IDL: script-opening #b left same-name sibling #a open ({got:?}). `<details name>` \
         exclusivity must hold on the IDL setter too, not only on a summary click — a state-driven \
         accordion writing el.open would otherwise show every section at once."
    );
    assert!(
        got.contains("bOpen:true"),
        "G_DETAILS_OPEN_IDL: `el.open = true` did not set #b's open attribute ({got:?})."
    );

    // `toggle` fired on BOTH: #b (opened) and #a (auto-closed by exclusivity).
    assert!(
        got.contains("log:") && got.contains('b') && {
            // The log substring is between "log:" and " |"; both letters must be in it.
            let seg = got.split("log:").nth(1).unwrap_or("");
            seg.contains('b') && seg.contains('a')
        },
        "G_DETAILS_OPEN_IDL: setting #b.open fired the wrong toggle set ({got:?}); expected `toggle` on \
         both #b (opened) and #a (auto-closed). A lazy-load listener wired to `toggle` learns nothing \
         from a silent attribute flip — the panel reveals empty and the collapsed one is never told."
    );

    // Scoping: the unnamed #solo is not a group — opening it must not close the named #b.
    assert!(
        got.contains("bStill:true") && got.contains("sOpen:true"),
        "G_DETAILS_OPEN_IDL: opening the unnamed #solo disturbed the named #b ({got:?}). Exclusivity must \
         ignore a nameless details — it belongs to no group."
    );
}
