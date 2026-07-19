//! **G_IFRAME_RERENDER — a frame whose document changes must change on screen.**
//!
//! OAuth **O2**, and the constellation's row 50: *"REAL iframes … BUT pixels are a snapshot — no
//! re-render on mutation. Blocks 3-D Secure challenge + interactive OAuth frames."*
//!
//! **The mechanism.** `render_iframe` builds the child document in full, paints it once into a
//! bitmap, and stores that bitmap in the parent's image map — the same map an `<img>` lands in. The
//! child `Page` is kept alive (that is what made `contentDocument` work), so scripts can reach in and
//! mutate it. Nothing re-paints it. The DOM changes; the pixels do not.
//!
//! **Why this specific gap is expensive.** The frame is exactly where the web puts things it will not
//! let the parent touch: a **3-D Secure challenge** (the bank's "approve this payment" step), an
//! **embedded OAuth consent screen**, a payment form, a CAPTCHA. All of them are *interactive by
//! definition* — they change in response to what the user does. A frame that renders its first state
//! forever shows the challenge and never shows the result, so the payment or the login cannot be
//! completed at all; the user sees a form that appears frozen.
//!
//! **What is asserted:** the frame's document is reachable and mutable from the parent (already
//! true), and — the actual claim — the parent's stored pixels for that frame **differ after the
//! mutation**. Pixels, not DOM state, because the DOM half already worked and is precisely what made
//! the gap invisible: everything reads correct and the screen is wrong.

use manuk_text::FontContext;

const FRAME_HTML: &str = r#"<!doctype html>
<html><body style="margin:0;background:#ffffff">
  <div id="challenge" style="width:200px;height:100px;background:#ffffff">approve</div>
</body></html>"#;

const PARENT_HTML: &str = r#"<!doctype html>
<html><body style="margin:0">
  <iframe id="f" width="200" height="100"></iframe>
  <button id="go">approve</button>
  <div id="out">-</div>
  <script>
    // Driven by a REAL click, not a test-only eval, so the gate exercises the path a page actually
    // takes: an event handler reaches into the frame and changes it.
    document.getElementById('go').addEventListener('click', function () {
      var d = document.getElementById('f').contentDocument;
      var c = d.getElementById('challenge');
      c.style.background = '#00ff00';
      c.textContent = 'approved';
      document.getElementById('out').textContent = 'mutated';
    });
  </script>
</body></html>"#;

/// Sum of the RGBA bytes of the frame's stored bitmap — a cheap total that cannot stay equal if the
/// frame repainted a large area in a different colour.
fn frame_ink(page: &manuk_page::Page, node: manuk_dom::NodeId) -> Option<u64> {
    page.image_for(node)
        .map(|img| img.rgba.iter().map(|&b| b as u64).sum())
}

#[test]
fn a_frames_pixels_follow_its_document() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(PARENT_HTML, "https://shop.test/", &fonts, 800.0);
    let root = page.dom().root();
    let frame = manuk_css::query_selector_all(page.dom(), root, "#f")[0];

    // Load the frame's document the way the sync path does (no async runtime in a gate).
    page.render_iframe(frame, FRAME_HTML, "https://bank.test/3ds", &fonts, 0);

    let before = frame_ink(&page, frame).expect(
        "the frame must paint at all — without a bitmap there is nothing on screen for the embed",
    );

    // The bank's challenge resolves: its document turns green. Driven through the parent, which is
    // what an OAuth/3DS flow does when it posts a result into the frame.
    // The challenge resolves: a real dispatched click runs the handler that mutates the frame —
    // the same path a 3-D Secure or embedded OAuth flow takes when it posts its result in.
    let go = manuk_css::query_selector_all(page.dom(), root, "#go")[0];
    page.dispatch_click(go, &fonts, 800.0);

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    assert_eq!(
        page.dom().text_content(out).trim(),
        "mutated",
        "the handler must have run and reached into the frame — otherwise the pixel claim below \
         would be testing nothing"
    );

    let after = frame_ink(&page, frame).expect("the frame must still have a bitmap after mutation");

    assert_ne!(
        before, after,
        "the frame's pixels must follow its document\n\n  \
         Equal totals mean the bitmap is still the FIRST paint. The DOM changed and the screen did \
         not — which is why this gap is invisible from script: every read comes back correct. A \
         3-D Secure challenge or an embedded OAuth consent screen shows its initial state forever, \
         so the payment or the login can never be completed."
    );
}
