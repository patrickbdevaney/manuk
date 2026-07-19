//! **G_POINTER_SEQUENCE — the menu that opens on `mousedown` never opened.**
//!
//! Tick 251 established that a double-click is a *sequence*. This is the same question one layer
//! down, and the answer was worse: `mousedown`, `mouseup` and `pointerdown` were dispatched
//! **nowhere in the engine** — zero hits.
//!
//! **Why that is bigger than it sounds.** A large class of real UI never listens for `click`.
//! Dropdown menus, comboboxes, drag handles, sliders and press-and-hold controls open on
//! `mousedown`, deliberately, so the menu is up before the button comes back up. Every one of them
//! was inert, and nothing threw to say so.
//!
//! ## How each assertion here can go RED
//!
//! - **The order.** `mousedown` → `mouseup` → `click`. RED, run: dispatch the pair *after*
//!   `dispatch_click_inner` and the sequence assertion fails while every "did the handler run"
//!   assertion stays green.
//!
//! - **`buttons` differs between down and up.** It is a mask of buttons *currently held*: 1 during
//!   `mousedown`, **0 during `mouseup`** — by then it has been released. RED, run: derive `buttons`
//!   from `button` for both (the obvious refactor) and `mouseup` reports 1, which is the shape of
//!   wrong that reads as right.
//!
//! - **`mousedown` does not cancel the click.** It suppresses focus and text selection; pages rely
//!   on that (a toolbar button preventing `mousedown` to keep the editor's selection alive still
//!   expects its click). RED, run: honour the `mousedown` verdict and the click stops firing on
//!   every such page.
//!
//! - **A label presses down ONCE, on the element under the pointer.** RED, run: have the label
//!   forwarding path re-enter `dispatch_click_detail` instead of `dispatch_click_inner` and the
//!   control receives a second `mousedown` it was never pressed with.

use manuk_page::Page;
use manuk_text::FontContext;

const W: f32 = 800.0;

/// One test — a `PageContext` is per-process, see `g_mouse_actuation.rs`.
#[test]
fn the_pointer_sequence_fires_in_order_with_a_truthful_buttons_mask() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<div id="t">target</div>
<input type="checkbox" id="c"><label for="c" id="l">Remember me</label>
<div id="log"></div>
<script>
  var seq = [];
  var downButtons = -1, upButtons = -1;
  // A menu that opens on mousedown and has NO click listener at all — the pattern this gate
  // exists for.
  var menuOpen = false;

  var t = document.getElementById('t');
  t.addEventListener('mousedown', function (e) {
    seq.push('down'); downButtons = e.buttons; menuOpen = true;
    e.preventDefault();          // suppress focus/selection — must NOT cancel the click
  });
  t.addEventListener('mouseup',   function (e) { seq.push('up'); upButtons = e.buttons; });
  t.addEventListener('click',     function ()  { seq.push('click'); });

  // The labelled control must be pressed down exactly ONCE, by the label, not twice.
  var controlDowns = 0;
  document.getElementById('c').addEventListener('mousedown', function () { controlDowns++; });
  document.getElementById('l').addEventListener('mousedown', function () { seq.push('labeldown'); });

  window.__report = function () {
    document.getElementById('log').textContent =
      'seq=' + seq.join('>') + ' downButtons=' + downButtons + ' upButtons=' + upButtons +
      ' menuOpen=' + menuOpen + ' controlDowns=' + controlDowns;
  };
</script></body>"#,
        "https://pointer.test/",
        &fonts,
        W,
    );

    let root = p.dom().root();
    let t = manuk_css::query_selector_all(p.dom(), root, "#t")[0];
    let l = manuk_css::query_selector_all(p.dom(), root, "#l")[0];
    let lg = manuk_css::query_selector_all(p.dom(), root, "#log")[0];

    p.dispatch_click(t, &fonts, W);
    p.dispatch_click(l, &fonts, W);
    p.eval_for_test("window.__report();");
    let out = p.dom().text_content(lg);

    assert!(
        out.contains("seq=down>up>click"),
        "the pointer sequence is mousedown > mouseup > click, in that order. got: {out}"
    );
    assert!(
        out.contains("menuOpen=true"),
        "A MENU THAT OPENS ON mousedown AND HAS NO CLICK LISTENER MUST OPEN — this is the whole \
         reason the gate exists, and before this tick it was inert with nothing thrown. got: {out}"
    );
    assert!(
        out.contains("downButtons=1"),
        "the primary button is held during mousedown; got: {out}"
    );
    assert!(
        out.contains("upButtons=0"),
        "`buttons` is a mask of buttons CURRENTLY HELD, so it is 0 on mouseup — the button has \
         been released by then. Deriving it from `button` for both events gives 1 here, which is \
         the shape of wrong that reads as right. got: {out}"
    );
    assert!(
        out.contains("controlDowns=0"),
        "a click on a <label> presses the mouse down ONCE, on the element actually under the \
         pointer. The label forwards only the CLICK to its control — the control was never \
         pressed. got: {out}"
    );
    assert!(
        out.contains("labeldown"),
        "...and the label itself DID receive the press, so the claim above is not vacuous. \
         got: {out}"
    );

    // `preventDefault()` on mousedown suppressed focus/selection but must not have eaten the click
    // — asserted by `seq` containing `click` above, and restated here as the named claim.
    assert!(
        out.contains(">click"),
        "a preventDefault() on mousedown suppresses focus and selection, NOT the click; got: {out}"
    );
}
