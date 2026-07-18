//! G_SCROLL_ANCHOR_LIVE — anchoring holds across the REAL delivery path a feed actually uses.
//!
//! `g_scroll_anchor` proves the mechanism against a click handler. But the jump users complain about
//! comes from the network: a lazy image, a late ad, or the next page of a feed arriving over
//! `fetch()` and being appended *above* what they are reading. That path is
//! `Page::deliver_fetch_stream`, and the shell wraps it in `with_scroll_anchor`.
//!
//! This gate does exactly what `gui.rs::with_scroll_anchor` does, in the same order, around the same
//! delivery call — capture, deliver, measure, apply — so the composition is proven even though the
//! shell itself has no UI harness to test through. If the mechanism and the delivery path disagreed
//! about when geometry is valid, this fails where the unit gate passes.

use manuk_page::FetchStreamEvent;
use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body style="margin:0">
  <div id="top" style="height:600px">above the fold</div>
  <div id="slot"></div>
  <div id="read" style="height:50px">the line being read</div>
  <div style="height:2000px">more feed</div>
  <script>
    // The classic feed pattern: fetch, then inject the result ABOVE the reading position.
    fetch('/ad').then(function(r) { return r.text(); }).then(function(html) {
      var ad = document.createElement('div');
      ad.setAttribute('style', 'height:' + html + 'px');
      ad.textContent = 'late ad';
      document.getElementById('slot').appendChild(ad);
    });
  </script>
</body></html>"#;

fn y_of(page: &manuk_page::Page, node: manuk_dom::NodeId) -> f32 {
    page.node_rects().get(&node).expect("node has a box").y
}

#[test]
fn a_late_fetch_that_grows_the_page_above_you_does_not_move_your_line() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://feed.test/", &fonts, 800.0);
    let read = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#read")[0];

    // The reader is parked with #read at the top of the viewport.
    let mut scroll_y = 600.0_f32;
    let screen_before = y_of(&page, read) - scroll_y;
    assert!(
        screen_before.abs() < 1.0,
        "precondition: #read starts at the viewport top ({screen_before})"
    );

    let reqs = page.take_fetches();
    assert_eq!(reqs.len(), 1, "the page fetched its ad");
    let id = reqs[0].0;

    // ── Exactly what `gui.rs::with_scroll_anchor` does around a delivery. ────────────────────
    let anchor = page
        .capture_scroll_anchor(scroll_y)
        .expect("an element sits at the viewport top");

    for event in [
        FetchStreamEvent::Head {
            status: 200,
            headers: vec![],
        },
        FetchStreamEvent::Chunk(b"300".to_vec()),
        FetchStreamEvent::End,
    ] {
        page.deliver_fetch_stream(id, &event, &fonts, 800.0);
    }

    let y_after = y_of(&page, read);
    assert!(
        (y_after - 900.0).abs() < 1.0,
        "precondition: the fetched ad really did push #read down by 300px (y={y_after})"
    );

    let delta = page.scroll_anchor_delta(&anchor, scroll_y);
    if delta.abs() > 0.5 {
        scroll_y += delta;
    }

    let screen_after = y_after - scroll_y;
    assert!(
        (screen_after - screen_before).abs() < 1.0,
        "G_SCROLL_ANCHOR_LIVE: a fetch that grew the document above the reader must not move the \
         reader. before={screen_before}px after={screen_after}px (delta={delta}). This is the \
         path a real feed takes, and the one the shell wraps."
    );
}
