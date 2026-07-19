//! **G_IFRAME_CLICK — the user can press a button inside a frame.**
//!
//! OAuth **O2**, second half. Tick 232 made a frame's pixels follow its document, but only a
//! *script* could change it: the host hit-tests the parent, gets the `<iframe>` ELEMENT, and
//! dispatching a click on that fires a click on the frame **box** — not on whatever the user
//! actually pressed. So an embedded form re-rendered correctly and still could not be operated.
//!
//! That is the whole point of the content the web puts in frames: a **3-D Secure challenge** has an
//! "approve" button, an **embedded OAuth consent screen** has "allow", a payment form has "pay".
//! Rendering them is not the capability — pressing them is.
//!
//! **The mechanism.** A frame is a separate document painted into a bitmap, so a click has to be
//! translated into the child's own coordinate space and hit-tested *there*. The child also lays out
//! at the FRAME's width, so it must be clicked at that width; using the window's would hit-test
//! against a layout the child never had.
//!
//! **What is asserted:** a click at a document point that falls inside the frame reaches the child's
//! button — its handler runs and mutates the child's DOM — and the frame's **pixels** change, so the
//! result is on screen and not merely in the child's tree. Then the negative: a click at a point
//! *outside* the frame must NOT reach it, which is what proves the routing is positional rather than
//! "any click goes to the frame".
//!
//! RED, run: dispatching on the hit node (the pre-232 behaviour) leaves the child on `pending` — the
//! frozen challenge exactly.

use manuk_text::FontContext;

/// The bank's challenge, with a real button to press.
const FRAME_HTML: &str = r#"<!doctype html>
<html><body style="margin:0;background:#ffffff">
  <button id="approve" style="width:200px;height:100px;background:#ffffff">approve</button>
  <div id="status">pending</div>
  <script>
    document.getElementById('approve').addEventListener('click', function () {
      document.getElementById('status').textContent = 'approved';
      // Repaint the BUTTON, not the body: the button has a box inside the frame's 200x100
      // viewport, so a colour change there is unambiguously visible in the frame's bitmap.
      var b = document.getElementById('approve');
      b.style.background = '#00ff00';
      b.textContent = 'approved';
    });
  </script>
</body></html>"#;

const PARENT_HTML: &str = r#"<!doctype html>
<html><body style="margin:0">
  <iframe id="f" width="200" height="100"></iframe>
  <div id="outside" style="width:200px;height:50px">outside</div>
</body></html>"#;

fn frame_ink(page: &manuk_page::Page, node: manuk_dom::NodeId) -> u64 {
    page.image_for(node)
        .map(|img| img.rgba.iter().map(|&b| b as u64).sum())
        .unwrap_or(0)
}

/// The child's `#status` text — what the bank's own document believes happened.
fn child_status(page: &manuk_page::Page, frame: manuk_dom::NodeId) -> String {
    let child = page
        .child_page(frame)
        .expect("the frame's document must be live");
    let root = child.dom().root();
    let n = manuk_css::query_selector_all(child.dom(), root, "#status")[0];
    child.dom().text_content(n).trim().to_string()
}

#[test]
fn a_click_inside_a_frame_reaches_the_frames_button() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(PARENT_HTML, "https://shop.test/", &fonts, 800.0);
    let root = page.dom().root();
    let frame = manuk_css::query_selector_all(page.dom(), root, "#f")[0];
    page.render_iframe(frame, FRAME_HTML, "https://bank.test/3ds", &fonts, 0);

    let before_ink = frame_ink(&page, frame);
    assert!(
        before_ink > 0,
        "the frame must paint at all before we click it"
    );
    assert_eq!(
        child_status(&page, frame),
        "pending",
        "the challenge starts unapproved — otherwise the assertion below proves nothing"
    );

    // ── A click well inside the frame's box, in PARENT document coordinates. The frame is at the
    // document origin and is 200x100, so (100, 50) is its centre — squarely on the button.
    page.dispatch_click_at(100.0, 50.0, &fonts, 800.0);

    assert_eq!(
        child_status(&page, frame),
        "approved",
        "the click must reach the button INSIDE the frame — dispatching on the hit node fires a \
         click on the frame BOX instead, which is not what the user pressed, and leaves an embedded \
         payment or consent form impossible to operate"
    );
    assert_ne!(
        before_ink,
        frame_ink(&page, frame),
        "and the result must be ON SCREEN: the child's handler repainted its background, so the \
         parent's bitmap for the frame must have changed too"
    );

    // ── The negative. A click below the frame must not reach it — routing is positional, not
    // "every click goes to the frame".
    let mut page2 = manuk_page::Page::load(PARENT_HTML, "https://shop.test/", &fonts, 800.0);
    let root2 = page2.dom().root();
    let frame2 = manuk_css::query_selector_all(page2.dom(), root2, "#f")[0];
    page2.render_iframe(frame2, FRAME_HTML, "https://bank.test/3ds", &fonts, 0);
    page2.dispatch_click_at(100.0, 130.0, &fonts, 800.0); // below the 100px-tall frame
    assert_eq!(
        child_status(&page2, frame2),
        "pending",
        "a click outside the frame's box must not be delivered into it"
    );
}
