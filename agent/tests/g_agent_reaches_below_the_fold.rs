//! **G_AGENT_REACHES_BELOW_THE_FOLD — the drive loop stopped at the first screenful, and said it
//! had not.**
//!
//! t1356 made a click point a *verified* claim: `A11yNode::landing` hit-tests the point back to
//! the target before an agent is handed it, so a button under a consent banner grounds as
//! `Obstructed` instead of as a confident coordinate that activates the banner. That fix has three
//! arms, and **it only covered two.** The third — `Landing::Unreachable` — fell back to
//! `bbox.center()` with no flag, and `Unreachable` is what a target gets when *no part of its box
//! is inside the viewport*. Which is to say: **everything below the fold**, on every page on the
//! web.
//!
//! ⭐⭐⭐ **THE OBSTRUCTION MAP AT SCROLL 0 IS NOT THE OBSTRUCTION MAP AT THE SCROLL WHERE THE
//! CLICK HAPPENS.** This is why the off-screen case cannot be answered by aiming at the box centre
//! and hoping, and it is what makes the bug invisible rather than merely approximate. A
//! `position:sticky` header's *document* rect moves with the view (`Page::restick`). A checkbox at
//! y=1000 is unobstructed while the header sits at y=0 — and is underneath it the moment the agent
//! scrolls far enough to see it. A below-the-fold target is **by definition** clicked after a
//! scroll, so a point verified in the viewport the target was *found* in says nothing about the
//! viewport the click happens in.
//!
//! The probe that opened this tick, verbatim, on the page below:
//!
//! ```text
//!   landing(target at y=1000, viewport 0..700) = Unreachable
//!   ground_action                              = Ready { point: (140,1030), confidence: 1.0 }
//!   ... agent scrolls to reach it; the sticky header re-sticks to y=1000..1070, over the target
//!   hit_test(140,1030)  = the header
//!   dispatch_click_at   = proceed: true
//!   state.checked       = False
//! ```
//!
//! Right node, maximum confidence, wrong coordinate, `proceed = true`, and nothing happened — the
//! same silent-misfire signature t1356 was written to eliminate, one branch over.
//!
//! **The fix is a variant that carries no point.** `Grounded::OffScreen { dy }` cannot be clicked;
//! it can only be scrolled to and asked again, which forces the verification to be re-run in the
//! viewport the click will actually occur in. `Landing::Unreachable` narrows to its true meaning
//! (on screen and `pointer-events: none`), which turns `to_viewport_lines`' standing comment about
//! that arm into a checked claim.
//!
//! **PERCEIVE → SCROLL → PERCEIVE AGAIN → ACT → OBSERVE, and the observable is the a11y tree
//! itself.** The targets are checkboxes: the agent reads role + name + geometry out of the tree,
//! is told to scroll, scrolls, re-reads, aims, clicks a coordinate, and reads `state.checked` back
//! out of the *same* tree. No script on the page at all, so this holds in a build without
//! SpiderMonkey and a green result cannot be an artefact of the test poking the DOM.
//!
//! PROVEN RED by the mutation the defect WAS: `Landing::OffScreen { .. } => (bbox.center(), None,
//! None)` in `resolve_target` → ARM 2 grounds `Ready` at scroll 0, the agent never scrolls, and
//! the checkbox never toggles.

use manuk_a11y::{A11yNode, Checked, Landing, Rect, Role};
use manuk_agent::grounding::{ground_action, Grounded};
use manuk_agent::Action;
use manuk_text::FontContext;

/// A 2400px document with a **sticky** header and four absolutely-placed checkboxes.
///
/// - `#near` (y 200) is on screen and clear — the control.
/// - `#far` (y 1000, 200 tall) is below the fold, and the 140px header re-sticks *over its centre*
///   once the agent scrolls to it. The rescue ladder must find a point below the header.
/// - `#naive` (y 1900, 200 tall) is the same shape, and is driven the OLD way — aim at the box
///   centre, scroll, click — to show the failure this gate exists for is real and not hypothetical.
/// - `#bur` (y 1600) is below the fold *and* completely covered once you get there.
/// - `#aside` is parked off to the right of the viewport: no `dy` brings it closer, and claiming
///   otherwise would send an agent scrolling forever.
const HTML: &str = r#"<!doctype html><html><head><style>
  body { margin: 0; height: 2400px; }
  #hdr { position: sticky; top: 0; height: 140px; background: #ccc; z-index: 50; }
  input[type=checkbox] { position: absolute; left: 40px; width: 200px; margin: 0; }
  #near  { top: 200px;  height: 60px; }
  #far   { top: 1000px; height: 200px; }
  #bur   { top: 1600px; height: 60px; }
  #naive { top: 1900px; height: 200px; }
  #aside { left: 1400px; top: 300px; height: 60px; }
  #wall { position: absolute; left: 0; top: 1600px; width: 700px; height: 200px; z-index: 60; }
</style></head><body>
  <div id="hdr">sticky header</div>
  <input type="checkbox" id="near"  aria-label="Near target">
  <input type="checkbox" id="far"   aria-label="Far target">
  <input type="checkbox" id="bur"   aria-label="Buried target">
  <input type="checkbox" id="naive" aria-label="Naive target">
  <input type="checkbox" id="aside" aria-label="Aside target">
  <div id="wall">We use cookies</div>
</body></html>"#;

const VW: f32 = 800.0;
const VH: f32 = 700.0;

/// The viewport in DOCUMENT coordinates at scroll `y` — the space the a11y tree's boxes live in.
fn vp(y: f32) -> Rect {
    Rect {
        x: 0.0,
        y,
        width: VW,
        height: VH,
    }
}

/// Read a target's checkedness **back out of the accessibility tree** — the agent's own perception
/// channel, not a DOM back door. This is the "observe" leg of the loop.
fn checked(page: &manuk_page::Page, name: &str) -> Checked {
    page.a11y_tree()
        .iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("VACUOUS: no node named {name:?} in the a11y tree"))
        .state
        .checked
        .unwrap_or_else(|| panic!("VACUOUS: {name:?} exposes no checked state to observe"))
}

fn ground(page: &manuk_page::Page, intent: &str, scroll: f32) -> Grounded {
    ground_action(
        &Action::ClickText {
            role: String::new(),
            name: intent.to_string(),
        },
        &page.a11y_tree(),
        vp(scroll),
        0.2,
    )
}

/// The sticky header's box, whichever node carries it — read from the tree so the assertion is
/// about what the *agent* can see, not about a DOM id.
fn header_box(page: &manuk_page::Page) -> Rect {
    page.a11y_tree()
        .iter()
        .filter_map(|n| n.bbox)
        .find(|b| b.height == 140.0 && b.width == VW)
        .expect("VACUOUS: the sticky header has no box in the a11y tree")
}

#[test]
fn an_agent_reaches_a_target_below_the_fold() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://drive.test/", &fonts, VW);

    // ── VACUITY. Five boxed, named, UNCHECKED checkboxes; a wall on a higher layer; and — the one
    // that matters — a header that really does RE-STICK, because every below-the-fold arm below is
    // about geometry changing under the scroll. A page where the header stayed at y=0 would let a
    // scroll-blind implementation pass arm 2.
    {
        let tree = page.a11y_tree();
        let boxes: Vec<&A11yNode> = tree
            .iter()
            .filter(|n| n.role == Role::CheckBox && n.bbox.is_some())
            .collect();
        assert_eq!(
            boxes.len(),
            5,
            "VACUOUS: expected 5 boxed checkboxes, got {boxes:#?}"
        );
        assert!(
            boxes
                .iter()
                .all(|n| n.state.checked == Some(Checked::False)),
            "VACUOUS: a target starts checked, so 'it toggled' proves nothing"
        );
        assert!(
            tree.iter().any(|n| n.z >= 60 && n.bbox.is_some()),
            "VACUOUS: the wall is not on a higher stacking layer, so nothing is covered"
        );
    }
    assert_eq!(header_box(&page).y, 0.0, "the header starts at the top");
    page.view_changed(1000.0, VW, VH, true);
    assert_eq!(
        header_box(&page).y,
        1000.0,
        "VACUOUS: the header did not RE-STICK under the scroll, so no arm below can distinguish a \
         point verified at scroll 0 from one verified where the click happens"
    );
    page.view_changed(0.0, VW, VH, true);

    // ── ARM 1 · CONTROL — an on-screen target is UNCHANGED: still its box centre, still one round,
    // still toggles. This tick adds a case; it does not re-aim the common path.
    let g = ground(&page, "Near target", 0.0);
    let Grounded::Ready { node, point, .. } = g else {
        panic!("an on-screen, unobstructed target must ground as Ready, got {g:?}");
    };
    let centre = page
        .a11y_tree()
        .iter()
        .find(|n| n.node == node)
        .and_then(|n| n.bbox)
        .expect("the control target has geometry")
        .center();
    assert_eq!(
        point, centre,
        "CONTROL: an on-screen unobstructed target must still be aimed at its box centre"
    );
    page.dispatch_click_at(point.0, point.1, &fonts, VW);
    assert_eq!(
        checked(&page, "Near target"),
        Checked::True,
        "the control click did not toggle — the drive loop is broken independently of any scroll"
    );

    // ── ARM 2 · THE LOOP CLOSES ACROSS A SCROLL — the load-bearing arm.
    //
    // Round 1: the target is off screen, so the agent is told to SCROLL, and is given no
    // coordinate it could misuse.
    let g = ground(&page, "Far target", 0.0);
    let Grounded::OffScreen { dy, node, .. } = g else {
        panic!("a below-the-fold target must ground as OffScreen, got {g:?}");
    };
    let far_box = page
        .a11y_tree()
        .iter()
        .find(|n| n.node == node)
        .and_then(|n| n.bbox)
        .expect("the far target has geometry");
    assert!(
        far_box.intersects(&vp(dy)),
        "the proposed scroll dy={dy} does not actually bring {far_box:?} into view"
    );

    // The agent scrolls — and the header follows it down, onto the target's centre. This is the
    // fact the pre-tick code could not see, so assert it rather than assume it.
    page.view_changed(dy, VW, VH, true);
    let hdr = header_box(&page);
    let far_centre = far_box.center();
    assert!(
        hdr.y <= far_centre.1 && far_centre.1 < hdr.bottom(),
        "VACUOUS: after the scroll the header {hdr:?} is NOT over the target's centre \
         {far_centre:?}, so this arm never exercises the re-verification"
    );

    // Round 2: ground AGAIN, in the viewport the click will happen in. The obstruction is found
    // and the ladder rescues a point below the header.
    let g = ground(&page, "Far target", dy);
    let Grounded::Ready { point, .. } = g else {
        panic!("after scrolling, the target is reachable and must ground as Ready, got {g:?}");
    };
    assert!(
        point.1 >= hdr.bottom(),
        "the rescued point {point:?} is still under the sticky header, which ends at \
         y={}",
        hdr.bottom()
    );
    page.dispatch_click_at(point.0, point.1, &fonts, VW);
    assert_eq!(
        checked(&page, "Far target"),
        Checked::True,
        "THE DRIVE LOOP DID NOT CLOSE: the agent was told to scroll, scrolled, was handed \
         {point:?} for a target it could reach, clicked exactly there, and nothing happened"
    );

    // ── ARM 3 · THE OLD WAY, ON AN IDENTICAL TARGET — the failure this gate exists for, pinned as
    // a NEGATIVE row so no future edit can satisfy the arms above by renaming something. Aim at
    // the bare box centre (what `resolve_target` used to publish), scroll to it, click it.
    let naive_centre = page
        .a11y_tree()
        .iter()
        .find(|n| n.name == "Naive target")
        .and_then(|n| n.bbox)
        .expect("the naive target has geometry")
        .center();
    page.view_changed(naive_centre.1 - 100.0, VW, VH, true);
    page.dispatch_click_at(naive_centre.0, naive_centre.1, &fonts, VW);
    assert_eq!(
        checked(&page, "Naive target"),
        Checked::False,
        "VACUOUS: the unverified box centre reached its target anyway, so nothing above is \
         evidence that verifying it bought anything"
    );

    // ── ARM 4 · HONEST REFUSAL AFTER THE SCROLL — a target that is off screen AND covered gets
    // `OffScreen` first (where is it) and `Obstructed` second (what is on it), naming the node to
    // dismiss. Two different answers to two different questions, in the right order.
    let g = ground(&page, "Buried target", 0.0);
    let Grounded::OffScreen { dy, .. } = g else {
        panic!("a below-the-fold covered target is OFF SCREEN first, got {g:?}");
    };
    page.view_changed(dy, VW, VH, true);
    let g = ground(&page, "Buried target", dy);
    let Grounded::Obstructed { by, point, .. } = g else {
        panic!("after the scroll the covered target must ground as Obstructed, got {g:?}");
    };
    assert_eq!(
        page.a11y_tree().hit_test(point.0, point.1).map(|n| n.node),
        Some(by),
        "Obstructed must name the node that actually intercepts the click"
    );
    assert_eq!(
        checked(&page, "Buried target"),
        Checked::False,
        "VACUOUS: the covered target was reachable after all, so this arm proves nothing"
    );

    // ── ARM 5 · PINNED NEGATIVE — a target parked OUTSIDE THE HORIZONTAL BAND is `Unreachable`,
    // never `OffScreen`. No vertical scroll brings it closer, and an agent handed a `dy` for it
    // scrolls the whole document and asks again forever.
    let tree = page.a11y_tree();
    let aside = tree
        .iter()
        .find(|n| n.name == "Aside target")
        .expect("VACUOUS: the aside target is missing from the tree");
    assert!(
        aside.bbox.is_some_and(|b| b.x >= VW),
        "VACUOUS: the aside target is inside the viewport's horizontal band"
    );
    assert_eq!(
        tree.landing(aside.node, Some(vp(0.0))),
        Landing::Unreachable,
        "a target no vertical scroll can reach must NOT be reported as OffScreen"
    );

    // ── AND THE MODEL-FACING VIEW IS UNCHANGED. `to_viewport_lines` lists only what is on screen,
    // so its `Unreachable` arm now provably describes `pointer-events: none` and nothing else.
    let lines = tree.to_viewport_lines(vp(0.0));
    assert!(
        lines.iter().any(|l| l.contains("Near target")),
        "the on-screen control must still be listed: {lines:#?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("Far target")),
        "an off-screen target must not be listed with a coordinate: {lines:#?}"
    );
}
