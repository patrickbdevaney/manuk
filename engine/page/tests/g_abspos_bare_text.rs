//! # G_ABSPOS_BARE_TEXT — a text node is never out of flow
//!
//! ```html
//! <div style="position:absolute">Menu</div>
//! ```
//!
//! That box measured **0×0**. Not misplaced — *sized to nothing*, so every dropdown item, tooltip,
//! badge, absolutely-positioned caption and `.sr-only` label whose content is bare text collapsed to
//! a point.
//!
//! ## The mechanism, and it is a cascade quirk meeting a style-only predicate
//!
//! `layout_children` filters a container's out-of-flow children out of the in-flow list:
//!
//! ```rust
//! .filter(|&k| { let s = self.style_of(k); !is_float(s) && !is_out_of_flow_positioned(s) })
//! ```
//!
//! **Under the Stylo cascade a bare text node carries a CLONE of its parent's style.** So inside a
//! `position:absolute` box, the box's own text answers *yes, I am out of flow* and filters itself out
//! of the content it IS. The box then has no children to measure, `shrink_to_fit` returns 0, and the
//! content height is 0.
//!
//! An **element** child hid it completely: `<div style="position:absolute"><span>Menu</span></div>`
//! is correct, because the `<span>` carries its own `position: static`. So the bug fires on exactly
//! the shape people write and not on the shape a test-writer reaches for.
//!
//! ```text
//!                                                        Chrome   before   after
//!   <div abspos>bare text</div>                           62x20     0x0    62x20   ✗→✓
//!   …with padding:10px                                    82x40    20x20   82x40   ✗→✓
//!   …with height:40px (width still auto)                  62x40     0x40   62x40   ✗→✓
//!   <span abspos>bare text</span>                        130x20     0x0   130x20   ✗→✓
//!   <div abspos>text<div/>text</div>  (MIXED)             70x52     ?x?    70x52   ✗→✓
//!   <div position:fixed>bare text</div>                  101x20     0x0   101x20   ✗→✓
//!   <div abspos left:0 right:0>bare text</div>           600x20   600x0    600x20  ✗→✓
//!   <div abspos><span>elem child</span></div>             72x20    72x20   72x20   ✓ always right
//!   <div float:left>floated bare text</div>              115x20   115x20  115x20   ✓ always right
//! ```
//!
//! `max_content_width_uncached` already documented this exact trap for `display:flex` — *"a bare run
//! inside `display:flex` reads back as `flex` here"* — and guards it with `is_element`. **Same
//! cascade quirk, same guard, four more call sites**: the in-flow filter, the has-a-float check, the
//! static-position loop, and the block-children dispatch. The fix is two node-aware predicates,
//! `kid_is_float` / `kid_is_out_of_flow`, which every child filter must now use.
//!
//! ## How this goes RED
//!
//! - **Restore the raw style predicates in the `flow_kids` filter** → `#c1`, `#c5`, `#c6`, `#c8`,
//!   `#ca` and `#cb` collapse to 0-size, while `#cc` (an element child) and `#cf` (a float) still
//!   pass. That split is the whole point: a gate written with a `<span>` inside the abspos box is
//!   green against this defect, which is why it survived.
//! - **Restore them in the BLOCK-children dispatch only** → `#c9` alone fails: the mixed
//!   text+block+text case is the one that takes that loop. It is in the fixture because the two
//!   paths are separately wrong and were separately fixed.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.25 sans-serif}
.r{position:relative;width:600px;height:90px}
.b{position:absolute}
</style></head><body>
<div class="r"><div class="b" id="c1">bare text</div></div>
<div class="r"><div class="b" id="c5" style="padding:10px">bare text</div></div>
<div class="r"><div class="b" id="c6" style="height:40px">bare text</div></div>
<div class="r"><span class="b" id="c8">an abspos INLINE</span></div>
<div class="r"><div class="b" id="c9">lead text<div id="c9k" style="width:70px;height:12px"></div>tail text</div></div>
<div class="r"><div class="b" id="ca" style="position:fixed">fixed bare text</div></div>
<div class="r"><div class="b" id="cb" style="left:0;right:0">bare text</div></div>
<div class="r"><div class="b" id="cc"><span>elem child</span></div></div>
<div class="r" style="height:40px"><div id="cf" style="float:left">floated bare text</div></div>
</body></html>"##;

fn size_of(page: &manuk_page::Page, sel: &str) -> [f32; 2] {
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
        .unwrap_or_else(|| panic!("no box for {sel} — the element generated none at all"));
    [r.width, r.height]
}

fn assert_size(page: &manuk_page::Page, sel: &str, w: f32, h: f32, why: &str) {
    let s = size_of(page, sel);
    assert!(
        (s[0] - w).abs() < 1.01 && (s[1] - h).abs() < 1.01,
        "G_ABSPOS_BARE_TEXT: `{sel}` expected {w}x{h} (MEASURED in headless Chrome on THIS fixture), \
         got {}x{}.\n  {why}",
        s[0],
        s[1]
    );
}

#[test]
fn g_abspos_bare_text() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://abs.test/", &fonts, 1200.0);

    // ── THE BUG: the box's own text filtered itself out of the box.
    assert_size(
        &page,
        "#c1",
        62.0,
        20.0,
        "`<div style=position:absolute>bare text</div>` — the shape every dropdown item, tooltip and \
         badge is written in. It measured 0x0",
    );
    assert_size(
        &page,
        "#c5",
        82.0,
        40.0,
        "padding must frame real content, not nothing: at 0x0 this read 20x20, which is the padding \
         alone and looks like a box rather than like an absence",
    );
    assert_size(
        &page,
        "#c6",
        62.0,
        40.0,
        "an explicit HEIGHT hides half the bug — the height was right and the width was still 0, so \
         a gate checking only height would have passed",
    );
    assert_size(
        &page,
        "#c8",
        130.0,
        20.0,
        "an abspos INLINE element blockifies and hits the same filter",
    );
    assert_size(
        &page,
        "#ca",
        101.0,
        20.0,
        "`position:fixed` is the other out-of-flow value and shares the predicate",
    );
    assert_size(
        &page,
        "#cb",
        600.0,
        20.0,
        "with left+right set the WIDTH came from the insets and was already right; only the height \
         was 0. Both halves are asserted because they fail independently",
    );

    // ── THE SECOND CALL SITE: mixed text + block + text takes the block-children loop, not the
    //    pure-IFC branch, and was wrong there for the same reason.
    assert_size(
        &page,
        "#c9",
        70.0,
        52.0,
        "text, then a block child, then text — the BLOCK dispatch. Two lines of text plus a 12px \
         block; if either text run is dropped this is 32 or 40, not 52",
    );

    // ── WHAT WAS ALWAYS RIGHT, and is what made this invisible.
    assert_size(
        &page,
        "#cc",
        72.0,
        20.0,
        "an ELEMENT child carries its own `position:static`, so it never filtered itself out. This \
         is the case a test-writer reaches for, and it has always passed",
    );
    assert_size(
        &page,
        "#cf",
        115.0,
        20.0,
        "a FLOAT with bare text — the same predicate pair, and it must not regress",
    );
}
