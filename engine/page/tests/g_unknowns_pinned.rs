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
//!   ResizeObserver              DELIVERS — see the correction below
//! ```
//!
//! **`overflow-anchor` had `works` in the map and is not implemented at all** — the property is not
//! even computed. That is the finding this audit was worth running for.
//!
//! ⚠⚠ **AND ONE OF t621's FINDINGS WAS WRONG, CORRECTED HERE AT t622.** ResizeObserver was pinned as
//! an "inert stub, callback NEVER called". It is not. `__runObservers` — the engine's only honest
//! moment to ask *"did this box change size?"* — is called from `view_changed`, and the t621 probe
//! drove the page with script evaluation alone. Driven the way a real frame drives it, the callback
//! fires with the right box. **I measured a capability through a path that cannot deliver it and
//! published "inert stub" to the map, the journal and a commit message — one tick after writing
//! *"suspect the instrument before the subject"* twice.** The assertion here is now the POSITIVE, and
//! the harness calls `view_changed` so the gate exercises the real delivery path.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head><style>
  ul { list-style: disc; }
  #box { width: 100px; height: 40px; }
</style></head><body>
  <ul id="ul"><li>one</li><li>two</li></ul>
  <div id="box">x</div>
  <div id="robox" style="width:80px;height:55px">r</div>
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

    // ── RESIZEOBSERVER. t621 pinned this as an "inert stub, callback NEVER called" and **that was
    // WRONG** — the probe drove it with script evaluation alone, and `__runObservers` (the engine's
    // only honest moment to ask "did this box change size?") is called from `view_changed`. Driven
    // the way the engine actually drives it, the callback fires. The assertion is now the POSITIVE,
    // and the harness below calls `view_changed` because that is what a real frame does.
    R.push('roCtor:' + (typeof ResizeObserver));
    globalThis.__roFired = 0;
    globalThis.__roSizes = [];
    new ResizeObserver(function (list) {
      globalThis.__roFired++;
      list.forEach(function (e) { globalThis.__roSizes.push(Math.round(e.contentRect.height)); });
      // A DEDICATED element: the forced-reflow assertion above mutates #box, and an observer on the
      // same node would report that mutation instead of the size it was asked about — two assertions
      // silently measuring each other.
    }).observe(document.getElementById('robox'));

    globalThis.__report = function () {
      R.push('roFired:' + (globalThis.__roFired > 0));
      R.push('roSize:' + globalThis.__roSizes.join(','));
      document.getElementById('out').textContent = R.join(' ');
    };
    document.getElementById('out').textContent = R.join(' ');
  </script>
</body></html>"#;

#[test]
fn capabilities_the_map_asserted_are_measured() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://u.test/", &fonts, 800.0);
    // **The engine drives the observers.** `__runObservers` runs from `view_changed`, which is what a
    // real frame does — not from script evaluation. Omitting this is precisely how t621 measured
    // ResizeObserver as inert and published it.
    page.view_changed(0.0, 600.0, 800.0, false);
    page.eval_for_test("__report()");
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
        // …and ResizeObserver DELIVERS, once driven the way the engine drives it.
        "roCtor:function",
        "roFired:true",
        "roSize:55",
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
