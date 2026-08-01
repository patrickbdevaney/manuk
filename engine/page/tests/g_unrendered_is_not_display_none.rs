//! # G_UNRENDERED_IS_NOT_DISPLAY_NONE — `<source>` is `inline`, and still draws nothing
//!
//! Eight elements were hidden with `display: none` in the UA sheet. That produced the right BOX and
//! the wrong ANSWER for half of them. Measured out of headless Chrome with
//! `getComputedStyle(el).display`, not recalled:
//!
//! ```text
//!   <source>    inline   ← we said none        <param>     none  ✓
//!   <track>     inline   ← we said none        <datalist>  none  ✓
//!   <area>      inline   ← we said none        <template>  none  ✓
//!   <noscript>  inline   ← we said none        <rp>        none  ✓
//! ```
//!
//! Those four generate no box because their **parent consumes them** — `<picture>`/`<video>` render
//! their `<img>`/media, `<map>` is not a container, `<noscript>` with scripting enabled holds raw
//! text — and *not* because a stylesheet hides them. The difference is invisible until a page asks,
//! and `getComputedStyle(source).display` is exactly what a responsive-image shim reads.
//! `<picture><source>` is how the entire modern web serves responsive images.
//!
//! ⚠⚠ **AND THE STRUCTURAL GUARD TURNED OUT NOT TO BE NEEDED, WHICH IS WHY THERE ISN'T ONE.** The
//! first version of this change added a `never_rendered(tag)` check to `is_rendered` to keep the four
//! from drawing once their `display` stopped being `none`. Disabling that check entirely changes
//! nothing here and nothing on the corpus — `mobcup.fm` reads 0.909091 either way — because these
//! elements' parents never lay them out as content in the first place. It also turned out to
//! *improve* `en.wikipedia.org`'s coverage (0.998141 → 1.000000). **A guard that cannot be shown to
//! do anything is not a safety margin, it is unexplained machinery**, so the shipped change is the UA
//! sheet edit alone. The `#w3` / `#w1` rows below stay: they are what MEASURED that, and they are
//! what will notice if it ever stops being true.
//!
//! ⚠ **BOTH cascades were changed in the same tick.** The `display: none` list exists twice — the
//! Stylo UA sheet and `apply_ua_defaults`'s `MinimalCascade` — and the second one's own comment says
//! *"Keep in lockstep … The two cascades disagreeing about which elements render at all is how a
//! `<source>` ends up with 19px of height in one configuration and none in the other."* This gate
//! runs on the shipping (Stylo) path; the lockstep is asserted by the layout rows below, which would
//! fail under either cascade if only one had moved.
//!
//! ## How this goes RED
//!
//! - **Put `source, track, area, noscript` back in the UA `display: none` list** → the four `inline`
//!   assertions fail, while the four `none` ones still pass. That split is the point: half the list
//!   was right and the fix must not flip the other half.
//! - ⚠ **There is no second mutation, and that is stated rather than implied.** The obvious one —
//!   "make them render" — has no switch to flip, because nothing renders them: `#w3` and `#w1` pass
//!   with or without the structural guard that was tried and removed. So this gate proves the
//!   COMPUTED VALUE half by mutation and pins the NO-BOX half by assertion only. A gate that cannot
//!   produce a red for one of its claims must say so.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>body{margin:0;font:16px/1.25 sans-serif}</style></head><body>
<div id="w1" style="width:600px"><picture id="pic"><source id="src" srcset="x.webp" type="image/webp"><img id="im" width="40" height="20" alt=""></picture></div>
<div id="w2" style="width:600px"><map id="mp"><area id="ar" shape="rect" coords="0,0,1,1"></map>text</div>
<div id="w3" style="width:600px"><noscript id="ns">HIDDEN TEXT THAT MUST NOT RENDER AT ALL EVER</noscript>visible</div>
<div id="w4" style="width:600px"><video id="vid"><track id="trk" kind="captions"></video></div>
<div id="w5" style="width:600px"><object id="obj"><param id="prm" name="a" value="b"></object></div>
<div id="w6" style="width:600px"><datalist id="dl"><option>x</option></datalist><template id="tpl"><b>x</b></template><ruby>x<rp id="rp">(</rp><rt>y</rt></ruby></div>
</body></html>"##;

fn display_of(page: &manuk_page::Page, sel: &str) -> manuk_css::Display {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    page.styles_map()
        .get(&n)
        .unwrap_or_else(|| panic!("{sel} has no computed style"))
        .display
}

fn assert_display(page: &manuk_page::Page, sel: &str, want: manuk_css::Display, why: &str) {
    let got = display_of(page, sel);
    assert!(
        got == want,
        "G_UNRENDERED_IS_NOT_DISPLAY_NONE: `{sel}` computed display expected {want:?} (MEASURED in \
         headless Chrome via getComputedStyle on THIS markup), got {got:?}.\n  {why}"
    );
}

fn height_of(page: &manuk_page::Page, sel: &str) -> f32 {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    page.root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel}"))
        .height
}

#[test]
fn g_unrendered_is_not_display_none() {
    use manuk_css::Display;
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://unrend.test/", &fonts, 1200.0);

    // ── THE FOUR THAT WERE WRONG. Chrome computes `inline` for every one.
    assert_display(
        &page,
        "#src",
        Display::Inline,
        "`<source>` in a `<picture>` — the responsive-image idiom of the entire modern web, and the \
         one a shim is most likely to interrogate",
    );
    assert_display(&page, "#trk", Display::Inline, "`<track>` in a `<video>`");
    assert_display(&page, "#ar", Display::Inline, "`<area>` in a `<map>`");
    assert_display(
        &page,
        "#ns",
        Display::Inline,
        "`<noscript>` — its CONTENT is not rendered, but the element's computed display is `inline`",
    );

    // ── THE FOUR THAT WERE RIGHT. Half the list was correct and the fix must not flip it.
    assert_display(
        &page,
        "#prm",
        Display::None,
        "`<param>` really IS display:none in Chrome — measured, not assumed by analogy with <source>",
    );
    assert_display(&page, "#dl", Display::None, "`<datalist>` likewise");
    assert_display(&page, "#tpl", Display::None, "`<template>` likewise");
    assert_display(&page, "#rp", Display::None, "`<rp>` likewise");

    // ── AND THEY STILL DRAW NOTHING. This is the assertion that keeps the change from being a
    //    trade: the whole reason the `display:none` was there is that these must not render.
    let w3 = height_of(&page, "#w3");
    assert!(
        (w3 - 20.0).abs() < 2.01,
        "G_UNRENDERED_IS_NOT_DISPLAY_NONE: `#w3` holds a <noscript> with a long string plus the word \
         `visible`, and must be ONE line tall (Chrome: 20). Got {w3}. If <noscript>'s raw text starts \
         rendering, this is the row that says so — and it is the exact failure the old \
         `display: none` was protecting against"
    );
    let w1 = height_of(&page, "#w1");
    assert!(
        w1 < 30.0,
        "G_UNRENDERED_IS_NOT_DISPLAY_NONE: `#w1` is a <picture> around a 20px <img>; a <source> that \
         started generating a real box would grow it. Got {w1}"
    );
}
