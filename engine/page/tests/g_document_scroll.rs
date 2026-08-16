//! **G_DOCUMENT_SCROLL — `window.scrollTo` is a layout input, not just a number to read back.**
//!
//! `host_scroll_to` pushes the request onto a queue for the host and — since tick 378 —
//! *optimistically* sets the scroll so `window.scrollY` reads back on the very next line. That
//! optimism is precisely what hid the gap: **the one observable anybody checks first is correct**,
//! while everything downstream of the scroll went on describing the unscrolled page. And
//! `Page::take_scroll_requests` has **no callers** — `grep -rn take_scroll_requests shell/ tests/`
//! is empty — so nothing ever performed the scroll either.
//!
//! Three things depend on the document scroll and none of them learned:
//!
//! 1. the **forced reflow's staleness guard** (t1283) — bumped by `el.scrollTop`, not by
//!    `window.scrollTo`, so no relayout happened at all;
//! 2. the **sticky constraint** (t1282) — resolved against the `Page`'s committed scroll, which is by
//!    definition the scroll *before* the one the script just made;
//! 3. `IntersectionObserver`'s `boundingClientRect` subtracts `scrollY` and not `scrollX` — and that
//!    third arm was **REFUSED here after being written**, because it is unfalsifiable: see below.
//!
//! This is `css/css-position/sticky`'s document-scroller family in one sentence: *"Sticky elements
//! work with the root (document) scroller — expected 750 but got 8"*, where 8 is the UA body margin,
//! i.e. the box had not moved at all.
//!
//! **To watch it go RED, three ways, one per arm:**
//!
//! 1. drop the `SCROLL_SEQ` bump from `host_scroll_to` → no reflow fires; `stuck:` reports the
//!    element's unscrolled position;
//! 2. use `c.sticky_scroll_y` instead of the live scroll in `forced_reflow` → the reflow runs and
//!    re-sticks against the OLD scroll, so `stuck:` lands short by exactly the scroll delta;
//!
//! ⚠⚠⚠ **THE THIRD ARM WAS WRITTEN, MEASURED, AND TAKEN BACK OUT.** An `IntersectionObserver` entry's
//! rect is client-relative on both axes, so `- scrollX` looks like a one-token fix. It is provably
//! INERT: `PageContext::view_changed` is the only caller of `__runObservers`, and it opens with
//! `SCROLL.set((0.0, scroll_y))` — it **zeroes the horizontal scroll before every observer pass**,
//! because the host's view-changed signature has no `scroll_x` to carry. The gate row for it read
//! `iox:none` (the observer never ran without a host-driven pass) and could not have been made to
//! fail for the right reason. **A half-true arm is worse than a missing one** (t1280); the real fix
//! is a layer up, in what the host tells the page about its own viewport.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="sec" style="height:4000px">
  <div id="hdr" style="position:sticky;top:0;height:40px">Header</div>
  <div style="height:3960px">body</div>
</div>
<div id="out">-</div>
<script>
  var R = [], hdr = document.getElementById('hdr');

  // Baseline: unscrolled, the header sits at the top of the document and of the screen.
  R.push('nat:' + Math.round(hdr.getBoundingClientRect().top));   // 0

  window.scrollTo(0, 750);
  R.push('sy:' + Math.round(window.scrollY));                     // 750 — already true before t1286

  // THE GATE. The header is `top:0` sticky inside a 4000px section, so at scroll 750 it is stuck to
  // the top of the VIEWPORT: client top 0, document y 750. Both numbers are asserted, because a
  // client top of 0 is also what an unstuck header at document 0 would report if the client
  // conversion silently used a stale scroll — the pair pins it.
  R.push('stuck:' + Math.round(hdr.getBoundingClientRect().top));            // 0  (client)
  R.push('doc:' + Math.round(hdr.getBoundingClientRect().top + window.scrollY)); // 750 (document)

  // The non-sticky sibling below it must have moved the full 750 — the control that says the page
  // really scrolled rather than the header simply never moving.
  var body = hdr.nextElementSibling;
  R.push('ctl:' + Math.round(body.getBoundingClientRect().top));  // 40 - 750 = -710

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

// ⚠ ONE `#[test]` fn per JS gate binary — two live `PageContext`s on two threads fault SpiderMonkey.
#[test]
fn a_document_scroll_reaches_layout_not_just_the_readback() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://docscroll.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "nat:0",
            "unscrolled baseline — a later client top of 0 is only meaningful if the DOCUMENT number \
             beside it moved",
        ),
        (
            "sy:750",
            "window.scrollY reads back the script's own scroll. This was ALREADY true (t378) and is \
             exactly what hid the rest: the first thing anyone checks was right",
        ),
        (
            "stuck:0",
            "THE GATE. A `top:0` sticky header inside a 4000px section is pinned to the top of the \
             VIEWPORT at scroll 750",
        ),
        (
            "doc:750",
            "...and its DOCUMENT position is 750, which is what says it really moved rather than the \
             client conversion being applied to a stale scroll. `expected 750 but got 8` is this row \
             in WPT's words",
        ),
        (
            "ctl:-710",
            "the non-sticky sibling moved the full 750 (40 - 750). Without this the rows above pass \
             for a page that never scrolled and a header that never stuck",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_DOCUMENT_SCROLL: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
