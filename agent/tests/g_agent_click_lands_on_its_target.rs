//! **G_AGENT_CLICK_LANDS_ON_ITS_TARGET — the drive loop's ACT half was never checked against its
//! PERCEIVE half.**
//!
//! Track C is *perceive → act → observe*, and both ends were built: the a11y tree gives an agent
//! role + name + geometry + state, and `Page::dispatch_click_at` turns a coordinate into a real
//! event through an occlusion-aware hit-test. **Nothing joined them.** Three separate entrances
//! published `bbox.center()` as "where an agent should click this element" —
//! `A11yNode::to_viewport_lines` (the coordinates the *model* is shown, whose own doc comment says
//! "an agent can act on these directly"), `targeting::resolve_target`, and
//! `grounding::ground_action` — and not one of them ran the point back through `hit_test`.
//!
//! ⭐⭐⭐ **ON THE COMMONEST PAGE ON THE WEB — one with a consent overlay — THAT MISFIRES IN
//! TOTAL SILENCE.** The probe that opened this tick, verbatim: `ground_action` returned
//! `Ready { node: "Sign in", point: (140,62), confidence: 1.0 }`; `hit_test(140,62)` returned the
//! cookie banner; `dispatch_click_at(140,62)` returned `proceed = true`; the button's handler never
//! ran. Right node, maximum confidence, wrong coordinate, and **every observable channel reported
//! success.** An agent cannot retry a failure it is told did not happen.
//!
//! So a click point is now a *verified* claim (`A11yNode::landing`): it hit-tests back to the
//! target or a descendant of it, or the target is reported `Obstructed` by whatever covers it.
//!
//! **THE OBSERVABLE IS THE A11Y TREE ITSELF, WHICH IS WHY THIS GATE IS THE DRIVE LOOP AND NOT A
//! UNIT TEST OF A HELPER.** The targets are checkboxes: the agent reads role + name + geometry out
//! of the tree, aims, clicks a coordinate, and then reads `state.checked` back out of the *same*
//! tree to find out whether it worked. Perceive → act → observe, with no script on the page at all
//! — so it holds under a build without SpiderMonkey, and a green result cannot be an artefact of
//! the test poking the DOM itself.
//!
//! **The three arms are one page, because the difference between them is the mechanism.** CLEAR is
//! the control — an ordinary target must still get its centre and behave exactly as before.
//! RESCUED is the load-bearing one: a sticky header over the top of a control is the ordinary web,
//! and the ladder must find a point below it *and the checkbox must actually toggle*, which is the
//! drive loop closing. OBSTRUCTED is the honest refusal that names what to dismiss first.
//!
//! PROVEN RED by the mutation the defect WAS: `landing` returning `Clear { point: centre }` with no
//! hit-test → RESCUED clicks the header and never toggles, OBSTRUCTED reports `Ready`.

use manuk_a11y::{A11yNode, Checked, Landing, Rect, Role};
use manuk_agent::grounding::{ground_action, Grounded};
use manuk_agent::Action;
use manuk_text::FontContext;

/// Three checkboxes and two overlays. `#clear` is uncovered; `#half` has a sticky header over its
/// top 50px; `#under` is completely covered by a consent banner. `#deep` carries the descendant
/// case — a button whose only content is a `<span>`, so a click on it lands on the child.
const HTML: &str = r#"<!doctype html><html><head><style>
  body { margin: 0; }
  input[type=checkbox] { position: absolute; left: 40px; width: 200px; height: 60px; margin: 0; }
  #clear { top: 300px; } #half { top: 400px; } #under { top: 500px; }
  #deep { position: absolute; left: 40px; top: 620px; width: 200px; height: 60px; }
  /* Covers the TOP of #half only (y 380..450 over a control at 400..460) — a sticky header. */
  #header { position: absolute; left: 0; top: 380px; width: 700px; height: 70px; z-index: 50; }
  /* Covers #under entirely (y 480..600 over a control at 500..560) — a consent wall. */
  #banner { position: absolute; left: 0; top: 480px; width: 700px; height: 120px; z-index: 60; }
</style></head><body>
  <input type="checkbox" id="clear" aria-label="Clear target">
  <input type="checkbox" id="half"  aria-label="Half covered">
  <input type="checkbox" id="under" aria-label="Fully covered">
  <button id="deep"><span>Deep target</span></button>
  <div id="header">sticky header</div>
  <div id="banner">We use cookies</div>
</body></html>"#;

const VIEWPORT: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 700.0,
};

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

/// Ground `intent` the way the agent loop does, then act on whatever coordinate it was handed —
/// which is the point of the gate: a caller that trusts the published point must not be misled.
fn drive(page: &mut manuk_page::Page, fonts: &FontContext, intent: &str) -> Grounded {
    let tree = page.a11y_tree();
    let g = ground_action(
        &Action::ClickText {
            role: String::new(),
            name: intent.to_string(),
        },
        &tree,
        VIEWPORT,
        0.2,
    );
    let point = match &g {
        Grounded::Ready { point, .. } | Grounded::Obstructed { point, .. } => *point,
        other => panic!("VACUOUS: {intent:?} resolved to no target at all: {other:?}"),
    };
    page.dispatch_click_at(point.0, point.1, fonts, 800.0);
    g
}

#[test]
fn a_grounded_click_point_reaches_the_element_it_names() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://drive.test/", &fonts, 800.0);

    // ── VACUITY. Three boxed, named checkboxes and two overlays ON A HIGHER LAYER. A page that
    // laid none of this out, or that put the overlays on layer 0, would sail through every arm.
    {
        let tree = page.a11y_tree();
        let boxes: Vec<&A11yNode> = tree
            .iter()
            .filter(|n| n.role == Role::CheckBox && n.bbox.is_some())
            .collect();
        assert_eq!(
            boxes.len(),
            3,
            "VACUOUS: expected 3 boxed checkboxes, got {boxes:#?}"
        );
        assert!(
            boxes
                .iter()
                .all(|n| n.state.checked == Some(Checked::False)),
            "VACUOUS: a target starts checked, so a 'it toggled' assertion proves nothing"
        );
        assert!(
            tree.iter().filter(|n| n.z > 0 && n.bbox.is_some()).count() >= 2,
            "VACUOUS: the overlays are not on a higher stacking layer, so nothing is covered"
        );
    }

    // ── ARM 1 · CLEAR (control) — an unobstructed target is UNCHANGED: it still gets its box
    // centre, and the click still lands. This check is an addition, not a new aiming policy.
    let g = drive(&mut page, &fonts, "Clear target");
    let Grounded::Ready { node, point, .. } = g else {
        panic!("an uncovered target must ground as Ready, got {g:?}");
    };
    let centre = page
        .a11y_tree()
        .iter()
        .find(|n| n.node == node)
        .and_then(|n| n.bbox)
        .expect("the clear target has geometry")
        .center();
    assert_eq!(
        point, centre,
        "CONTROL: an unobstructed target must still be aimed at its box centre"
    );
    assert_eq!(
        checked(&page, "Clear target"),
        Checked::True,
        "the control click did not toggle the checkbox — the drive loop is broken independently \
         of any obstruction"
    );

    // ── ARM 2 · RESCUED — the load-bearing arm. The header covers y 380..450 and the control sits
    // at 400..460, so the CENTRE (y=430) is intercepted. A verified point must move below the
    // header, and the checkbox must actually toggle.
    let g = drive(&mut page, &fonts, "Half covered");
    let Grounded::Ready { point, .. } = g else {
        panic!("a half-covered target is still reachable and must ground as Ready, got {g:?}");
    };
    assert!(
        point.1 > 450.0,
        "the rescued point {point:?} is still under the header, which ends at y=450"
    );
    assert_eq!(
        checked(&page, "Half covered"),
        Checked::True,
        "THE DRIVE LOOP DID NOT CLOSE: the agent was handed {point:?} for a target it could \
         reach, clicked exactly there, and nothing happened"
    );

    // ── ARM 3 · OBSTRUCTED — honest refusal, naming what to deal with first.
    let g = drive(&mut page, &fonts, "Fully covered");
    let Grounded::Obstructed { by, point, .. } = g else {
        panic!("a fully covered target must ground as Obstructed, got {g:?}");
    };
    assert_eq!(
        page.a11y_tree().hit_test(point.0, point.1).map(|n| n.node),
        Some(by),
        "Obstructed must name the node that actually intercepts the click"
    );
    assert_eq!(
        checked(&page, "Fully covered"),
        Checked::False,
        "VACUOUS: the covered target was reachable after all, so this arm proves nothing"
    );

    // ── AND THE MODEL-FACING VIEW SAYS SO. `to_viewport_lines` is what the model reads; a covered
    // element listed with a bare coordinate is an instruction to click the thing on top of it.
    let lines = page.a11y_tree().to_viewport_lines(VIEWPORT);
    let line_for = |name: &str| {
        lines
            .iter()
            .find(|l| l.contains(name))
            .unwrap_or_else(|| panic!("VACUOUS: {name:?} is not in the viewport listing"))
            .clone()
    };
    assert!(
        line_for("Fully covered").ends_with("obstructed"),
        "a covered element must be MARKED, not silently mis-addressed: {:?}",
        line_for("Fully covered")
    );
    assert!(
        !line_for("Clear target").ends_with("obstructed"),
        "CONTROL: an uncovered element must not be marked obstructed"
    );
}

/// `landing` is the one rule all three entrances share, so its contract is asserted directly:
/// **a hit on a DESCENDANT reaches the target** — events bubble, so clicking the `<span>` inside a
/// button activates the button — and a node that is not in the tree has nowhere to aim.
#[test]
fn a_hit_on_a_descendant_reaches_the_target_it_is_inside() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://drive.test/", &fonts, 800.0);
    let tree = page.a11y_tree();

    let btn = tree
        .iter()
        .find(|n| n.role == Role::Button && n.name.contains("Deep target"))
        .expect("VACUOUS: no deep-target button in the tree");
    let inner: Vec<manuk_dom::NodeId> = btn.children.iter().map(|c| c.node).collect();
    assert!(
        !inner.is_empty(),
        "VACUOUS: the fixture's button has no child node, so 'a descendant counts' is untested"
    );

    let Landing::Clear { point } = tree.landing(btn.node, Some(VIEWPORT)) else {
        panic!("the uncovered button must have a clear point");
    };
    let hit = tree
        .hit_test(point.0, point.1)
        .expect("something is at the verified point");
    assert!(
        hit.node == btn.node || inner.contains(&hit.node),
        "the verified point must reach the button or something inside it, reached {:?} {:?}",
        hit.role,
        hit.name
    );

    assert_eq!(
        tree.landing(manuk_dom::NodeId(u32::MAX as u64), Some(VIEWPORT)),
        Landing::Unreachable,
        "a node that is not in the tree has nowhere to aim"
    );
}
