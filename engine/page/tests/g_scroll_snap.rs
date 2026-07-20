//! # G_SCROLL_SNAP — the carousel stops ON a slide
//!
//! `scroll-snap-type` + `scroll-snap-align` is what every paged feed, image gallery, story tray and
//! mobile card row on the web is built from. Without it a flick lands wherever momentum stopped —
//! two half-slides on screen and neither readable — and the page looks broken in a way no capability
//! count can see, because the scroll container "works".
//!
//! Measured absent by `g_probe_capabilities` (`scrollsnap: no`) before this landed.
//!
//! ## How each assertion here can go RED
//!
//! - **A scroll LANDS on a snap point.** RED, run: return `at` unchanged from `snap_scroll`. The
//!   container still scrolls, still clamps, still reports a sane offset — and stops mid-slide
//!   forever. Everything except the one number a user can see stays correct.
//!
//! - **It snaps to the NEAREST point, not the first.** RED, run: take `xs[0]` instead of the
//!   minimum by distance. Scrolling to slide 3 lands back on slide 0, and a test that only ever
//!   scrolls a short distance from the start cannot tell the two apart — which is why this scrolls
//!   deep into the container.
//!
//! - **Snapping happens AFTER clamping.** RED, run: snap before `clamp`. A snap point beyond the
//!   scrollable range gets chosen and then clamped back to an unaligned offset, so the container
//!   can never reach its own LAST slide — the classic carousel bug, and invisible unless the gate
//!   scrolls to the end.
//!
//! - **A container with NO aligned children still scrolls.** RED, run: let the empty candidate set
//!   fall through to "nearest of nothing" pinned at 0. `scroll-snap-type` alone would freeze the
//!   container at the top, turning a declared-but-unused property into a broken scroller.
//!
//! - **The properties round-trip to `getComputedStyle`.** A carousel library reads `scrollSnapType`
//!   to decide whether to run its own JS fallback; an empty string sends it down the polyfill path
//!   over a working native snap.

use manuk_text::FontContext;

/// Five 100px slides in a 100px-wide scroller: snap points at exactly 0, 100, 200, 300, 400.
/// Round numbers on purpose — an assertion that has to allow slop cannot tell snapping from
/// not-snapping.
const HTML: &str = r#"<!doctype html><html><head><style>
  #rail { overflow-y: scroll; width: 200px; height: 100px; scroll-snap-type: y mandatory; }
  #rail > div { height: 100px; scroll-snap-align: start; }
  /* Declares snapping but has no aligned children — must still scroll freely. */
  #loose { overflow-y: scroll; width: 200px; height: 100px; scroll-snap-type: y mandatory; }
  #loose > div { height: 100px; }
</style></head><body>
  <div id="rail"><div>a</div><div>b</div><div>c</div><div>d</div><div>e</div></div>
  <div id="loose"><div>a</div><div>b</div><div>c</div><div>d</div><div>e</div></div>
</body></html>"#;

fn ids(page: &manuk_page::Page, sel: &str) -> manuk_dom::NodeId {
    let root = page.dom().root();
    manuk_css::query_selector_all(page.dom(), root, sel)[0]
}

#[test]
fn g_scroll_snap() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://snap.test/", &fonts, 800.0);
    let rail = ids(&page, "#rail");
    let loose = ids(&page, "#loose");

    // ── BASELINE: the container genuinely scrolls. Without this every "it snapped to 0" result
    //    below would be indistinguishable from a container that never moved at all.
    let far = page.set_element_scroll(loose, 0.0, 250.0);
    assert!(
        far.1 > 200.0,
        "a snap container with NO aligned children must scroll FREELY — got {far:?}. \
         Pinning it (the empty-candidate-set bug) turns a declared property into a dead scroller"
    );

    // ── A short scroll lands on the NEAR slide, not wherever it was dropped.
    let a = page.set_element_scroll(rail, 0.0, 120.0);
    assert_eq!(
        a.1, 100.0,
        "scrolling to 120 must LAND on the slide at 100 — a scroller that stops at 120 shows \
         two half-slides and neither is readable"
    );

    // ── Deep into the rail: proves NEAREST, not first-candidate. 270 is nearer 300 than 200.
    let b = page.set_element_scroll(rail, 0.0, 270.0);
    assert_eq!(
        b.1, 300.0,
        "must snap to the NEAREST point; taking the first candidate lands back on slide 0 and \
         only a deep scroll can tell the difference"
    );
    let c = page.set_element_scroll(rail, 0.0, 230.0);
    assert_eq!(c.1, 200.0, "230 is nearer 200 than 300");

    // ── The LAST slide is reachable. Snapping before clamping picks an out-of-range point and
    //    clamps back to an unaligned offset, so the container never reaches its own end.
    let end = page.set_element_scroll(rail, 0.0, 9999.0);
    assert_eq!(
        end.1, 400.0,
        "the final slide must be reachable — got {}. Snapping BEFORE the clamp is what makes a \
         carousel refuse to show its last item",
        end.1
    );

    // ── The unsnapped axis is untouched: `scroll-snap-type: x` says nothing about y.
    let y = page.set_element_scroll(rail, 7.0, 0.0);
    assert_eq!(y.1, 0.0, "y still snaps");
    assert!(
        y.0 <= 7.0,
        "x must not be snapped by a y-only declaration; got {}",
        y.0
    );
}

/// The properties reach `getComputedStyle` — the feature-detect a carousel library runs.
#[test]
fn g_scroll_snap_computed_style() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(
        r#"<!doctype html><html><head><style>
             #r { overflow-x: scroll; scroll-snap-type: y mandatory; width: 100px; }
             #s { scroll-snap-align: center; }
           </style></head><body>
             <div id="r"><div id="s">a</div></div><div id="out">-</div>
             <script>window.__ready = 1;</script>
           </body></html>"#,
        "https://snap.test/",
        &fonts,
        800.0,
    );
    page.eval_for_test(
        "(function(){var g=getComputedStyle;\
           document.getElementById('out').textContent = \
             g(document.getElementById('r')).scrollSnapType + '|' + \
             g(document.getElementById('s')).getPropertyValue('scroll-snap-align');})()",
    );
    let out = ids(&page, "#out");
    let got = page.dom().text_content(out);
    assert!(
        got.starts_with("y ") && got.ends_with("|center"),
        "scroll-snap must round-trip through getComputedStyle (camelCase AND getPropertyValue) — \
         got {got:?}; an empty string sends every carousel library down its polyfill path"
    );
}
