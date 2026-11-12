//! **G_MENU_POPOVER_CONTROLS_ARE_CLEAR: the controls inside an opened popover are clickable, and
//! the invisible backdrop behind them still covers the page.**
//!
//! RG.S1.a.a. Zilloft's Beds & Baths filter is a MUI `Menu`: a `position:fixed; z-index:1300`
//! root holding an invisible `z-index:-1` backdrop and a `z-index:500` paper whose option buttons are
//! `position:relative` with an absolutely positioned ripple child. Under the old per-level
//! `z-index:auto` stacking (`parent + 1`) the empty generic wrappers outranked the options, and
//! every control (exact-match, 3+ beds, 2+ baths, Apply) read `OCCLUDED by generic ""`: 18-22
//! refusals per run, where Chrome's `elementFromPoint` returns the control (RG.S1.a transcripts).
//! RG.S2's flat-auto stacking fixed the mechanism; this pins it on the popover shape itself.
//!
//! Mutation that must turn this red: `MANUK_NESTED_Z_MUTATE=1` (a nested explicit z-index uses the
//! absolute `n * 1024 + 1` again, not a step inside its enclosing context) → `Any` reads Obstructed.
//! (`MANUK_STACK_MUTATE`/`MANUK_AUTO_SLICE_MUTATE` leave this shape green: the shipped hit-test ranks
//! positioned boxes by tree order, which already covers the auto-depth part.)
//!
//! Expectations are Chrome's (145.0.7632.116 `elementFromPoint` at each control's centre): the
//! options, the input and Apply hit themselves (or their ripple child); `under`, a page button
//! beneath the popover, hits the backdrop, so it is Obstructed.

use manuk_a11y::Landing;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
html, body { margin: 0 }
.root { position: fixed; z-index: 1300; inset: 0 }
.backdrop { position: fixed; z-index: -1; inset: 0; display: flex }
.paper { position: absolute; z-index: 500; left: 200px; top: 100px; width: 400px; height: 200px; background: #fff }
.list { position: relative; margin: 0; padding: 16px }
.opt { position: relative; display: inline-flex; width: 60px; height: 36px; margin-right: 8px }
.ripple { position: absolute; z-index: 0; left: 0; top: 0; right: 0; bottom: 0 }
.field { position: relative; width: 200px; height: 30px }
.field .in { position: relative; width: 200px; height: 30px }
.sw { position: relative; display: inline-flex; width: 34px; height: 34px }
.sw input { position: absolute; z-index: 1; opacity: 0; left: 0; top: 0; width: 34px; height: 34px; margin: 0 }
.sw .ic { position: relative; width: 16px; height: 16px; margin: 9px }
.sw .rp { position: absolute; z-index: 0; left: 0; top: 0; width: 34px; height: 34px }
.apply { position: absolute; right: 4px; bottom: 4px; width: 80px; height: 30px }
</style></head><body>
<button id="under" style="position:absolute;left:20px;top:20px;width:120px;height:30px">Under</button>
<div class="root">
<div class="backdrop"></div>
<div class="paper"><div class="list">
<div><button class="opt"><span class="ripple"></span>Any</button><button class="opt"><span class="ripple"></span>3+</button></div>
<div class="field"><div class="in"><input aria-label="Min"></div></div>
<label class="lbl"><span class="sw"><input type="radio" aria-label="Exact"><span class="ic"></span><span class="rp"></span></span>Use exact match</label>
<button class="apply">Apply</button>
</div></div></div>
</body></html>"##;

fn landing_of(name: &str) -> Landing {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://popover.test/", &fonts, 800.0);
    let root = page.a11y_tree();
    let n = root
        .iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("no node named {name:?}"));
    root.landing(n.node, None)
}

#[test]
fn every_control_in_the_popover_is_clear() {
    for name in ["Any", "3+", "Min", "Exact", "Apply"] {
        assert!(
            matches!(landing_of(name), Landing::Clear { .. }),
            "{name}: {:?}",
            landing_of(name)
        );
    }
}

#[test]
fn a_page_button_under_the_backdrop_is_obstructed() {
    assert!(
        matches!(landing_of("Under"), Landing::Obstructed { .. }),
        "got {:?}",
        landing_of("Under")
    );
}
