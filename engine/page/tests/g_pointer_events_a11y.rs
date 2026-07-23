//! **G_POINTER_EVENTS_A11Y — `pointer-events: none` is transparent to the AGENT's hit-test too.**
//!
//! Tick 448 taught the JS `elementFromPoint` path to pass through a `pointer-events:none` overlay.
//! But the agent grounds a coordinate click through a *different* path — the accessibility tree's
//! `hit_test` (`manuk_a11y::A11yNode::hit_test`), which is how an agent confirms what it is about to
//! click. That path was still occlusion-only: a decorative `pointer-events:none` overlay with a high
//! stacking layer sat on top of a real control and the agent's hit-test returned the overlay, so an
//! agent (or the shell's click-by-coordinate) actuated the wrong element.
//!
//! This gate proves the live wiring end-to-end: `Page::a11y_tree` now feeds the set of computed
//! `pointer-events:none` nodes into the builder, which marks each `hittable = false` (they stay in the
//! tree — a screen reader still announces them — but drop out of hit-testing). Each claim is a way this
//! goes RED:
//!
//!   * the `pointer-events:none` roled node is present in the tree but `hittable == false`.
//!   * a normal (auto) node stays `hittable == true` — no over-marking.
//!   * `hit_test` over the overlap returns the control BEHIND the overlay, not the overlay.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<button id="btn" style="position:absolute;left:0;top:0;width:120px;height:40px">Buy</button>
<a id="ov" href="#" style="position:absolute;left:0;top:0;width:120px;height:40px;pointer-events:none">scrim</a>
<a id="plain" href="#" style="position:absolute;left:0;top:100px;width:120px;height:40px">Help</a>
</body></html>"##;

fn node_for_id<'a>(
    tree: &'a manuk_a11y::A11yNode,
    dom: &manuk_dom::Dom,
    id: &str,
) -> Option<&'a manuk_a11y::A11yNode> {
    tree.iter()
        .find(|n| dom.element(n.node).and_then(|e| e.attr("id")) == Some(id))
}

#[test]
fn pointer_events_none_is_transparent_to_the_agent_hit_test() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://pe-a11y.test/", &fonts, 800.0);
    let tree = page.a11y_tree();
    let dom = page.dom();

    let ov =
        node_for_id(&tree, dom, "ov").expect("the pointer-events:none link is still in the tree");
    assert!(
        !ov.hittable,
        "G_POINTER_EVENTS_A11Y: the pointer-events:none overlay must be present but hittable=false — it \
         is announced to a screen reader yet transparent to coordinate hit-testing. Got hittable=true, so \
         the live non_hittable_nodes() → build_tree_full wiring is not marking it."
    );

    let plain = node_for_id(&tree, dom, "plain").expect("the plain link is in the tree");
    assert!(
        plain.hittable,
        "G_POINTER_EVENTS_A11Y: a normal (pointer-events:auto) node must stay hittable — the fix must not \
         mark everything non-hittable."
    );

    // The overlap point (60, 20) is inside BOTH the button and the overlay. The overlay is later in DOM
    // (would win an occlusion tie), so returning the button proves the pass-through.
    let hit = tree.hit_test(60.0, 20.0);
    let hit_id = hit
        .and_then(|n| dom.element(n.node))
        .and_then(|e| e.attr("id"));
    assert_ne!(
        hit_id,
        Some("ov"),
        "G_POINTER_EVENTS_A11Y: the agent's hit_test must pass THROUGH the pointer-events:none overlay — \
         it returned the overlay, so an agent grounding a click by coordinate actuates the wrong element."
    );
    assert_eq!(
        hit_id,
        Some("btn"),
        "G_POINTER_EVENTS_A11Y: the hit must land on the button behind the overlay. Got {hit_id:?}."
    );
}
