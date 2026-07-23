//! **G_USER_SELECT — `getComputedStyle(el).userSelect` reflects the cascaded `user-select`.**
//!
//! `user-select` is one of the most ubiquitous UI properties on the web: toolbars, buttons,
//! drag-handles, tab strips and code-copy widgets all set `user-select: none` so a stray
//! double-click-drag on the chrome does not select label text; rich editors set `user-select: all`
//! on atomic tokens. Feature-detection and layout code read the value back through the CSSOM
//! (`getComputedStyle(el).userSelect` / `.webkitUserSelect` / `.getPropertyValue('user-select')`)
//! to decide whether to run their own selection-suppression fallback.
//!
//! Before this, the property did not exist in the engine at all: Stylo's servo build gates
//! `user-select` behind its shared `layout.unimplemented` pref (off by default), so it was dropped
//! at parse and never reached a computed value. `getComputedStyle(el).userSelect` was `undefined`.
//!
//! The fix flips that pref in `cascade_via_stylo` (a sanctioned Option-1 pref flip, like
//! `layout.grid.enabled`), maps the computed `UserSelect` keyword onto `ComputedStyle.user_select`,
//! and serializes it (plus the `webkitUserSelect` alias Chrome also exposes) in `getComputedStyle`.
//!
//! Proven RED: revert the `stylo_map.rs` mapping (or the pref flip) and every element reads `auto`,
//! so `none`/`text`/`all`/`prefix-alias`/`get-none` all fail — the value is no longer coming from
//! the stylesheet. The `getPropertyValue` reads go from the real keyword back to the initial value.
//!
//! (Scope, stated honestly: this resolves the COMPUTED VALUE the CSSOM reports. The geometry of a
//! user mouse-drag selection honouring `user-select` is a layout/hit-test concern the engine does
//! not model — the same boundary the `Selection` shim documents.)

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  .txt  { user-select: text; }
  #none { user-select: none; }
</style></head><body>
<div id="none">no-select</div>
<div id="all" style="-webkit-user-select: all">all-select</div>
<div id="txt" class="txt">text-select</div>
<div id="def">default</div>
<div id="out">-</div>
<script>
var r = [];
try {
  var g = function (id) { return getComputedStyle(document.getElementById(id)); };
  r.push('none:' + g('none').userSelect);
  r.push('all:' + g('all').userSelect);
  r.push('all-webkit:' + g('all').webkitUserSelect);
  r.push('txt:' + g('txt').userSelect);
  r.push('def:' + g('def').userSelect);
  // getPropertyValue routes both the canonical name and the -webkit- alias to the same value.
  r.push('get-none:' + g('none').getPropertyValue('user-select'));
  r.push('get-all-alias:' + g('all').getPropertyValue('-webkit-user-select'));
} catch (e) {
  r.push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn user_select_computed_value_reflects_the_cascade() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://userselect.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("USER-SELECT RESULT: {got}");

    for claim in [
        "none:none",         // inline `user-select: none` — the toolbar/button case
        "all:all",           // `-webkit-user-select: all` prefix aliases onto the longhand
        "all-webkit:all",    // Chrome also exposes it as `webkitUserSelect`
        "txt:text",          // stylesheet rule `.txt { user-select: text }`
        "def:auto",          // the initial value, unset
        "get-none:none",     // getPropertyValue('user-select') serves the keyword
        "get-all-alias:all", // getPropertyValue('-webkit-user-select') resolves to the same longhand
    ] {
        assert!(
            got.contains(claim),
            "G_USER_SELECT: expected `{claim}`\n  got: {got}\n\n  \
             `user-select` must cascade and reach `getComputedStyle`. If every element reads \
             `auto`, the Stylo `layout.unimplemented` pref flip or the `stylo_map` keyword mapping \
             was lost — the value is no longer coming from the stylesheet."
        );
    }
}
