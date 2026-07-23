//! **G_INERT_A11Y — the HTML `inert` attribute is transparent to the AGENT's hit-test.**
//!
//! `inert` is what `<dialog>.showModal()` (and every hand-rolled modal) sets on the rest of the page
//! to neutralise it: the backdrop content stays visible and announced, but must not receive clicks. An
//! agent grounds a coordinate click through the accessibility tree's `hit_test`
//! (`manuk_a11y::A11yNode::hit_test`) — the same path tick 449 taught to honour `pointer-events:none`.
//! That path knew nothing about `inert`, so an agent (or the shell's click-by-coordinate) actuated a
//! button *behind an open modal* — a component-#2 failure that silently defeats the whole point of a
//! modal.
//!
//! Unlike `pointer-events` (which inherits through the cascade), `inert` inherits down the DOM subtree:
//! the attribute sits on ONE container and every descendant becomes non-interactive. `non_hittable_nodes()`
//! now walks the subtree and unions the inert descendants into the set fed to the a11y builder, which
//! marks each `hittable = false` (they stay in the tree). Each claim is a way this goes RED:
//!
//!   * a control inside an `inert` container is present in the tree but `hittable == false`.
//!   * a sibling control OUTSIDE the inert container stays `hittable == true` — no over-marking.
//!   * `hit_test` over the inert control does NOT return it — the agent cannot actuate neutralised UI.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="bg" inert>
  <button id="behind" style="position:absolute;left:0;top:0;width:120px;height:40px">Behind</button>
</div>
<button id="live" style="position:absolute;left:0;top:100px;width:120px;height:40px">Live</button>
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
fn inert_is_transparent_to_the_agent_hit_test() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://inert-a11y.test/", &fonts, 800.0);
    let tree = page.a11y_tree();
    let dom = page.dom();

    let behind = node_for_id(&tree, dom, "behind")
        .expect("the button inside the inert container is still in the tree");
    assert!(
        !behind.hittable,
        "G_INERT_A11Y: a control inside an `inert` container must be present but hittable=false — the \
         page behind an open modal is announced yet transparent to coordinate hit-testing. Got \
         hittable=true, so the inert subtree walk in non_hittable_nodes() → build_tree_full is not \
         marking it."
    );

    let live = node_for_id(&tree, dom, "live").expect("the live button is in the tree");
    assert!(
        live.hittable,
        "G_INERT_A11Y: a control OUTSIDE the inert container must stay hittable — the fix must not mark \
         everything non-hittable (the inert subtree walk must stop at the container's descendants)."
    );

    // (60, 20) is inside the inert button. It must NOT be the hit target — an agent grounding a click
    // there is trying to press a button neutralised by the modal, and hit_test must refuse it.
    let hit = tree.hit_test(60.0, 20.0);
    let hit_id = hit
        .and_then(|n| dom.element(n.node))
        .and_then(|e| e.attr("id"));
    assert_ne!(
        hit_id,
        Some("behind"),
        "G_INERT_A11Y: the agent's hit_test must NOT return a control inside an `inert` subtree — it \
         returned it, so an agent grounding a click by coordinate actuates UI the page has neutralised."
    );
}
