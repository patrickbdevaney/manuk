//! **G_SCROLL_INTO_VIEW_ALIGNMENT — `scrollIntoView` ignored its argument, so every call scrolled the
//! element's top-left to the viewport origin.**
//!
//! That is `{block: "start", inline: "start"}` — and it is not the default. CSSOM-View defaults
//! `inline` to **`"nearest"`**, so even `el.scrollIntoView()` with no argument was wrong on the
//! horizontal axis. `css/cssom-view/scrollintoview.html` scored **0 of 40**.
//!
//! ```text
//!   arg                     block     inline
//!   omitted / undefined     start     nearest
//!   true                    start     nearest
//!   false                   end       nearest
//!   {block:…, inline:…}     per-key overrides, defaulting as above
//! ```
//!
//! ⭐ **`"nearest"` is the alignment that needs the CURRENT scroll position**, and it is why the
//! no-argument form could not be right by accident: it scrolls the MINIMUM — nothing at all if the
//! box already fits on that axis, otherwise just enough to bring the nearer edge in. It is also the
//! alignment an agent wants by default, because it does not throw away the reader's context.
//!
//! Fixture: WPT's own — a `200x200` box in a `padding: 4000px` body, viewport 800x720.
//!
//! ```text
//!                             want x/y      before
//!   scrollIntoView()  @ 0,0    3400/4000   4000/4000   ← inline `nearest`, not `start`
//!   …                @ 12000,0 4000/4000   4000/4000   CONTROL — nearest picks the OTHER edge
//!   scrollIntoView(false)      3400/3480   4000/4000   ← block `end`
//!   {block:center,inline:center} 3700/3740 4000/4000
//!   {block:start,inline:start} 4000/4000   4000/4000   CONTROL — the old behaviour, still right
//!   {block:end,inline:end}     3400/3480   4000/4000
//! ```

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body { margin: 0; padding: 4000px; overflow: hidden }
#testDiv { width: 200px; height: 200px; }
</style></head><body>
<div id=testDiv></div>
<div id="out">-</div>
<script>
var t = document.getElementById('testDiv');
var rows = [];
function go(label, arg, sx, sy) {
  window.scrollTo(sx, sy);
  if (arg === "OMIT") t.scrollIntoView(); else t.scrollIntoView(arg);
  rows.push(label + "=" + Math.round(window.scrollX) + "/" + Math.round(window.scrollY));
}
go("omit_tl", "OMIT", 0, 0);
go("omit_right", "OMIT", 12000, 0);
go("false_tl", false, 0, 0);
go("center", {block:"center",inline:"center"}, 0, 0);
go("startstart", {block:"start",inline:"start"}, 0, 0);
go("endend", {block:"end",inline:"end"}, 0, 0);
document.getElementById("out").textContent =
  "W=" + window.innerWidth + " H=" + window.innerHeight + " " + rows.join(" ");
</script></body></html>"##;

#[test]
fn scroll_into_view_honours_block_and_inline_alignment() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://siv.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SCROLL INTO VIEW: {got}");

    // ── VACUITY. Every expected number below is derived from this viewport; if it is not the one the
    //    numbers were computed for, the rows are asserting arithmetic about a different page.
    assert!(
        got.contains("W=800 H=720"),
        "VACUOUS: the viewport is not the 800x720 these numbers were computed for — got {got:?}"
    );

    for (claim, why) in [
        ("omit_tl=3400/4000", "⭐ THE DEFECT. With no argument, `block` is `start` (y = 4000, which was already right) and `inline` is **`nearest`** — from a left-scrolled page the nearer edge is the RIGHT one, so x = 4000 - 800 + 200 = 3400. Ours scrolled to 4000/4000, which is `inline: start`, an alignment nobody asked for."),
        ("omit_right=4000/4000", "⭐ CONTROL, and it is the one that proves `nearest` reads the CURRENT scroll. Same call, same element, page scrolled to x=12000 instead of 0: now the nearer edge is the LEFT one and x = 4000. A fix that hard-codes either edge fails one of these two rows."),
        ("false_tl=3400/3480", "`scrollIntoView(false)` is `{block: \"end\"}`: y = 4000 - 720 + 200 = 3480. The boolean overload is the oldest spelling of this API and still the commonest in the wild."),
        ("center=3700/3740", "`center` on both axes — 4000 - 800/2 + 200/2 and 4000 - 720/2 + 200/2."),
        ("startstart=4000/4000", "CONTROL — the alignment the old code always used is still available and still correct when it is actually asked for."),
        ("endend=3400/3480", "and `end` on both axes, which must equal the `false` row: the boolean overload is not a separate code path."),
    ] {
        assert!(
            got.contains(claim),
            "G_SCROLL_INTO_VIEW_ALIGNMENT: expected `{claim}` — {why}\n  got: {got}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  ignore the argument and scroll to the element's top-left (the pre-tick state)
//       -> every row except `startstart` reads 4000/4000.
// N2  default `inline` to `start` instead of `nearest`
//       -> omit_tl and omit_right both read 4000 on x; the explicit rows stay green.
// N3  implement `nearest` as "always the start edge"
//       -> omit_tl reads 4000/4000 while omit_right stays green — the pair is what catches it.
