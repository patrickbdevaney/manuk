//! **G_SCROLL_CENTRES_UNDER_STICKY_HEADER — the scroll an act does to reach a target put the target
//! at the very top of the viewport, which is exactly where a sticky header sits.** (RG.E2.b)
//!
//! staynb's stay page: a sticky `z-index:99` header, and a "Reserve stay" button in a sticky booking
//! panel further down. `Landing::OffScreen` asked for `dy = bbox.y - viewport.y`, i.e. the button's
//! top edge at the viewport's top edge, so the button landed under the header: the second `reach`
//! pass read it OCCLUDED by the header's guest picker, or the click hit the picker. Playwright
//! (scrollIntoViewIfNeeded) centres the target, which clears any header shorter than half the
//! viewport. Chrome 145 with the same click opens /book; manuk's act reported NO EFFECT.
//!
//! RED by the mutation that was the defect: top-align in `Landing::OffScreen` (see the HOW THIS GOES
//! RED note below).
use manuk_agent::AgentBrowser;

const HTML: &str = r#"<body style="margin:0;height:3000px">
<div style="position:sticky;top:0;z-index:99;height:100px;background:white"><button aria-label="Header picker" style="height:100px;width:800px">Header</button></div>
<button aria-label="Reserve stay" onclick="this.setAttribute('aria-label','Reserved')" style="position:absolute;left:40px;top:1500px;width:300px;height:45px">Reserve</button>
</body>"#;

#[tokio::test]
async fn a_scrolled_to_target_is_not_left_under_a_sticky_header() {
    let mut b = AgentBrowser::new(800, 600);
    b.navigate(&format!("data:text/html,{HTML}")).await.unwrap();

    // VACUITY: the target is below the fold and the header is a boxed, raised layer.
    {
        let tree = b.a11y_tree().unwrap();
        let t = tree
            .iter()
            .find(|n| n.name == "Reserve stay" && n.bbox.is_some())
            .expect("VACUOUS: no boxed Reserve stay button");
        assert!(
            t.bbox.unwrap().y > 600.0,
            "VACUOUS: the target is already on screen"
        );
        assert!(
            tree.iter()
                .any(|n| n.name == "Header picker" && n.z >= 99 && n.bbox.is_some()),
            "VACUOUS: the header is not a raised layer"
        );
    }

    let act = b
        .click_by_name(&manuk_a11y::Role::Button, "Reserve stay")
        .await
        .expect("a below-the-fold button under a sticky header's reach must be clickable after scrolling");
    let _ = act;
    let tree = b.a11y_tree().unwrap();
    assert!(
        tree.iter().any(|n| n.name == "Reserved"),
        "the click did not reach the Reserve button's own handler (scroll_y={})",
        b.scroll_offset()
    );
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
// N1  engine/a11y `Landing::OffScreen`: `dy = bbox.y - v.y` (top-align) for every target
//       -> the button scrolls under the 100px header; `click_by_name` errors "covered by" the header
//          button, or the handler never runs.
