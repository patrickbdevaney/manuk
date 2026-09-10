//! **G_LOAD_FIRES_AFTER_THE_WEBFONT_ARRIVES — every `window.onload` handler measured text in a font
//! the page does not use.**
//!
//! `load_async`'s pre-`load` block waits for subframes, images and masks, and each of those waits has
//! a comment saying why: a handler that reaches into a not-yet-loaded frame **throws** (it cost the
//! entire 767k-subtest `encoding` suite), and one that measures an undecoded image gets
//! `naturalWidth === 0` (it cost `grid-minimum-size-grid-items-021` exactly half its subtests — every
//! WIDTH assertion passing and every HEIGHT one failing).
//!
//! **The stylesheet phase was not in that list**, and it is where `@font-face` is fetched and where an
//! arriving face triggers the relayout. So `load` fired on a document laid out in the FALLBACK face;
//! `__fireLoad`'s once-only guard (`load` fires exactly once, ever — correctly) then made the later,
//! correct dispatch a no-op.
//!
//! ## Arbitrated against headless Chrome
//!
//! Ahem defines every glyph as exactly one em, so five characters at 32px is **exactly 160px** — a
//! fallback cannot coincidentally produce that number.
//!
//! ```text
//!                    DOMContentLoaded    load     a later task
//!   Chrome                  96            160         160
//!   before                  96             96          96
//!   after                   96            160         160
//! ```
//!
//! ⚠⚠ **THE LAYOUT WAS RIGHT THE WHOLE TIME.** `root_box` measured 160 before this fix; only the
//! geometry JS could see was stale, and a host re-entry one call later read 160 correctly. **No
//! rendering test could catch this** — the same shape as t1479's `document.styleSheets`, where the
//! effect was right and the description of it was wrong.
//!
//! ⚠ **`dcl:96` IS PART OF THE ASSERTION, NOT NOISE.** Chrome fires `DOMContentLoaded` before the font
//! has arrived and this engine must too. A fix that simply loaded fonts earlier would make DCL read
//! 160 and be *differently* wrong; the ordering is the claim.
//!
//! ## Who this is for
//!
//! Every component that measures text after load: a carousel sizing its slides, a virtualised list
//! computing row heights, a chart laying out axis labels, a "fit text to box" widget, and every
//! framework that measures on `load`. All of them were being handed the fallback face's metrics.
//!
//! Mutations that must turn this red:
//!   1. remove the pre-`load` stylesheet pass   -> `load:96`
//!   2. move it after the `load` dispatch       -> `load:96`
//!   3. make `__fireLoad` re-entrant            -> `dcl` and `load` both fire twice; the log doubles
//!   4. drop the `has_stylesheet_source` guard   -> the predicate arm below reads `true` for bare
//!                                                  markup (and, on the corpus, `wpt html/semantics`
//!                                                  goes HANG/CRASH 0 -> 1)
//!   5. unbound the phase's deadline               -> `g_load_budget` goes 4.2s -> 13.7s
//!
//! ⚠⚠ **THE GUARD IS PART OF THE FIX, AND THE UNGUARDED VERSION WAS A BAR 0.** Waiting for the
//! stylesheet phase unconditionally made `wpt html/semantics` go `HANG/CRASH 0 -> 1` on
//! `tabular-data/processing-model-1/span-limits.html` — bare markup with `colspan=1000` cells, no
//! `<style>`, no `<link>`, no `@font-face`. The pass could not change one pixel of it and charged it
//! a second full relayout of the expensive kind. The same-hour old-binary control confirmed the
//! attribution; the guard is the precondition, not a heuristic.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
@font-face { font-family: "AhemGate"; src: url("ahem.ttf") format("truetype"); }
#a { font-family: "AhemGate", monospace; font-size: 32px; }
#m { font-family: monospace; font-size: 32px; }
</style></head><body>
<span id="a">MMMMM</span><br><span id="m">MMMMM</span>
<div id="out">-</div>
<script>
function w(i) { return Math.round(document.getElementById(i).getBoundingClientRect().width); }
var log = [];
document.addEventListener('DOMContentLoaded', function () { log.push('dcl:' + w('a')); });
window.addEventListener('load', function () {
  log.push('load:' + w('a'));
  setTimeout(function () {
    log.push('t0:' + w('a'));
    document.getElementById('out').textContent = log.join(' ') + ' m=' + w('m');
  }, 0);
});
</script></body></html>"##;

#[test]
fn window_onload_measures_the_webfont_and_not_the_fallback() {
    let dir = std::path::Path::new("/tmp/g1490");
    std::fs::create_dir_all(dir).unwrap();
    assert!(
        dir.join("ahem.ttf").exists(),
        "fixture precondition: Ahem must be at /tmp/g1490/ahem.ttf (servo/tests/wpt/tests/fonts/)"
    );
    std::fs::write(dir.join("page.html"), HTML).unwrap();

    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(
            HTML,
            &format!("file://{}/page.html", dir.display()),
            &fonts,
            800.0,
        )
        .await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("LOAD-TIME GEOMETRY: {got}");

    // ── VACUITY, and it is what makes the 160 mean anything. `m=96` is the monospace FALLBACK, so a
    //    fixture where the webfont silently failed would read `a` and `m` the same. Ahem's 160 is
    //    arithmetic (one em per glyph, five glyphs, 32px) and a fallback cannot produce it by chance.
    assert!(
        got.contains("m=96"),
        "VACUOUS: the monospace control did not measure 96, so this fixture is not measuring what it \
         claims — {got:?}"
    );

    // Chrome headless, exactly. `dcl:96` is part of the claim: Chrome fires DOMContentLoaded BEFORE
    // the font arrives, and a fix that merely loaded fonts earlier would be differently wrong.
    let want = "dcl:96 load:160 t0:160 m=96";
    assert_eq!(
        got, want,
        "\n  `load` must fire after the document's @font-face faces have arrived and been laid out \
         with — the geometry every window.onload handler measures\n  want: {want}\n  got:  {got}"
    );

    // ── THE GUARD'S OWN ARM. The wait is paid only where it can change the answer; an unconditional
    //    one was a Bar 0 (see the header). This fixture has an inline `<style>`, so it must pay it —
    //    and a document with no stylesheet source at all must not.
    assert!(
        page.has_stylesheet_source(),
        "this fixture declares its @font-face in an inline <style>, so the pre-`load` pass MUST be \
         reached for it — a guard that skipped here would make the assertion above pass for the \
         wrong reason"
    );
    let bare = rt.block_on(async {
        manuk_page::Page::load_async(
            "<!doctype html><html><body><table><tr><td colspan=1000>a</table></body></html>",
            "https://bare.test/",
            &fonts,
            800.0,
        )
        .await
    });
    assert!(
        !bare.has_stylesheet_source(),
        "bare markup has no stylesheet source, so it has no @font-face to wait for — charging it a \
         second full relayout is what turned `wpt html/semantics` red on span-limits.html"
    );
}
