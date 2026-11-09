//! **G_OVERFLOW_CLIPPED_CONTENT_DOES_NOT_OCCLUDE: content an `overflow` ancestor clips away is not
//! painted, so it cannot take a click.**
//!
//! RG.S1.a. A carousel is a positioned `overflow:hidden` wrapper whose slides overflow it (here the
//! wrapper is 1px high and the slides 80px). The hit-test used the slides' raw boxes, so every
//! control the slides overflowed onto read `OCCLUDED by image` and the click was refused, where
//! Chrome clicks straight through (the slides are clipped away there). Found on the Dashdish
//! carousels: 6 filter pills and every "See all" button.
//!
//! Mutations that must turn this red:
//!   1. `hit_test` ignoring `clip`                       → `link` reads Obstructed
//!   2. `clip_map_for(true)` not skipping `html`/`body`  → `body_scroll` reads Unreachable
//!   3. `landing` refusing a target wholly outside its clip → `clipped_target` reads Unreachable
//!   4. every positioned box treated as clipped          → `under` reads Clear
//!
//! The occlusion expectations are Chrome's (145.0.7632.116 `elementFromPoint` at each button's
//! centre): pill/link/body hit themselves, under hits the unclipped slide. `clipped_target` is
//! scrolled out of its `overflow:hidden` box (elementFromPoint there hits body_scroll); a click
//! scrolls its container to it first (Playwright's scrollIntoViewIfNeeded), so it is not refused.
//! Refusing it read UNREACHABLE on every scrolled-out listbox option (RG.S1.a, opendining-4).

use manuk_a11y::Landing;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
html, body { margin: 0; overflow: hidden; height: 100% }

.row { width: 300px; height: 30px; margin: 10px 0 }
.row button { position: relative; width: 120px; height: 30px }
.car { position: relative; width: 300px; height: 1px; overflow: hidden }
.slide { position: absolute; left: 0; top: 0; width: 300px; height: 80px; background: #ccc }
.track { position: relative; width: 300px; height: 1px }
.free { position: relative; width: 300px; height: 1px }
.free .slide { position: absolute }
.clipbox { position: relative; width: 100px; height: 20px; overflow: hidden }
.clipbox button { position: absolute; left: 0; top: 40px; width: 90px; height: 20px }
</style></head><body>
<div class="row"><button id="pill">Pill</button></div>
<div class="car"><div class="track"><div class="slide"><img alt="Slide" width="300" height="80"></div></div></div>
<div class="row"><button id="link">Link</button></div>
<div class="clipbox"><button id="clipped_target">Hidden</button></div>
<div class="row"><button id="body_scroll">Body</button></div>
<div class="row"><button id="under">Under</button></div>
<div class="free"><div class="slide" style="top:-40px"></div></div>
</body></html>"##;

fn landing_of(name: &str) -> Landing {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://clip.test/", &fonts, 800.0);
    let root = page.a11y_tree();
    let n = root
        .iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("no node named {name:?}"));
    root.landing(n.node, None)
}

#[test]
fn a_button_a_clipped_carousel_overflows_onto_is_clear() {
    // The 1px wrapper sits at y=50; the slide's raw box (50..130) covers `link` (y 60..90) but is
    // clipped to 50..51.
    assert!(
        matches!(landing_of("Link"), Landing::Clear { .. }),
        "got {:?}",
        landing_of("Link")
    );
}

#[test]
fn a_button_before_the_carousel_is_clear() {
    assert!(matches!(landing_of("Pill"), Landing::Clear { .. }));
}

#[test]
fn a_target_wholly_outside_its_own_clip_is_still_clickable() {
    assert!(
        matches!(landing_of("Hidden"), Landing::Clear { .. }),
        "got {:?}",
        landing_of("Hidden")
    );
}

#[test]
fn body_overflow_hidden_is_the_viewport_not_a_clip() {
    assert!(
        matches!(landing_of("Body"), Landing::Clear { .. }),
        "got {:?}",
        landing_of("Body")
    );
}

#[test]
fn unclipped_overflow_still_occludes() {
    // The control: the same slide with no clipping ancestor covers `under`, as it does in Chrome.
    assert!(
        matches!(landing_of("Under"), Landing::Obstructed { .. }),
        "got {:?}",
        landing_of("Under")
    );
}
