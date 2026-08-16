//! **G_CLIENT_COORDS — `getBoundingClientRect` is CLIENT-relative, not document-relative.**
//!
//! The `Client` in the name is the whole specification: CSSOM View defines the returned rect
//! relative to the **viewport**, so on a page scrolled to `y = 300` an element sitting at document
//! `y = 500` reports `top === 200`. Ours reported `500` — the raw document coordinate, because
//! `layout_rect` reads the layout snapshot and nothing ever subtracted the scroll.
//!
//! ⚠⚠⚠ **A WRONG ANSWER OF THE RIGHT TYPE, and it is exactly zero percent wrong until the page
//! scrolls.** Every test that measures at scroll 0 passes, which is nearly all of them, so the
//! defect is invisible to the entire gate wall and to most of WPT. It becomes 100% wrong the moment
//! a user scrolls, and what it breaks is the single most common measurement idiom on the web:
//!
//! ```js
//!   if (el.getBoundingClientRect().top <= 0) header.classList.add('is-stuck');   // never fires
//!   var inView = r.top < innerHeight && r.bottom > 0;                            // always true
//!   var docY   = r.top + window.scrollY;                                         // double-counts
//! ```
//!
//! That last line is the tell, and it is why the two APIs cannot be checked independently:
//! `rect.top + window.scrollY` is *the* documented way to get a document coordinate, so `scrollY`
//! and `getBoundingClientRect` are only correct **together**. Ours had a truthful `scrollY` and a
//! document-relative rect, so the idiom returned `y + scroll` — off by exactly one scroll offset,
//! in the direction that looks plausible.
//!
//! **To watch it go RED:** drop the `- sx` / `- sy` from `el_get_bounding_rect` (or from
//! `el_get_client_rects`) and the scrolled rows below report the document coordinates again.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div style="height:500px">spacer</div>
<div id="target" style="height:40px">target</div>
<div style="height:3000px">tail</div>
<div id="out">-</div>
<script>
  var R = [], t = document.getElementById('target');

  // Unscrolled, client and document coordinates coincide — stated first so the scrolled rows
  // cannot be luck, and so a fix that simply subtracts something everywhere fails HERE.
  R.push('top0:' + Math.round(t.getBoundingClientRect().top));   // 500
  R.push('rects0:' + Math.round(t.getClientRects()[0].top));     // 500 — same API family

  window.scrollTo(0, 300);
  R.push('sy:' + Math.round(window.scrollY));                    // 300
  R.push('top1:' + Math.round(t.getBoundingClientRect().top));   // 200 — CLIENT-relative
  R.push('bot1:' + Math.round(t.getBoundingClientRect().bottom));// 240 — bottom moves with it
  R.push('rects1:' + Math.round(t.getClientRects()[0].top));     // 200

  // THE IDIOM. `rect.top + scrollY` is the documented way back to a document coordinate, so the
  // two APIs are only correct together — this row is what makes that a claim rather than an aside.
  R.push('doc:' + Math.round(t.getBoundingClientRect().top + window.scrollY));  // 500

  // Width/height are NOT positions and must not move.
  R.push('h:' + Math.round(t.getBoundingClientRect().height));   // 40

  // offsetTop is offsetParent-relative, NOT client-relative — it must NOT have changed. Without
  // this row a fix that subtracted the scroll from the shared `layout_rect` would pass everything
  // above while silently breaking every `offsetTop`-based measurement on the web.
  R.push('off:' + Math.round(t.offsetTop));                      // 500

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

// ⚠ ONE `#[test]` fn per JS gate binary — two live `PageContext`s on two threads fault SpiderMonkey.
#[test]
fn get_bounding_client_rect_is_relative_to_the_viewport() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://clientcoords.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "top0:500",
            "unscrolled, client and document coordinates coincide — the control that stops a blanket \
             subtraction from passing",
        ),
        (
            "rects0:500",
            "getClientRects agrees with getBoundingClientRect at scroll 0; they are one API family \
             and must not drift apart",
        ),
        (
            "sy:300",
            "window.scrollY reports the scroll synchronously — the other half of the idiom, and \
             already true before this gate",
        ),
        (
            "top1:200",
            "THE GATE. 500 document minus 300 scrolled = 200 in the VIEWPORT. Reading 500 here is \
             what makes `rect.top <= 0` never fire for a sticky header and `rect.top < innerHeight` \
             always true for a lazy-load check",
        ),
        (
            "bot1:240",
            "every edge moves, not just `top` — a rect with a client top and a document bottom is a \
             worse object than either",
        ),
        (
            "rects1:200",
            "getClientRects moves with it; an animation library that feature-detects on \
             getClientRects().length and then measures with it must get the same coordinate space",
        ),
        (
            "doc:500",
            "`rect.top + window.scrollY` is the documented way back to a document coordinate. It \
             read 800 before — off by exactly one scroll offset, in the direction that looks \
             plausible",
        ),
        ("h:40", "a dimension is not a position and must not move"),
        (
            "off:500",
            "offsetTop is offsetParent-relative and must NOT change. This row is the one that fails \
             if the scroll is subtracted from the shared `layout_rect` instead of from the two \
             client-coordinate APIs",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_CLIENT_COORDS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
