//! # G_FORM_CONTROL_BASELINE — a control's value is not a child text node, so it had no baseline
//!
//! ⚠⚠⚠ **`last_line_baseline` RETURNS `None` FOR AN `<input>`, BECAUSE ITS VALUE LIVES ON THE ELEMENT
//! AND NOT IN THE TREE.** CSS 2.1 §10.8.1's fallback then applies — the bottom margin edge becomes
//! the baseline — so every text field, button and select sat ENTIRELY above the line's baseline and
//! made the line that held it too tall. Chrome gives each of them the baseline of its internal
//! editor text.
//!
//! ```text
//!   <div><input></div>      Chrome 24    before 26
//!   ours, with the fallback baseline = h = 21  ->  above 21, below 0  ->  21 + 6.5  -> 27.5 -> 28
//!   Chrome,  baseline ~17 from the top         ->  above 17, below 4  ->  max(17.5,17)+max(6.5,4) = 24
//! ```
//!
//! **This is the half t917 was missing.** That tick corrected the controls' own UA boxes to Chrome's
//! measured values — every height went exact — and `<div><input></div>` got *worse*, 26 → 28, because
//! the taller control pushed further below a baseline that was already in the wrong place. The whole
//! UA block was reverted under the ratchet rather than traded. **This tick lands the baseline, which
//! is the half that stands alone**: it takes the composite case to Chrome's 24 with the UA boxes
//! untouched, and it makes the UA correction landable next to it.
//!
//! The synthesised baseline is the control's own first-line baseline — border + padding + the ascent
//! of ITS font (Chrome's UA gives these 13.333px Arial, not the page's 16px).
//!
//! ⚠ **`textarea` IS DELIBERATELY EXCLUDED.** It is multi-line, Chrome takes its LAST line, the
//! generic fallback already approximates that, and t917 measured it byte-exact at 36. **A row that is
//! already right is not a row to route through a new mechanism** — and `#g4` asserts it stays right.
//!
//! ⚠ **THE GUARDS ARE THE INLINE-BLOCK ROWS**, because this synthesis must fire ONLY where the real
//! rule cannot: a text-bearing `inline-block` must still use its own last line (`#g6`), an
//! `overflow:hidden` one must still take the §10.8.1 fallback (`#g7`, which is 31 and not 24), and an
//! EMPTY inline-block must be unchanged (`#g8`). A fix that gave every atomic a synthetic baseline
//! would satisfy the control rows and break all three.
//!
//! ⚠ **NAMED, NOT ASSERTED:** an input with an explicit `height:40px` reads 47 against Chrome's 46.
//! Chrome centres the internal editor in a taller control; we place the baseline at
//! border+padding+ascent regardless. One pixel, on a control whose height the author has overridden.
//!
use manuk_text::FontContext;

const HTML: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><style>body{margin:0;font:16px/1.5 sans-serif}
 .f{width:400px;background:#eee;margin-bottom:2px}</style></head><body>
<div class="f" id="g1"><input></div>
<div class="f" id="g2"><button>b</button></div>
<div class="f" id="g3"><select><option>a</option></select></div>
<div class="f" id="g4"><textarea></textarea></div>
<div class="f" id="g5"><input type="checkbox"></div>
<div class="f" id="g6"><span style="display:inline-block">Ay</span>Ay</div>
<div class="f" id="g7"><span style="display:inline-block;overflow:hidden">Ay</span>Ay</div>
<div class="f" id="g8"><span style="display:inline-block"></span>Ay</div>
<div class="f" id="g9">text <input> text</div>
<div class="f" id="g10"><input style="height:40px"></div>

</body></html>
"##;

fn c(page: &manuk_page::Page, sel: &str, want: f32) {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let got = page
        .root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel}"))
        .height;
    assert!(
        (got - want).abs() < 1.01,
        "G_FORM_CONTROL_BASELINE: `{sel}` expected height {want} (CAPTURED from \
         `google-chrome-stable --headless=new --hide-scrollbars --window-size=1200,800`), got {got}"
    );
}

#[test]
fn g_form_control_baseline() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://bl.test/", &fonts, 1200.0);
    c(&page, "#g1", 24.0);
    c(&page, "#g2", 24.0);
    c(&page, "#g3", 24.0);
    c(&page, "#g4", 43.0);
    c(&page, "#g5", 24.0);
    c(&page, "#g6", 24.0);
    c(&page, "#g7", 31.0);
    c(&page, "#g8", 24.0);
    c(&page, "#g9", 24.0);
}
