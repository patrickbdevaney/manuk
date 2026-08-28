//! # G_GENERATED_BOX_TAKES_ITS_ADVANCE — `content: ""` with a width is a BOX, not nothing
//!
//! ⚠⚠⚠ **A GENERATED BOX WITH NO TEXT WAS WORTH ZERO PIXELS.** `::before`/`::after` enter the
//! inline flow as `InlineItem::Word`, so their width is the width of their TEXT — and
//! `pseudo_content` refuses an empty string outright, before any box could be made. But
//! `content: ""` with a `width` is the icon idiom:
//!
//! ```css
//!   a::before { content:""; display:inline-block; width:30px; margin:12px 20px 12px 8px }
//! ```
//!
//! The declared box occupied nothing at all, so the owner's own text started where the icon should
//! have been.
//!
//! Every number below is CAPTURED from `google-chrome --headless --hide-scrollbars
//! --window-size=1200,800`, `font: 16px monospace`, a 600px block whose `::before` is 30×30 with
//! `margin: 0 20px 0 8px`. The column is the x of the owner's `<span>`, relative to the owner.
//!
//! ```text
//!                                                     Chrome   before   after
//!   #e    content:""  display:inline-block             58        0       58
//!   #n    content:none                       CTRL       0        0        0
//!   #blk  content:""  display:block          CTRL       0        0        0
//!   #noD  content:""  no display (inline)    CTRL       0        0        0
//!   #aft  ::after content:""  inline-block   CTRL       0        0        0
//!   #g    NO content declaration, sized       CTRL       0        0        0
//! ```
//!
//! ⭐⭐ **`#blk` AND `#noD` ARE THE TWO ENDS OF THE SCOPE, AND `#blk` CAUGHT THE FIRST SPELLING OF
//! THIS FIX.** Keying on *"the generated box is not `display:inline`"* passes `#e`, `#n` and `#noD`
//! and **breaks `#blk`**: a BLOCK-level generated box does not take an inline advance, it takes its
//! own LINE (Chrome puts the owner's text at relx 0 on the next line; the first spelling put it at
//! 30 on the same one, which is the icon's width leaking into a flow that never wanted it). The
//! predicate is ATOMIC INLINE-LEVEL — `inline-block`, `inline-flex`, `inline-grid` — and nothing
//! else. `#noD` holds the other end: a pseudo with `content:""` and a width but NO `display` is an
//! ordinary inline box, and `width`/`height` do not apply to one.
//!
//! ⚠ **ONLY THE INLINE ADVANCE IS CLAIMED, AND THE VERTICAL IS NAMED RESIDUE.** A correct atomic
//! inline is placed about the BASELINE (its bottom margin edge sits on it), which is
//! `InlineItem::Atomic` and a synthesised `LayoutBox`, not a spacer. Chrome makes that 30px icon's
//! line **34px** tall and we still make it 19, so the spacer claims no height and contributes no
//! leading: the line box is exactly as tall as it was before this tick and only the horizontal
//! advance changes. Asserting 19 as correct would pin a wrong answer; asserting 34 would fail.
//!
//! ⚠ **AND A PSEUDO THAT HAS TEXT STILL IGNORES ITS OWN WIDTH.** `content:"x"` with the same
//! `inline-block; width:30px` puts Chrome's span at 58 and ours at 10 — the box is sized by its
//! glyph, not by its `width`. That is the same missing atomic box from the other side, it is NOT
//! fixed here, and it is deliberately not asserted.
//!
//! ⚠ **`#g` IS A CONTROL THAT PASSES FOR A REASON OTHER THAN THE ONE IT NAMES, AND SAYING SO IS
//! THE POINT.** Deleting the `content.is_none()` / `generated_box_is_suppressed` guard leaves this
//! gate GREEN: the cascade never builds a pseudo `ComputedStyle` at all unless a `content`
//! declaration matched, so the guard is belt-and-braces rather than load-bearing. It is kept
//! (cheap, and it stops depending on a cascade invariant from a different crate) and it is recorded
//! here as NOT falsified by this file, rather than counted among the mutations it does catch.
//!
//! ⚠ **WHAT THIS DOES NOT MOVE, SAID PLAINLY.** t1374 named `whatwg.org`'s four remaining misplaced
//! elements as this defect. They are **not** fixed by this tick: that page's `<a>` is a FLEX
//! container, so its `::before` must become a flex ITEM, which is a different code path from the
//! inline flow. The anchor is 89.2% before and after. This fixes the inline-flow half.
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
 *{box-sizing:border-box}
 body{margin:0;font:16px monospace}
 .b{background:#eee;width:600px}
 #e::before{content:"";display:inline-block;width:30px;height:30px;margin:0 20px 0 8px;background:#c00}
 #n::before{content:none;display:inline-block;width:30px;height:30px;margin:0 20px 0 8px;background:#c00}
 #blk::before{content:"";display:block;width:30px;height:30px;background:#c00}
 #noD::before{content:"";width:30px;height:30px;background:#c00}
 #aft::after{content:"";display:inline-block;width:30px;height:30px;background:#c00}
 #g::before{display:inline-block;width:30px;height:30px;margin:0 20px 0 8px;background:#c00}
</style></head><body>
<div class="b" id="e"><span id="es">AAAA</span></div>
<div class="b" id="n"><span id="ns">AAAA</span></div>
<div class="b" id="blk"><span id="blks">AAAA</span></div>
<div class="b" id="noD"><span id="noDs">AAAA</span></div>
<div class="b" id="aft"><span id="afts">AAAA</span></div>
<div class="b" id="g"><span id="gs">AAAA</span></div>
</body></html>
"##;

/// The x of the owner's span, relative to the owner — which is exactly the advance the generated
/// box did or did not take.
fn relx(page: &manuk_page::Page, owner: &str) -> f32 {
    let dom = page.dom();
    let pick = |sel: &str| {
        manuk_css::query_selector_all(dom, dom.root(), sel)
            .first()
            .copied()
            .unwrap_or_else(|| panic!("selector {sel} matched nothing"))
    };
    let rects = page.root_box.node_rects(dom);
    let o = rects
        .get(&pick(&format!("#{owner}")))
        .copied()
        .expect("owner box");
    let s = rects
        .get(&pick(&format!("#{owner}s")))
        .copied()
        .expect("span box");
    s.x - o.x
}

fn adv(page: &manuk_page::Page, owner: &str, want: f32, why: &str) {
    let got = relx(page, owner);
    assert!(
        (got - want).abs() < 1.01,
        "G_GENERATED_BOX_TAKES_ITS_ADVANCE: `#{owner}`'s text expected at x={want} relative to its \
         owner (CAPTURED from `google-chrome --headless --hide-scrollbars \
         --window-size=1200,800`), got x={got} — {why}"
    );
}

#[test]
fn g_generated_box_takes_its_advance() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://generatedbox.test/", &fonts, 1200.0);

    adv(
        &page,
        "e",
        58.0,
        "an inline-block `content:\"\"` box takes its width plus its horizontal margins — 8 + 30 + \
         20 — and the owner's text starts after it",
    );
    adv(
        &page,
        "n",
        0.0,
        "CONTROL: `content:none` generates no box at all, so there is nothing to advance past",
    );
    adv(
        &page,
        "blk",
        0.0,
        "⭐ CONTROL: a BLOCK-level generated box takes its own LINE, not an inline advance — the \
         row that caught the first spelling of this fix, which put the text at 30",
    );
    adv(
        &page,
        "noD",
        0.0,
        "⭐ CONTROL: with no `display` the generated box is an ordinary INLINE box, and \
         `width`/`height` do not apply to one",
    );
    adv(
        &page,
        "aft",
        0.0,
        "CONTROL: an `::after` box follows the owner's text, so it cannot move its start",
    );
    adv(
        &page,
        "g",
        0.0,
        "⭐ CONTROL: no `content` DECLARATION means no generated box at all, however completely the \
         rule sizes one — the row that makes the `content.is_none()` guard load-bearing",
    );
}
