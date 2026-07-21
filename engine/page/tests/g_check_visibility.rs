//! **G_CHECK_VISIBILITY — `Element.checkVisibility([options])`.**
//!
//! "Is this element actually rendered?" — the one call that replaces the `getComputedStyle` +
//! `offsetParent` + ancestor-walk dance every UI library reinvents (scroll-into-view guards, lazy
//! mounting, a11y on-screen checks). It was ABSENT, so the call threw `checkVisibility is not a
//! function`.
//!
//! Teeth, keyed to the actual cascade (a stub that returns a constant fails several at once):
//!   * `shown` — a normal element is visible.
//!   * `none` — `display:none` is not.
//!   * `child-of-none` — a child of a `display:none` ancestor is not (proves the ancestor WALK, since
//!     the child's own computed `display` is unaffected).
//!   * `vis-default` — `visibility:hidden` is STILL "visible" by default (default checks only rendering).
//!   * `vis-opt` — with `{visibilityProperty:true}` it is NOT.
//!   * `op-default` / `op-opt` — `opacity:0` is visible by default, not with `{opacityProperty:true}`.
//!
//! Proven RED: delete the shim and `present` reads `undefined` while the first call throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="shown">a</div>
<div id="none" style="display:none">b</div>
<div style="display:none"><span id="child">c</span></div>
<div id="vh" style="visibility:hidden">d</div>
<div id="op" style="opacity:0">e</div>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function $(id) { return document.getElementById(id); }
try {
  push('present:' + (typeof $('shown').checkVisibility === 'function'));
  push('shown:' + ($('shown').checkVisibility() === true));
  push('none:' + ($('none').checkVisibility() === false));
  push('child-of-none:' + ($('child').checkVisibility() === false));
  // visibility:hidden — visible by DEFAULT, hidden only when the option asks.
  push('vis-default:' + ($('vh').checkVisibility() === true));
  push('vis-opt:' + ($('vh').checkVisibility({ visibilityProperty: true }) === false));
  // opacity:0 — visible by DEFAULT, hidden only when the option asks.
  push('op-default:' + ($('op').checkVisibility() === true));
  push('op-opt:' + ($('op').checkVisibility({ opacityProperty: true }) === false));
} catch (e) {
  push('THREW:' + e);
}
$('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn check_visibility_reads_the_real_cascade() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://vis.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CHECK-VIS RESULT: {got}");

    for claim in [
        "present:true",
        "shown:true",
        "none:true",          // (=== false held) display:none -> not visible
        "child-of-none:true", // (=== false held) ancestor walk catches it
        "vis-default:true",   // visibility:hidden is visible by default
        "vis-opt:true",       // (=== false held) ...but not with visibilityProperty
        "op-default:true",    // opacity:0 is visible by default
        "op-opt:true",        // (=== false held) ...but not with opacityProperty
    ] {
        assert!(
            got.contains(claim),
            "G_CHECK_VISIBILITY: expected `{claim}`\n  got: {got}\n\n  \
             `Element.checkVisibility` must read the real cascade: false for display:none (self OR \
             ancestor) and disconnected; visibility/opacity only fold in when the option asks."
        );
    }
}
