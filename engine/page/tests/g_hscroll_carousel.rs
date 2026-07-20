//! **G_HSCROLL_CAROUSEL — a horizontal rail reports the scroll range Chrome reports.**
//!
//! Every product carousel, poster row, tab strip and chip bar on the web is one of two shapes: a
//! `white-space: nowrap` row of `inline-block`s, or a `display: flex` row whose items are stopped
//! from shrinking. If the container reports no horizontal scroll range, the user cannot reach the
//! second item — not by wheel, not by drag, not by keyboard — and nothing errors.
//!
//! This gate exists because a **stale defect note** claimed exactly that failure ("an inline-block
//! row yields no horizontal scroll range, `max_x = 0`"). It had been fixed, and nothing recorded
//! that it had. An unpinned fix is indistinguishable from an open bug, and the next reader spends a
//! tick re-diagnosing it. So the numbers below are pinned, and they are **not** our own output
//! played back: each was read out of headless Chrome 145 at 1280×720 on the identical markup.
//!
//! ```
//!                                    Chrome     here
//!   flex, items may shrink          300/300   300/300
//!   flex, flex-shrink: 0           1000/300  1000/300
//!   inline-block + nowrap          1000/300  1000/300
//! ```
//!
//! ## The shrinking case is the one worth asserting
//!
//! `flexDefault` looks like the bug and is the correct answer. A flex item defaults to
//! `flex-shrink: 1`, and `min-width: auto` only floors it at its *min-content* width — these cards
//! hold one digit, so Chrome squeezes five 200px cards into 300px and there is genuinely nothing to
//! scroll. A carousel that works is one whose author wrote `flex-shrink: 0`.
//!
//! Asserting it keeps a future "fix" honest: making rails scroll by ignoring shrink would satisfy
//! the two positive claims and fail this one. Without it, the gate would accept an engine that
//! never shrinks flex items at all.
//!
//! ## The RED probe (run, not imagined)
//!
//! Clamping `content_extent`'s width to the container's client width — the shape of the original
//! defect — flips `flexShrink0` and `inlineNowrap` to 300 while `flexDefault` stays green.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  .rail { overflow: auto; display: flex; width: 300px }
  .card { display: inline-block; width: 200px; height: 100px }
  .noshrink .card { flex-shrink: 0 }
  .inline { overflow-x: auto; white-space: nowrap; width: 300px }
</style></head><body>
  <div class="rail" id="flexDefault"><div class="card">1</div><div class="card">2</div><div class="card">3</div><div class="card">4</div><div class="card">5</div></div>
  <div class="rail noshrink" id="flexShrink0"><div class="card">1</div><div class="card">2</div><div class="card">3</div><div class="card">4</div><div class="card">5</div></div>
  <div class="inline" id="inlineNowrap"><div class="card">1</div><div class="card">2</div><div class="card">3</div><div class="card">4</div><div class="card">5</div></div>
</body></html>"##;

#[test]
fn horizontal_rails_report_the_scroll_range_chrome_reports() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://rail.test/", &fonts, 1280.0);
    let dom = page.dom();

    // (scrollWidth, clientWidth), each read out of headless Chrome 145 at 1280x720.
    for (id, want_scroll, want_client, why) in [
        (
            "flexDefault",
            300.0,
            300.0,
            "flex items default to `flex-shrink: 1` and `min-width: auto` floors them only at \
             min-content, so five 200px one-digit cards genuinely fit in 300px and there is \
             nothing to scroll. This is the claim a 'make rails scroll' fix would break",
        ),
        (
            "flexShrink0",
            1000.0,
            300.0,
            "`flex-shrink: 0` is how a real carousel is built — the rail must report 700px of \
             horizontal range or the user cannot reach the second card by any means",
        ),
        (
            "inlineNowrap",
            1000.0,
            300.0,
            "the other real carousel shape, a `white-space: nowrap` row of inline-blocks — this \
             is the one a stale note claimed reported no range at all",
        ),
    ] {
        let n = manuk_css::query_selector_all(dom, dom.root(), &format!("#{id}"))[0];
        let g = page
            .scroll_geometry(n)
            .unwrap_or_else(|| panic!("G_HSCROLL_CAROUSEL {id}: no scroll geometry — {why}"));
        assert_eq!(
            (g[3], g[5]),
            (want_scroll, want_client),
            "G_HSCROLL_CAROUSEL {id}: scrollWidth/clientWidth must match Chrome's \
             {want_scroll}/{want_client}.\n\n  {why}."
        );
    }
}
