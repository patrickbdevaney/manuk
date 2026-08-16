//! **G_DYNAMIC_SCROLLER — a scroll container the SCRIPT created could not be scrolled.**
//!
//! `el.scrollTop = n` is clamped in the bindings against the published scroll geometry —
//! *"max = content extent − visible window"* — so that a script writing `1e9` to reach the bottom
//! reads back the real maximum. That map is published by the host **before the script runs**. For an
//! element the script created *this round* there is no entry at all, so `max` is `0` and the
//! assignment is clamped to **zero**.
//!
//! ⚠⚠⚠ **THE CLAMP IS CORRECT, ITS INPUT IS STALE, AND THE RESULT IS A SCROLL THAT SILENTLY DOES
//! NOTHING.** No throw, no warning, and `scrollTop` reads back `0` — which is a legal value, so even
//! a careful caller checking its own write sees nothing wrong.
//!
//! What it costs is every SPA that builds a scroll container and then scrolls it in the same task: a
//! chat pane jumping to the newest message, a virtualised list restoring position, a carousel moving
//! to the active slide, a modal scrolling its body to the top.
//!
//! ⭐ **A SECOND, INDEPENDENT INSTANCE OF THE SAME CLASS SITS BEHIND IT.** Once the scroll worked, a
//! `position: sticky` child *still* did not stick — because `has_sticky` is a flag derived from the
//! cascade at `Page` construction and captured again when the reflow scope is armed, i.e. **before**
//! the script created the sticky element. Two values derived from a snapshot; one thing created after
//! the snapshot was taken. The reflow now asks its own fresh cascade instead.
//!
//! The fixture is WPT's own `css/css-position/resources/sticky-util.js` structure, rebuilt inline —
//! that harness builds its entire DOM with `createElement`/`appendChild`, which is why the whole
//! sticky suite's scroll-container family was affected by a defect that has nothing to do with sticky.
//!
//! **To watch it go RED, one mutation per arm:**
//!
//! 1. drop the `set_scroll_geometry` republish from `forced_reflow` → `unstuck`/`stuck` report the
//!    unscrolled position and `clamp` reads `0`;
//! 2. drop `force_reflow_if_stale()` from `el_set_scroll_axis` → the same, by the other route;
//! 3. use `c.has_sticky` instead of the fresh cascade in `forced_reflow` → only `stuck` fails, which
//!    is what makes the sticky arm a separate claim rather than a restatement.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<script>
  // WPT's sticky-util.js shape: scroller 100x200 (overflow:hidden) > contents 100x500 >
  // prepadding 100x100 + container 300x300 > filler 100x100 + sticky 100x100 (top:50).
  function mk() {
    var s = document.createElement('div');
    s.style.position = 'relative'; s.style.width = '100px';
    s.style.height = '200px'; s.style.overflow = 'hidden';
    var c = document.createElement('div'); c.style.height = '500px'; c.style.width = '100%';
    var p = document.createElement('div'); p.style.height = '100px'; p.style.width = '100%';
    var ct = document.createElement('div'); ct.style.height = '300px'; ct.style.width = '300px';
    var f = document.createElement('div'); f.style.height = '100px'; f.style.width = '100%';
    var k = document.createElement('div'); k.style.height = '100px'; k.style.width = '100%';
    k.style.position = 'sticky'; k.style.top = '50px';
    ct.appendChild(f); ct.appendChild(k); c.appendChild(p); c.appendChild(ct);
    s.appendChild(c); document.body.appendChild(s);
    return { s: s, k: k };
  }
  var R = [];

  // Scrolled 100: the sticky box's own flow position (200 into the contents) is still below the
  // `top:50` threshold, so it has NOT stuck and sits 100 below the scroller's top edge.
  var e1 = mk(); e1.s.scrollTop = 100;
  R.push('unstuck=' + Math.round(e1.k.getBoundingClientRect().y - e1.s.getBoundingClientRect().y));
  R.push('readback=' + Math.round(e1.s.scrollTop));

  // Scrolled 200: past the threshold, so it pins at 50 below the scroller's top edge.
  var e2 = mk(); e2.s.scrollTop = 200;
  R.push('stuck=' + Math.round(e2.k.getBoundingClientRect().y - e2.s.getBoundingClientRect().y));

  // The clamp itself must still work — and against the REAL maximum: 500 content − 200 viewport.
  var e3 = mk(); e3.s.scrollTop = 1e9;
  R.push('clamp=' + Math.round(e3.s.scrollTop));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

// ⚠ ONE `#[test]` fn per JS gate binary — two live `PageContext`s on two threads fault SpiderMonkey.
#[test]
fn a_script_created_scroll_container_can_be_scrolled() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://dynscroll.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "unstuck=100",
            "THE GATE. The scroller was created by the script and then scrolled 100px in the same \
             task. This read 200 — the unscrolled position — because the write was clamped to zero \
             against a geometry map published before the element existed",
        ),
        (
            "readback=100",
            "and the script reads its own write back. `scrollTop` returning 0 is a LEGAL value, so \
             even a caller that checks its own assignment saw nothing wrong — which is what made \
             this silent",
        ),
        (
            "stuck=50",
            "the SECOND, independent arm: a `position: sticky` child created by the same script must \
             pin at its 50px threshold. `has_sticky` is derived from the cascade at construction and \
             re-read when the reflow scope is armed — both BEFORE this element existed — so the \
             sticky pass was skipped for exactly the element being measured",
        ),
        (
            "clamp=300",
            "CONTROL — the clamp must still clamp, and against the REAL maximum (500 content − 200 \
             viewport). A fix that simply stopped clamping would pass every row above and let \
             `scrollTop = 1e9` read back a billion",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_DYNAMIC_SCROLLER: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
