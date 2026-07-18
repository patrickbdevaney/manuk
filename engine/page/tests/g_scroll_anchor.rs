//! G_SCROLL_ANCHOR — the feed stops jumping when something loads above what you are reading.
//!
//! **The failure this gate exists for.** A feed loads an image, an ad, or the next page of posts
//! *above* the user's reading position. The document gets taller above them, every following box
//! shifts down by that height, and the line they were mid-sentence on jumps off the screen. On an
//! infinite feed that fires on every load, which is why it is one of the most complained-about
//! behaviours on the mobile web — and why every engine implements scroll anchoring.
//!
//! The claim is falsifiable and geometric: **after content is inserted above the fold, the element
//! the user was looking at must be at the same screen position it was before.** Without a
//! correction it is exactly `inserted_height` pixels lower, which is what this asserts against.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body style="margin:0">
  <div id="top" style="height:600px">above the fold</div>
  <div id="slot"></div>
  <div id="read" style="height:50px">the line being read</div>
  <div style="height:2000px">more feed</div>
  <button id="load">load</button>
  <script>
    // A late-loading ad/image/next-page lands ABOVE what the user is reading.
    document.getElementById('load').addEventListener('click', function() {
      var ad = document.createElement('div');
      ad.setAttribute('style', 'height:300px');
      ad.textContent = 'late-loading ad';
      document.getElementById('slot').appendChild(ad);
    });
  </script>
</body></html>"#;

fn y_of(page: &manuk_page::Page, node: manuk_dom::NodeId) -> f32 {
    page.node_rects().get(&node).expect("node has a box").y
}

#[test]
fn content_loading_above_the_fold_does_not_move_what_you_are_reading() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://feed.test/", &fonts, 800.0);
    let root = page.dom().root();
    let read = manuk_css::query_selector_all(page.dom(), root, "#read")[0];

    // The user has scrolled so that #read sits near the top of the viewport.
    let scroll_y = 600.0_f32;
    let y_before = y_of(&page, read);
    let screen_before = y_before - scroll_y;
    assert!(
        screen_before.abs() < 1.0,
        "precondition: #read starts at the viewport top (screen y {screen_before})"
    );

    // Capture the anchor, exactly as a host does before a mutation that may reflow.
    let anchor = page
        .capture_scroll_anchor(scroll_y)
        .expect("something is at the top of the viewport");

    // An ad/image/next-page loads ABOVE the fold and the document grows there.
    let load = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#load")[0];
    page.dispatch_click(load, &fonts, 800.0);

    let y_after = y_of(&page, read);
    assert!(
        (y_after - y_before - 300.0).abs() < 1.0,
        "precondition: the insertion really did push #read down by its height \
         (before {y_before}, after {y_after})"
    );

    // Uncorrected, the user's line is now 300px lower on screen — the jump.
    let uncorrected = y_after - scroll_y;
    assert!(
        (uncorrected - 300.0).abs() < 1.0,
        "without anchoring the read line jumps down by the inserted height ({uncorrected}px)"
    );

    // THE CLAIM: applying the delta puts it back exactly where it was.
    let delta = page.scroll_anchor_delta(&anchor, scroll_y);
    let corrected_scroll = scroll_y + delta;
    let screen_after = y_after - corrected_scroll;
    assert!(
        (screen_after - screen_before).abs() < 1.0,
        "G_SCROLL_ANCHOR: after anchoring, the line the user was reading must be at the SAME \
         screen position. before={screen_before}px after={screen_after}px (delta={delta}). \
         A feed that fails this jumps under the reader's eyes on every lazy-loaded image."
    );
    assert!(
        (delta - 300.0).abs() < 1.0,
        "the correction equals the height inserted above the fold ({delta})"
    );

    // A relayout that changes nothing above the fold must NOT move the page.
    let anchor2 = page.capture_scroll_anchor(corrected_scroll).unwrap();
    let quiet = page.scroll_anchor_delta(&anchor2, corrected_scroll);
    assert!(
        quiet.abs() < 0.001,
        "no growth above the fold means no correction — anchoring must be inert when nothing \
         moved, or it becomes its own source of drift (got {quiet})"
    );
}
