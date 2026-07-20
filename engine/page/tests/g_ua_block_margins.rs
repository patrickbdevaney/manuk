//! # G_UA_BLOCK_MARGINS — the UA sheet's vertical metrics, against Chrome's actual numbers
//!
//! The first broad FID-SWEEP (observer, tick 267) measured the shape of the Phase-0 gap: **coverage
//! 85.9%, placement 4.5%** against a ≥75% exit bar. We render nearly every element Chrome does and
//! put almost none of them within 8px. Its most tractable population is the NEAR-MISS group —
//! old.reddit `mdy=12`, airbnb `mdy=20`, wikipedia `mdy=45`, usa.gov `mdy=82` — and the signature is
//! unmistakable: **`mdx=0`, `mdy` small, positive, and growing with the page's content density.**
//! Horizontal placement is exact. Only vertical drifts, and it accumulates down the document.
//!
//! That is the signature of missing **vertical UA margins**, not of layout math. Wikipedia's captured
//! first divergence is `after #p-tb, element #n-randompage is off by dy=-61` — both are sidebar
//! **list** items; usa.gov's is a mobile **menu** block. Negative `dy` means our element sits *higher*
//! than Chrome's: our page is too short because the boxes above it never got their margins.
//!
//! ## Two cascades, and they disagreed
//!
//! `apply_ua_defaults` (css/src/lib.rs, the `MinimalCascade` path) sets `ul`/`ol` to `1em 0` and
//! `body` to `8px`. The **Stylo `UA_CSS` sheet — the live path for every real page** — sets neither:
//! it gives `ul, ol` a `padding-left` and no margin at all. The file's own comments warn twice to
//! "keep in lockstep with `apply_ua_defaults`", because the two cascades disagreeing is how a
//! `<source>` got 19px of height in one configuration and none in the other. They had drifted again,
//! and this time on the property that decides where everything below a list lands.
//!
//! ## The numbers are MEASURED, not invented
//!
//! Every expectation below was read out of real Chrome rather than recalled from the spec:
//!
//! ```text
//! google-chrome --headless --dump-dom  (createElement + getComputedStyle per tag)
//!   body   mt=8px  mb=8px  ml=8px  mr=8px      ul/ol/menu  mt=16px mb=16px pl=40px
//!   dl     mt=16px mb=16px                     dd          ml=40px
//!   pre    mt=13px mb=13px  (1em of 13px)      hr          mt=8px  mb=8px
//!   figure mt=16px mb=16px ml=40px mr=40px     blockquote  mt=16px mb=16px ml=40px mr=40px
//!   NESTED ul (a `ul` inside a `li`)  mt=0px mb=0px
//! ```
//!
//! The **nested-list zero** is the one a from-memory implementation always misses, and it is the one
//! Wikipedia's sidebar is built from: `ul ul` gets *no* vertical margin. Adding `1em` to every list
//! unconditionally would fix the top-level case and newly break every nested menu on the web —
//! trading a 61px error for a different one while the headline number improved.
//!
//! `blockquote` is the horizontal half of the same bug: ours said `margin: 1em 0`, which does not
//! merely omit the 40px indent, it explicitly **zeroes** it.
//!
//! ## How each assertion here can go RED
//!
//! - **Delete any one added rule from `UA_CSS`.** That element's assertion fails with the Chrome
//!   number it was measured against.
//! - **Drop the `ul ul, ul ol, ol ul, ol ol { margin-block: 0 }` rule.** Only the nested assertion
//!   fails — the top-level ones stay green, which is exactly why that rule needs its own assertion.
//! - **Set the margins on the wrong axis** (`margin: 0 1em`): the vertical assertions fail while the
//!   element still "has a margin".

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<p id="p">p</p>
<blockquote id="bq">bq</blockquote>
<ul id="ul"><li id="li">a</li></ul>
<ol id="ol"><li>a</li></ol>
<dl id="dl"><dt id="dt">t</dt><dd id="dd">d</dd></dl>
<pre id="pre">pre</pre>
<hr id="hr">
<figure id="fig">f</figure>
<ul id="outer"><li><ul id="inner"><li>x</li></ul></li></ul>
</body></html>"##;

/// `[margin-top, margin-bottom, margin-left, margin-right]`, in px, as the live cascade computed it.
fn margins(page: &manuk_page::Page, sel: &str) -> [f32; 4] {
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let s = page
        .styles_of(n)
        .unwrap_or_else(|| panic!("no style for {sel}"));
    // The UA margins here are all absolute lengths or em of a known font-size, so a 0-width
    // percentage basis cannot change any of them.
    [
        s.margin.top.resolve(0.0, 0.0),
        s.margin.bottom.resolve(0.0, 0.0),
        s.margin.left.resolve(0.0, 0.0),
        s.margin.right.resolve(0.0, 0.0),
    ]
}

fn assert_v(page: &manuk_page::Page, sel: &str, top: f32, bottom: f32, why: &str) {
    let m = margins(page, sel);
    assert!(
        (m[0] - top).abs() < 0.51 && (m[1] - bottom).abs() < 0.51,
        "UA vertical margin for `{sel}`: expected top={top} bottom={bottom} (MEASURED in Chrome), \
         got top={} bottom={}.\n  {why}",
        m[0],
        m[1]
    );
}

#[test]
fn g_ua_block_margins() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ua.test/", &fonts, 800.0);

    // ── The control: `p` already had its margin, and must keep it. If this one ever fails the
    //    change broke the rules that already worked rather than adding the missing ones.
    assert_v(
        &page,
        "#p",
        16.0,
        16.0,
        "`p { margin: 1em 0 }` already worked — this is the control",
    );

    // ── Lists. The single biggest contributor: navigation menus, sidebars, footers, tables of
    //    contents and every article's bullet list are `ul`/`ol`, and each one that lost 32px of
    //    margin pulled everything below it up by 32px.
    assert_v(
        &page,
        "#ul",
        16.0,
        16.0,
        "the Stylo UA sheet gave `ul` a padding-left and NO margin, while `apply_ua_defaults` gave \
         it 1em — the two cascades had drifted apart on the property that places every list",
    );
    assert_v(
        &page,
        "#ol",
        16.0,
        16.0,
        "`ol` carries the same margin as `ul`",
    );

    // ── The nested-list ZERO. A `ul` inside a `li` has NO vertical margin in Chrome. Without this,
    //    "add 1em to every list" fixes the top level and breaks every nested menu — Wikipedia's
    //    sidebar, which is where the measured dy=-61 divergence was found, is exactly this shape.
    assert_v(
        &page,
        "#inner",
        0.0,
        0.0,
        "a NESTED list must have ZERO vertical margin. Giving every `ul` 1em unconditionally would \
         make the top-level assertions pass and newly over-space every nested menu on the web",
    );
    // …and the outer list of that same pair still gets its margin, so the rule is scoped to
    // nesting rather than switching list margins off wholesale.
    assert_v(
        &page,
        "#outer",
        16.0,
        16.0,
        "the OUTER list of a nested pair keeps its margin — if this is 0 the nesting rule is \
         matching too broadly and has simply disabled list margins",
    );

    // ── The rest of the block elements the sheet never had.
    assert_v(
        &page,
        "#dl",
        16.0,
        16.0,
        "`dl` — definition lists carry the same 1em as `ul`",
    );
    assert_v(
        &page,
        "#pre",
        13.0,
        13.0,
        "`pre` is 1em of its OWN 13px monospace font, not of 16px",
    );
    assert_v(
        &page,
        "#hr",
        8.0,
        8.0,
        "`hr { margin: 0.5em 0 }` — a rule missing entirely",
    );
    assert_v(
        &page,
        "#fig",
        16.0,
        16.0,
        "`figure` — every article image/code figure is one",
    );

    // ── The HORIZONTAL half of the same bug. `blockquote` did not merely lack the 40px indent:
    //    `margin: 1em 0` explicitly ZEROED it, so quotes sat flush with body text.
    let bq = margins(&page, "#bq");
    assert!(
        (bq[2] - 40.0).abs() < 0.51 && (bq[3] - 40.0).abs() < 0.51,
        "`blockquote` is indented 40px on both sides in Chrome — got left={} right={}. \
         Ours said `margin: 1em 0`, which does not omit the indent, it zeroes it",
        bq[2],
        bq[3]
    );
    let dd = margins(&page, "#dd");
    assert!(
        (dd[2] - 40.0).abs() < 0.51,
        "`dd` is indented 40px from its `dt` — got left={}. Without it a definition list renders \
         as a flat run of alternating lines with no visible structure",
        dd[2]
    );
    // `dt` is NOT indented — the pair is what makes a definition list readable.
    let dt = margins(&page, "#dt");
    assert!(
        dt[2].abs() < 0.51,
        "`dt` must NOT be indented (got left={}) — indenting both halves of the pair restores the \
         flat look the `dd` rule was added to fix",
        dt[2]
    );

    // ── `body`'s 8px. Most modern sites reset it away, which is why it does not show up in `mdx`
    //    on the sweep's app-class pages — but every unstyled document, every rendered email and
    //    every plain HTML page on the web depends on it.
    assert_v(
        &page,
        "body",
        8.0,
        8.0,
        "`body { margin: 8px }` — present in `apply_ua_defaults`, absent from the Stylo UA sheet",
    );
    let body = margins(&page, "body");
    assert!(
        (body[2] - 8.0).abs() < 0.51 && (body[3] - 8.0).abs() < 0.51,
        "…and 8px horizontally too — got left={} right={}",
        body[2],
        body[3]
    );
}
