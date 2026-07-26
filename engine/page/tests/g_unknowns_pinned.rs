//! **G_UNKNOWNS_PINNED — four capabilities the map asserted without evidence, measured and pinned.**
//!
//! Surface audit #34 (t618) moved 14 rows to `unknown` because nothing in the repo measured them —
//! they had been claiming `works` on a bare assertion. This gate is the follow-through for the ones
//! that turned out to WORK, so they can go back to `gated` with something behind them. The ones that
//! did NOT work are recorded in the map as `missing`/`partial` with what was measured, not gated here:
//! **a gate for a capability that does not work is a gate that cannot pass.**
//!
//! Measured in one probe:
//!
//! ```text
//!   constructable stylesheets   new CSSStyleSheet() ok · replaceSync fn · document.adoptedStyleSheets = [] ok
//!   forced reflow               40 -> 140   (a mid-script style write is visible to the NEXT read)
//!   list markers                li.left 48 vs ul.left 8 — the marker's 40px of indent exists
//!   overflow-anchor             MISSING from getComputedStyle — the property is not computed at all
//!   ResizeObserver              constructs, observes, and NEVER DELIVERS (fired:0 after mutate+drain)
//! ```
//!
//! **`overflow-anchor` and `ResizeObserver` are the reason this audit was worth running.** Both had
//! `works` in the map. One is not implemented at all; the other is the `G_MUTATION` shape — an object
//! that constructs, accepts `observe()`, and calls your callback never. *"The global exists" is not
//! "the observer fires"*, and only a probe that waits for delivery can tell them apart.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head><style>
  ul { list-style: disc; }
  #box { width: 100px; height: 40px; }
</style></head><body>
  <ul id="ul"><li>one</li><li>two</li></ul>
  <div id="box">x</div>
  <div id="out">-</div>
  <script>
    var R = [];

    // ── CONSTRUCTABLE STYLESHEETS. A design-system runtime builds sheets in JS and adopts them;
    // without the constructor it must fall back to injecting <style> tags, and without the setter
    // that fallback is all there is.
    R.push('sheetCtor:' + (typeof CSSStyleSheet));
    var sh = new CSSStyleSheet();
    R.push('replaceSync:' + (typeof sh.replaceSync));
    (function () {
      'use strict';                       // the setter must not merely no-op — assignment must work
      try { document.adoptedStyleSheets = []; R.push('adoptSet:ok'); }
      catch (e) { R.push('adoptSet:' + e.name); }
    })();
    R.push('adoptGet:' + (Array.isArray(document.adoptedStyleSheets) ? 'array' : typeof document.adoptedStyleSheets));

    // ── FORCED REFLOW. Every measurement-driven library writes a style and reads geometry back in the
    // same tick. If the read returns the PRE-mutation box, the library computes from a stale layout —
    // silently, and only on the first frame.
    var b = document.getElementById('box');
    var h1 = Math.round(b.getBoundingClientRect().height);
    b.style.height = '140px';
    var h2 = Math.round(b.getBoundingClientRect().height);
    R.push('reflow:' + h1 + '->' + h2);

    // ── LIST MARKERS occupy space. A marker that generates no box makes every `<li>` start at its
    // parent's edge, which is what an unstyled-looking list actually is.
    var li = document.getElementById('ul').getElementsByTagName('li')[0];
    var indent = Math.round(li.getBoundingClientRect().left)
               - Math.round(document.getElementById('ul').getBoundingClientRect().left);
    R.push('markerIndent:' + indent);

    // ── RESIZEOBSERVER, pinned as the HONEST NEGATIVE it measured as. It constructs and accepts
    // `observe()`, and the callback is never called — the `G_MUTATION` shape. This assertion is a
    // NEGATIVE and is therefore the one that rots: **the moment delivery lands it goes RED, and that
    // is the signal to re-price the map row, not to retune this line.** An honest "no" that nobody
    // updates becomes a lie exactly when the capability arrives.
    R.push('roCtor:' + (typeof ResizeObserver));
    var roFired = 0;
    var ro = new ResizeObserver(function () { roFired++; });
    ro.observe(document.getElementById('box'));
    b.style.height = '260px';               // a real size change on the observed element
    R.push('roFired:' + roFired);

    document.getElementById('out').textContent = R.join(' ');
  </script>
</body></html>"#;

#[test]
fn capabilities_the_map_asserted_are_measured() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://u.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        // constructable stylesheets — the whole adopt path, not just the constructor's existence
        "sheetCtor:function",
        "replaceSync:function",
        "adoptSet:ok",
        "adoptGet:array",
        // a mid-script write is visible to the next read
        "reflow:40->140",
        // the marker's indent exists (Chrome's default is 40px; the claim is that it is NOT 0)
        "markerIndent:40",
        // …and the honest negative: the surface exists, delivery does not.
        "roCtor:function",
        "roFired:0",
    ] {
        assert!(
            got.contains(claim),
            "G_UNKNOWNS_PINNED: expected `{claim}`\n  got: {got}\n\n  \
             These are capabilities the map claimed `works` with NO gate until surface audit #34 \
             demoted them to `unknown`. This gate is what lets them be claimed again. \
             `reflow:40->40` means a style write is not visible to the next geometry read, which \
             silently feeds every measurement-driven library a stale layout. `markerIndent:0` means \
             list markers generate no box, which is what an unstyled-looking list actually is. \
             `adoptSet:TypeError` means the setter is getter-only — a throw, not a no-op."
        );
    }
}
