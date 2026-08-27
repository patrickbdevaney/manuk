//! # G_ALIGN_ATTRIBUTE_IS_A_FLOAT — `align` on an image is a FLOAT, and the TAG is not what
//! decides it
//!
//! t1367 landed the `text-align` half of HTML's `align` attribute and named this half in its own
//! closing note rather than pin it at our wrong value. This is that half.
//!
//! ⚠⚠⚠ **`<img align=right>` DOES NOT MEAN "ALIGN THE IMAGE RIGHT". IT MEANS `float: right`.** The
//! image leaves the inline flow entirely, and every word after it moves to the LEFT edge — the
//! opposite direction from where a reader of the attribute name would expect anything to move. We
//! implemented no part of it, so the image stayed inline and the text sat one image-width in:
//!
//! ```text
//!   <div style="width:600px"><img align=right width=40><span>t</span></div>
//!                                   Chrome        before      after
//!     the image                      560             0         560
//!     the following text               0            40           0
//!     the container's height          40            44          40
//! ```
//!
//! ⭐⭐ **THE ELEMENT SET IS NOT THE TAG SET, AND `<input>` IS THE ROW THAT PROVES IT.** Blink
//! reaches this mapping through `HTMLElement::ApplyAlignmentAttributeToStyle`, called from
//! `HTMLImageElement`, `HTMLObjectElement`, `HTMLEmbedElement`, `HTMLIFrameElement` and — this is
//! the whole point — from `HTMLInputElement` **only in the `type=image` branch**. Measured in
//! headless Chrome with `getComputedStyle().float`, one element per row:
//!
//! ```text
//!   img     align=left|right   float:left|right + vertical-align:top
//!   object  align=left|right   float:left|right + vertical-align:top
//!   embed   align=left|right   float:left|right + vertical-align:top
//!   iframe  align=left|right   float:left|right + vertical-align:top
//!   table   align=left|right   float:left|right   ONLY — no vertical-align
//!   input   align=left|right   float:NONE              ← type=text
//!   input   align=left|right   float:left|right + top  ← type=IMAGE, the SAME TAG
//!   hr  applet  marquee        float:NONE
//!   video  canvas  svg  button  select  textarea       float:NONE
//! ```
//!
//! **Our own prose said otherwise and so does everyone's.** The list this engine already carried —
//! the set excluded from the `text-align` mapping because "their `align` means FLOAT" — is
//! `img object embed iframe applet table hr input marquee`. Three of those (`hr`, `applet`,
//! `marquee`) do not float in Chrome at all, and `input` floats only sometimes. **Not floating is
//! the correct behaviour and the natural implementation gets it wrong**, which is why `#f7` is in
//! this gate: it is the same tag as `#f6`, one attribute apart, and it must not move.
//!
//! The vertical half of the same Blink call, also Chrome-measured, and identical on `img`,
//! `object`, `embed`, `iframe` and `input type=image`:
//!
//! ```text
//!   align=top        top       align=absmiddle  middle     align=left|right  top (with the float)
//!   align=texttop    text-top  align=absbottom  bottom     align=bottom      baseline (nothing)
//!   align=center     middle    align=baseline   baseline   align=<unknown>   baseline (nothing)
//!   align=middle     -webkit-baseline-middle    ← NOT MAPPED; see the NEXT note below
//! ```
//!
//! ⭐ **`center` AND `middle` ARE NOT SYNONYMS HERE**, which is the row nobody would guess and
//! `#f14` is here to hold: `center` is CSS `middle` (the box's centre against the baseline plus half
//! the x-height) while `middle` is `-webkit-baseline-middle` (the box's centre against the baseline
//! itself). Every other place in HTML that spells this attribute treats the two as the same word.
//!
//! ⚠ **`align=middle` IS DELIBERATELY ABSENT FROM THIS GATE AND FROM THE ENGINE.** We have no
//! `-webkit-baseline-middle`, and `VerticalAlign::Middle` is a *different value* — on the fixture
//! Chrome puts the following text's baseline at the image's centre (a delta of 5.0 against an image
//! 40 tall) where `Middle` puts it half an x-height lower. Answering `Middle` would be a wrong
//! answer of the right type, and asserting it here would bank the wrong answer as correct. Measured
//! and named instead: it is this tick's NEXT, and it needs a new `VerticalAlign` variant plus the
//! four layout sites that switch on one.
//!
//! Every number below is CAPTURED from `google-chrome --headless --hide-scrollbars
//! --window-size=1200,800` on this exact document, 16px/20px monospace.
//!
//! ```text
//!                                                     Chrome    before    goes red under
//!   #f1   <img align=right>                    KEY      560         0     M1 M2
//!   #f1s  …the text after it                   KEY        0        40     M1 M2
//!   #f2   <img align=left>                                0         0     M4 (560)
//!   #f2s  …the text after it                             40        40     M4
//!   #f3   <img align=RIGHT>                    KEY      560         0     M1 M2 M5 (0)
//!   #f4   <table align=right>                           500         0     M1 M3 (0)
//!   #f4s  …the text after it                              0         0     —
//!   #f5s  <table align=left> → the text after           100         0     M1 M3
//!   #f6   <input type=image align=right>       KEY      560         —     M1 M6 (0)
//!   #f7   <input align=right>                  KEY        0         0     M2 (395)
//!   #f7s  …the text after it                   KEY      205       205     M2 (0)
//!   #c1   float:right, no attribute            CTRL     560       560     —
//!   #c2   a plain <img>                        CTRL       0         0     —
//!   #c3   <table align=center>                 CTRL     250       250     M3 (0)
//!   #c4   align=right + style="float:none"     CTRL       0         0     M7 (560)
//!   #ra   the container's HEIGHT               KEY       40        44     M1
//!   delta #f10  align=top                                 0        25     M8 (25)
//!   delta #f12  align=absmiddle                         9.5        25     M8 (25)
//!   delta #f13  align=absbottom                          20        25     M8 (25)
//!   delta #f14  align=center                   KEY      9.5        25     M9 (0)
//!   delta #f15  align=top, line-height 40                10        30     M8 (30)
//!   delta #f16  align=texttop, line-height 40             0        30     M8 (30)
//!   delta #c5   align=top + vertical-align:baseline CTRL 25        25     M10 (0)
//!   delta #c2   a plain <img>                  CTRL      25        25     —
//! ```
//!
//! ⭐ **`#c4` AND `#c5` ARE THE ORIGIN PAIR, AND THEY ARE WHY NEITHER HALF IS ASSIGNED ONTO THE
//! COMPUTED STYLE.** A presentational hint must lose to an author declaration. `#c4` writes
//! `float:none` beside `align=right` and `#c5` writes `vertical-align:baseline` beside `align=top`;
//! both must read as though the attribute were not there. The `float` half is a declaration in
//! `presentational_hint_block`, at hint origin; the `vertical-align` half is in `apply_ua_defaults`,
//! which runs before author rules — and it has to be there rather than beside its twin, because
//! `vertical-align` is one of the properties `stylo_engine.rs` recovers from `MinimalCascade`
//! wholesale after Stylo has run, so a rule written in the Stylo hint block is overwritten and
//! inert. That is the third time this trap has been sprung here (t923's `sup`, t1366's `<td>`), and
//! M8 is the mutation that re-proves it: putting `vertical-align` in the hint block instead leaves
//! every delta row at its pre-tick value while every `float` row still passes.
//!
//! MUTATIONS, each applied to the engine, rebuilt, and read back:
//!
//! * **M1 — the `float` half not emitted at all** → `#f1` 0, `#f1s` 40, `#ra` 44
//! * **M2 — the float set keyed on the existing `NO_TEXT_ALIGN` tag list**, the natural
//!   simplification → every subject row still passes and `#f7` jumps to 395 with `#f7s` at 0
//! * **M3 — `table` dropped from the float set** → `#f4` 0, and `#c3` still 250 (so `align=center`
//!   is not what is being tested)
//! * **M4 — `left` mapped to `float:right`** → `#f2` 560, not 0
//! * **M5 — the attribute compared case-SENSITIVELY** → `#f3` 0
//! * **M6 — `input` matched on the tag alone, without the `type=image` test** → `#f6` 560 still,
//!   but `#f7` 395: the mutation M2 catches from the other side
//! * **M7 — the float written onto the computed style after the cascade** → `#c4` 560, not 0
//! * **M8 — the `vertical-align` half emitted in `presentational_hint_block` beside the float**,
//!   i.e. the spelling that looks right → every delta row back to its pre-tick value (25/25/25/30)
//!   while every `float` row stays green
//! * **M9 — `center` folded into the `middle` arm's neighbour** (mapped to `Top`) → `#f14` 0
//! * **M10 — the vertical half assigned after author rules instead of in `apply_ua_defaults`** →
//!   `#c5` 0, not 25
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 body{margin:0;font:16px/20px monospace}
 div.r{width:600px;overflow:hidden}
 img{width:40px;height:40px}
 table{border-collapse:separate;border-spacing:0}
 td{padding:0}
 #f6{width:40px;height:40px;padding:0;border:0}
</style></head><body>
<div class="r" id="ra"><img id="f1" align="right"><span id="f1s">t</span></div>
<div class="r" id="rb"><img id="f2" align="left"><span id="f2s">t</span></div>
<div class="r" id="rc"><img id="f3" align="RIGHT"><span id="f3s">t</span></div>
<div class="r" id="rd"><table id="f4" align="right" width="100"><tr><td>x</td></tr></table><span id="f4s">t</span></div>
<div class="r" id="re"><table id="f5" align="left" width="100"><tr><td>x</td></tr></table><span id="f5s">t</span></div>
<div class="r" id="rf"><input type="image" id="f6" align="right"><span id="f6s">t</span></div>
<div class="r" id="rg"><input id="f7" align="right"><span id="f7s">t</span></div>
<div class="r" id="ri"><img id="c1" style="float:right"><span id="c1s">t</span></div>
<div class="r" id="rj"><img id="c2"><span id="c2s">t</span></div>
<div class="r" id="rk"><table id="c3" align="center" width="100"><tr><td>x</td></tr></table></div>
<div class="r" id="rl"><img id="c4" align="right" style="float:none"><span id="c4s">t</span></div>
<div class="r" id="rm"><img id="f10" align="top"><span id="f10s">t</span></div>
<div class="r" id="ro"><img id="f12" align="absmiddle"><span id="f12s">t</span></div>
<div class="r" id="rp"><img id="f13" align="absbottom"><span id="f13s">t</span></div>
<div class="r" id="rq"><img id="f14" align="center"><span id="f14s">t</span></div>
<div class="r" id="rr"><img id="c5" align="top" style="vertical-align:baseline"><span id="c5s">t</span></div>
<div class="r" id="rs" style="line-height:40px"><img id="f15" align="top"><span id="f15s">t</span></div>
<div class="r" id="rt" style="line-height:40px"><img id="f16" align="texttop"><span id="f16s">t</span></div>
</body></html>
"##;

fn rect(page: &manuk_page::Page, sel: &str) -> (f32, f32, f32, f32) {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let r = page
        .root_box
        .node_rects(dom)
        .get(&n)
        .copied()
        .unwrap_or_else(|| panic!("no box for {sel}"));
    (r.x, r.y, r.width, r.height)
}

/// The float half is a HORIZONTAL position, so x is what those rows assert.
fn x(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let (got, _, _, _) = rect(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_ALIGN_ATTRIBUTE_IS_A_FLOAT: `{sel}` expected x={want} (CAPTURED from \
         `google-chrome --headless --hide-scrollbars --window-size=1200,800`), got x={got} — {why}"
    );
}

/// Width, so a box that reached the right x by being the wrong SIZE does not read as a pass — a
/// right-floated box is placed from its own right edge, so a width error and a placement error land
/// in the same number.
fn w(page: &manuk_page::Page, sel: &str, want: f32) {
    let (_, _, got, _) = rect(page, sel);
    assert!(
        (got - want).abs() < 1.02,
        "G_ALIGN_ATTRIBUTE_IS_A_FLOAT: `{sel}` expected w={want}, got w={got} — a box at the right \
         x for the wrong reason is not a pass"
    );
}

fn h(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let (_, _, _, got) = rect(page, sel);
    assert!(
        (got - want).abs() < 1.01,
        "G_ALIGN_ATTRIBUTE_IS_A_FLOAT: `{sel}` expected h={want}, got h={got} — {why}"
    );
}

/// The vertical half, as the OFFSET of the following text from the box's own top. A delta rather
/// than an absolute y on purpose: every row of this fixture stacks on the ones above it, so one
/// absolute number would make eighteen rows fail whenever any single one moved.
fn dy(page: &manuk_page::Page, sel: &str, want: f32, why: &str) {
    let (_, top, _, _) = rect(page, sel);
    let (_, text, _, _) = rect(page, &format!("{sel}s"));
    let got = text - top;
    assert!(
        (got - want).abs() < 1.01,
        "G_ALIGN_ATTRIBUTE_IS_A_FLOAT: `{sel}` expected the following text {want}px below the \
         box's top (CAPTURED from headless Chrome), got {got}px — {why}"
    );
}

#[test]
fn g_align_attribute_is_a_float() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://alignfloat.test/", &fonts, 1200.0);

    // ── THE SUBJECT: `align=left|right` is a FLOAT, and the text after it moves the OTHER WAY.
    w(&page, "#f1", 40.0);
    x(
        &page,
        "#f1",
        560.0,
        "<img align=right> floats to the container's right edge; it does not sit inline",
    );
    x(
        &page,
        "#f1s",
        0.0,
        "…and the text after a right float starts at the LEFT edge, not after the image",
    );
    h(
        &page,
        "#ra",
        40.0,
        "the container is as tall as the float it contains, not as tall as an inline image plus \
         its descender",
    );
    x(
        &page,
        "#f2",
        0.0,
        "<img align=left> floats to the left edge",
    );
    x(
        &page,
        "#f2s",
        40.0,
        "…and a LEFT float pushes the following text to its right — the opposite of #f1s",
    );
    x(
        &page,
        "#f3",
        560.0,
        "the attribute value is ASCII case-insensitive: align=RIGHT is align=right",
    );

    // ── The same mapping on a <table>, which takes the float and NOT the vertical alignment.
    w(&page, "#f4", 100.0);
    x(&page, "#f4", 500.0, "<table align=right> floats right");
    x(
        &page,
        "#f4s",
        0.0,
        "…and the text after it goes to the left edge",
    );
    x(&page, "#f5", 0.0, "<table align=left> floats left");
    x(
        &page,
        "#f5s",
        100.0,
        "…and the text after it clears the table's own 100px width",
    );

    // ── ⭐⭐ THE PAIR THAT PROVES THE KEY IS NOT THE TAG. One attribute apart, opposite answers.
    w(&page, "#f6", 40.0);
    x(
        &page,
        "#f6",
        560.0,
        "<input type=image align=right> DOES float — Blink reaches the alignment mapping from \
         HTMLInputElement's image branch",
    );
    x(
        &page,
        "#f7",
        0.0,
        "<input align=right> — a TEXT field — does NOT float, and the natural tag-keyed \
         implementation floats it",
    );
    x(
        &page,
        "#f7s",
        205.0,
        "…so the text after a non-floating input still sits after the control's full 205px width",
    );

    // ── CONTROLS. Each one must read exactly as though the attribute were not involved.
    x(
        &page,
        "#c1",
        560.0,
        "CONTROL: a CSS float:right on the same image — float layout itself works, so a failure \
         above is the attribute mapping and nothing else",
    );
    x(&page, "#c1s", 0.0, "CONTROL: …and its text goes left");
    x(&page, "#c2", 0.0, "CONTROL: a plain <img> stays inline");
    x(
        &page,
        "#c3",
        250.0,
        "CONTROL: <table align=center> is `margin-inline:auto`, NOT a float — the centred table \
         must not be dragged into the float mapping",
    );
    x(
        &page,
        "#c4",
        0.0,
        "CONTROL/ORIGIN: an author's `float:none` beats the presentational hint, which is only \
         true while the hint is a DECLARATION and not a value assigned after the cascade",
    );
    x(
        &page,
        "#c4s",
        40.0,
        "CONTROL: …and the image is inline again",
    );

    // ── THE VERTICAL HALF of the same Blink call.
    dy(
        &page,
        "#c2",
        25.0,
        "CONTROL: a plain inline image sits ON the baseline, so the text starts 25px down",
    );
    dy(
        &page,
        "#f10",
        0.0,
        "align=top puts the box's top on the line box's top",
    );
    dy(
        &page,
        "#f12",
        9.5,
        "align=absmiddle is CSS `middle`, not `top`",
    );
    dy(&page, "#f13", 20.0, "align=absbottom is CSS `bottom`");
    dy(
        &page,
        "#f14",
        9.5,
        "⭐ align=center is CSS `middle` — the one value whose name says `center` and whose \
         meaning is `middle`",
    );
    dy(
        &page,
        "#f15",
        10.0,
        "align=top is the LINE BOX's top: at line-height 40 the 40px image fills the line and the \
         text's own 20px line sits centred 10px down",
    );
    dy(
        &page,
        "#f16",
        0.0,
        "…while align=texttop is the parent FONT's text top, which at line-height 40 is a \
         different number — the row that tells `top` and `text-top` apart",
    );
    dy(
        &page,
        "#c5",
        25.0,
        "CONTROL/ORIGIN: an author's `vertical-align:baseline` beats align=top, which is only \
         true while the hint runs BEFORE author rules",
    );
}
