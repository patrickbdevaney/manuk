//! **G_POINTER_EVENTS — `pointer-events: none` is transparent to hit-testing, and getComputedStyle reads it.**
//!
//! `pointer-events` was dropped by the cascade entirely: `getComputedStyle(el).pointerEvents` was
//! `undefined`, and — the real defect — `document.elementFromPoint(x, y)` returned the topmost box even
//! when that box was `pointer-events: none`. So a full-bleed decorative overlay (the canonical use: a
//! gradient scrim, a `::before` sheen, a drag-ghost) *stole every click meant for the content beneath
//! it*. This is not cosmetic — it is the agentic actuation surface (component #2): an agent resolving a
//! click target via `elementFromPoint` hit the transparent overlay, dispatched the click there, and the
//! button underneath never fired. The fix bridges the (inherited) computed value from Stylo and drops any
//! `none` node out of the `elementFromPoint` candidate set. Each claim is a way this goes RED:
//!
//!   * `elementFromPoint` over a `pointer-events:none` overlay returns the element BEHIND it, not the overlay.
//!   * an overlay WITHOUT `pointer-events:none` still wins the hit (no over-correction).
//!   * `getComputedStyle(el).pointerEvents` resolves `"none"` / the initial `"auto"`, never `undefined`.
//!   * `getPropertyValue('pointer-events')` (kebab accessor) maps to the same value.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<div id="under" style="position:absolute;left:0;top:0;width:200px;height:200px">under</div>
<div id="ghost" style="position:absolute;left:0;top:0;width:200px;height:200px;pointer-events:none">ghost</div>
<div id="under2" style="position:absolute;left:300px;top:0;width:200px;height:200px">under2</div>
<div id="solid" style="position:absolute;left:300px;top:0;width:200px;height:200px">solid</div>
<div id="pe" style="pointer-events:none"></div>
<div id="plain"></div>
<script>
  var R = [], cs = function (id) { return getComputedStyle(document.getElementById(id)); };
  var t1 = document.elementFromPoint(50, 50);
  R.push('ghost:' + (t1 ? t1.id : 'null'));   // passes THROUGH the pointer-events:none ghost -> under
  var t2 = document.elementFromPoint(350, 50);
  R.push('solid:' + (t2 ? t2.id : 'null'));   // a normal overlay still wins -> solid (no over-correction)
  R.push('pe:' + cs('pe').pointerEvents);                            // "none"
  R.push('pePV:' + cs('pe').getPropertyValue('pointer-events'));     // "none" via kebab
  R.push('peDflt:' + cs('plain').pointerEvents);                     // "auto" (initial), NOT undefined
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn pointer_events_none_is_transparent_to_hit_testing() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://pe.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "ghost:under",
            "elementFromPoint must pass THROUGH a pointer-events:none overlay to the element behind it, \
             or every click on content under a decorative scrim is swallowed by the scrim",
        ),
        (
            "solid:solid",
            "an overlay WITHOUT pointer-events:none must still win the hit — the fix must not make every \
             overlay transparent",
        ),
        ("pe:none", "getComputedStyle(el).pointerEvents must resolve the computed keyword, not undefined"),
        ("pePV:none", "getPropertyValue('pointer-events') (kebab) must map to the same value"),
        (
            "peDflt:auto",
            "the INITIAL value must resolve too — `undefined` for an unset property is the bug, not a value",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_POINTER_EVENTS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
